use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tracing::{debug, error, info, warn};

use crate::cleanup::{CleanupOptions, CleanupResult, DeviceCleanupHandler};
use crate::device::DeviceDetector;
use crate::error::{DeviceError, Error, Result};
use crate::playlist::PlaylistManager;
use crate::transfer::{TransferEngine, TransferProgress, TransferResult, TransferStatus};

use super::types::{SyncOptions, SyncPhase, SyncProgress, SyncRequest, SyncResult};

// =============================================================================
// Sync Orchestrator
// =============================================================================

/// Orchestrator for synchronizing playlists to devices.
///
/// The orchestrator coordinates the complete sync workflow:
/// 1. Verify device connection and available space
/// 2. Clean up old content from the device (optional)
/// 3. Transfer selected playlists to the device
///
/// It provides progress tracking, cancellation support, and error handling.
#[derive(Debug)]
pub struct SyncOrchestrator {
    /// Cancellation flag.
    cancelled: Arc<AtomicBool>,
    /// Cleanup handler.
    cleanup_handler: DeviceCleanupHandler,
}

impl Default for SyncOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncOrchestrator {
    /// Create a new sync orchestrator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            cleanup_handler: DeviceCleanupHandler::new(),
        }
    }

    /// Create a sync orchestrator with a shared cancellation flag.
    #[must_use]
    pub fn with_cancellation(cancelled: Arc<AtomicBool>) -> Self {
        Self {
            cancelled,
            cleanup_handler: DeviceCleanupHandler::new(),
        }
    }

    /// Request cancellation of the sync operation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Check if cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Get a cancellation token that can be shared across threads.
    #[must_use]
    pub fn cancellation_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    /// Perform the sync operation.
    ///
    /// # Arguments
    ///
    /// * `playlist_manager` - Manager for accessing playlists
    /// * `device_detector` - Detector for verifying device connection
    /// * `request` - The sync request specifying playlists and device
    /// * `options` - Configuration options for the sync
    /// * `progress_callback` - Optional callback for progress updates
    ///
    /// # Errors
    ///
    /// Returns an error if the sync fails due to device issues, permission errors, etc.
    #[allow(
        clippy::too_many_lines,
        reason = "sync orchestration is inherently sequential and splitting would reduce readability"
    )]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "SyncRequest and Option<F> are consumed by the sync operation"
    )]
    pub fn sync<D, F>(
        &self,
        playlist_manager: &PlaylistManager,
        device_detector: &D,
        request: SyncRequest,
        options: &SyncOptions,
        progress_callback: Option<F>,
    ) -> Result<SyncResult>
    where
        D: DeviceDetector,
        F: Fn(&SyncProgress),
    {
        let start_time = Instant::now();
        let total_playlists = request.playlists.len();
        let mut result = SyncResult::empty(total_playlists);
        let mut progress = SyncProgress::verifying(total_playlists);

        info!(
            "Starting sync of {} playlist(s) to {}",
            total_playlists,
            request.device_mount_point.display()
        );

        Self::emit_progress(progress_callback.as_ref(), &progress);

        // Check for cancellation
        if self.check_cancelled(&mut progress, &mut result, progress_callback.as_ref()) {
            return Ok(result);
        }

        // Phase 1: Verify device
        self.run_verification_phase(
            device_detector,
            playlist_manager,
            &request,
            &mut progress,
            &mut result,
            progress_callback.as_ref(),
        )?;

        // Check for cancellation
        if self.check_cancelled(&mut progress, &mut result, progress_callback.as_ref()) {
            return Ok(result);
        }

        // Phase 2: Cleanup (if enabled)
        if options.cleanup_enabled {
            self.run_cleanup_phase(
                device_detector,
                &request,
                options,
                &mut progress,
                &mut result,
                progress_callback.as_ref(),
            )?;
        }

        // Check for cancellation
        if self.check_cancelled(&mut progress, &mut result, progress_callback.as_ref()) {
            return Ok(result);
        }

        // Phase 3: Transfer playlists
        self.run_transfer_phase(
            playlist_manager,
            device_detector,
            &request,
            options,
            &mut progress,
            &mut result,
            progress_callback.as_ref(),
        )?;

        // Finalize result
        let duration_secs = start_time.elapsed().as_secs_f64();
        result.finalize(duration_secs);
        progress.completed(duration_secs);

        info!("{}", result.summary());

        Self::emit_progress(progress_callback.as_ref(), &progress);

        Ok(result)
    }

    /// Emit progress to callback if present.
    fn emit_progress<F: Fn(&SyncProgress)>(callback: Option<&F>, progress: &SyncProgress) {
        if let Some(cb) = callback {
            cb(progress);
        }
    }

    /// Check if cancelled and update state accordingly. Returns true if cancelled.
    fn check_cancelled<F: Fn(&SyncProgress)>(
        &self,
        progress: &mut SyncProgress,
        result: &mut SyncResult,
        callback: Option<&F>,
    ) -> bool {
        if self.is_cancelled() {
            progress.cancelled();
            result.was_cancelled = true;
            result.final_phase = SyncPhase::Cancelled;
            Self::emit_progress(callback, progress);
            true
        } else {
            false
        }
    }

    /// Handle a sync failure by updating progress and result.
    fn handle_failure<F: Fn(&SyncProgress)>(
        progress: &mut SyncProgress,
        result: &mut SyncResult,
        callback: Option<&F>,
        message: String,
        error: &Error,
    ) {
        progress.failed(message);
        result.error_message = Some(error.to_string());
        result.final_phase = SyncPhase::Failed;
        Self::emit_progress(callback, progress);
    }

    /// Run Phase 1: Device verification.
    fn run_verification_phase<D, F>(
        &self,
        device_detector: &D,
        playlist_manager: &PlaylistManager,
        request: &SyncRequest,
        progress: &mut SyncProgress,
        result: &mut SyncResult,
        callback: Option<&F>,
    ) -> Result<()>
    where
        D: DeviceDetector,
        F: Fn(&SyncProgress),
    {
        info!("Phase 1: Verifying device...");
        if let Err(e) = self.verify_device(device_detector, &request.device_mount_point) {
            error!("Device verification failed: {}", e);
            Self::handle_failure(
                progress,
                result,
                callback,
                format!("Device verification failed: {e}"),
                &e,
            );
            return Err(e);
        }

        // Calculate total bytes to transfer
        let total_bytes = self.calculate_total_bytes(playlist_manager, &request.playlists)?;
        progress.total_bytes = total_bytes;

        // Verify device has enough space
        self.verify_device_space(&request.device_mount_point, total_bytes)?;

        Ok(())
    }

    /// Run Phase 2: Device cleanup.
    fn run_cleanup_phase<D, F>(
        &self,
        device_detector: &D,
        request: &SyncRequest,
        options: &SyncOptions,
        progress: &mut SyncProgress,
        result: &mut SyncResult,
        callback: Option<&F>,
    ) -> Result<()>
    where
        D: DeviceDetector,
        F: Fn(&SyncProgress),
    {
        info!("Phase 2: Cleaning device...");
        progress.cleaning("Cleaning device contents...");
        progress.overall_progress_percent = 5.0;
        Self::emit_progress(callback, progress);

        match self.cleanup_device(
            device_detector,
            &request.device_mount_point,
            &options.cleanup_options,
        ) {
            Ok(cleanup_result) => {
                info!(
                    "Cleanup complete: {} files, {} directories deleted ({} bytes freed)",
                    cleanup_result.files_deleted,
                    cleanup_result.directories_deleted,
                    cleanup_result.bytes_freed
                );
                progress.cleanup_result = Some(cleanup_result.clone());
                result.cleanup_result = Some(cleanup_result);
                progress.overall_progress_percent = 10.0;
            }
            Err(e) => {
                error!("Cleanup failed: {}", e);
                if options.abort_on_cleanup_failure {
                    Self::handle_failure(
                        progress,
                        result,
                        callback,
                        format!("Cleanup failed: {e}"),
                        &e,
                    );
                    return Err(e);
                }
                warn!("Cleanup failed but continuing: {}", e);
            }
        }

        // Verify device again after cleanup
        if options.verify_device_between_phases
            && let Err(e) = self.verify_device(device_detector, &request.device_mount_point)
        {
            error!("Device disconnected during cleanup: {}", e);
            Self::handle_failure(
                progress,
                result,
                callback,
                format!("Device disconnected: {e}"),
                &e,
            );
            return Err(e);
        }

        Ok(())
    }

    /// Run Phase 3: Transfer playlists.
    #[allow(
        clippy::too_many_arguments,
        reason = "transfer phase needs access to all orchestration state"
    )]
    fn run_transfer_phase<D, F>(
        &self,
        playlist_manager: &PlaylistManager,
        device_detector: &D,
        request: &SyncRequest,
        options: &SyncOptions,
        progress: &mut SyncProgress,
        result: &mut SyncResult,
        callback: Option<&F>,
    ) -> Result<()>
    where
        D: DeviceDetector,
        F: Fn(&SyncProgress),
    {
        info!("Phase 3: Transferring playlists...");
        let mut transfer_engine = TransferEngine::with_cancellation(Arc::clone(&self.cancelled));
        let total_playlists = request.playlists.len();

        for (index, playlist_name) in request.playlists.iter().enumerate() {
            // Check for cancellation
            if self.check_cancelled(progress, result, callback) {
                return Ok(());
            }

            // Verify device before each playlist (if enabled)
            if options.verify_device_between_phases
                && index > 0
                && let Err(e) = self.verify_device(device_detector, &request.device_mount_point)
            {
                error!("Device disconnected during transfer: {}", e);
                Self::handle_failure(
                    progress,
                    result,
                    callback,
                    format!("Device disconnected: {e}"),
                    &e,
                );
                return Err(e);
            }

            self.transfer_single_playlist(
                &mut transfer_engine,
                playlist_manager,
                playlist_name,
                index,
                total_playlists,
                request,
                options,
                progress,
                result,
                callback,
            )?;
        }

        Ok(())
    }

    /// Transfer a single playlist to the device.
    #[allow(
        clippy::too_many_arguments,
        reason = "single playlist transfer needs access to all orchestration state"
    )]
    fn transfer_single_playlist<F>(
        &self,
        transfer_engine: &mut TransferEngine,
        playlist_manager: &PlaylistManager,
        playlist_name: &str,
        index: usize,
        total_playlists: usize,
        request: &SyncRequest,
        options: &SyncOptions,
        progress: &mut SyncProgress,
        result: &mut SyncResult,
        callback: Option<&F>,
    ) -> Result<()>
    where
        F: Fn(&SyncProgress),
    {
        let playlist_index = index + 1;

        info!(
            "Transferring playlist {}/{}: {}",
            playlist_index, total_playlists, playlist_name
        );
        progress.transferring(playlist_name.to_string(), playlist_index);
        Self::emit_progress(callback, progress);

        let playlist_path = playlist_manager.get_playlist_path(playlist_name)?;
        #[allow(
            clippy::float_arithmetic,
            clippy::cast_precision_loss,
            reason = "progress weight calculation; precision loss acceptable"
        )]
        let playlist_weight = if total_playlists > 0 {
            1.0 / total_playlists as f64
        } else {
            1.0
        };

        let transfer_result = transfer_engine.transfer_playlist(
            &playlist_path,
            &request.device_mount_point,
            &options.transfer_options,
            None::<fn(&TransferProgress)>,
        );

        match transfer_result {
            Ok(transfer_result) => {
                self.handle_transfer_success(
                    playlist_name,
                    transfer_result,
                    playlist_weight,
                    progress,
                    result,
                    callback,
                );
                Ok(())
            }
            Err(e) => self.handle_transfer_error(playlist_name, e, progress, result, callback),
        }
    }

    /// Handle successful playlist transfer.
    #[allow(
        clippy::unused_self,
        reason = "method for API consistency with other handler methods"
    )]
    fn handle_transfer_success<F>(
        &self,
        playlist_name: &str,
        transfer_result: TransferResult,
        playlist_weight: f64,
        progress: &mut SyncProgress,
        result: &mut SyncResult,
        callback: Option<&F>,
    ) where
        F: Fn(&SyncProgress),
    {
        info!(
            "Playlist '{}' transferred: {} files, {} bytes",
            playlist_name, transfer_result.files_transferred, transfer_result.bytes_transferred
        );

        let fake_progress = TransferProgress {
            status: if transfer_result.success {
                TransferStatus::Completed
            } else {
                TransferStatus::Failed
            },
            current_file_index: transfer_result.total_files,
            total_files: transfer_result.total_files,
            current_file_name: String::new(),
            current_file_bytes: 0,
            current_file_total: 0,
            total_bytes_transferred: transfer_result.bytes_transferred,
            total_bytes: transfer_result.bytes_transferred + transfer_result.bytes_skipped,
            files_completed: transfer_result.files_transferred,
            files_skipped: transfer_result.files_skipped,
            files_failed: transfer_result.files_failed,
            transfer_speed_bps: transfer_result.average_speed_bps,
            estimated_remaining_secs: Some(0.0),
            elapsed_secs: transfer_result.duration_secs,
        };
        progress.update_transfer_progress(&fake_progress, playlist_weight);

        if transfer_result.was_cancelled {
            progress.cancelled();
            result.was_cancelled = true;
            result.add_transfer_result(playlist_name.to_string(), transfer_result);
            result.final_phase = SyncPhase::Cancelled;
            Self::emit_progress(callback, progress);
            return;
        }

        result.add_transfer_result(playlist_name.to_string(), transfer_result);
        Self::emit_progress(callback, progress);
    }

    /// Handle transfer error for a playlist.
    #[allow(
        clippy::unused_self,
        reason = "method for API consistency with other handler methods"
    )]
    fn handle_transfer_error<F>(
        &self,
        playlist_name: &str,
        error: Error,
        progress: &mut SyncProgress,
        result: &mut SyncResult,
        callback: Option<&F>,
    ) -> Result<()>
    where
        F: Fn(&SyncProgress),
    {
        if matches!(error, Error::Cancelled) {
            progress.cancelled();
            result.was_cancelled = true;
            result.final_phase = SyncPhase::Cancelled;
            Self::emit_progress(callback, progress);
            return Ok(());
        }

        error!("Failed to transfer playlist '{}': {}", playlist_name, error);
        Self::handle_failure(
            progress,
            result,
            callback,
            format!("Transfer failed for '{playlist_name}': {error}"),
            &error,
        );
        Err(error)
    }

    /// Verify that the device is connected and accessible.
    #[allow(
        clippy::unused_self,
        reason = "method for API consistency with other orchestrator methods"
    )]
    fn verify_device<D: DeviceDetector>(&self, detector: &D, mount_point: &Path) -> Result<()> {
        if !detector.is_device_connected(mount_point) {
            return Err(Error::Device(DeviceError::Disconnected {
                name: mount_point.display().to_string(),
            }));
        }

        if !mount_point.exists() {
            return Err(Error::Device(DeviceError::NotMounted {
                mount_point: mount_point.to_path_buf(),
            }));
        }

        if !mount_point.is_dir() {
            return Err(Error::Device(DeviceError::NotMounted {
                mount_point: mount_point.to_path_buf(),
            }));
        }

        debug!("Device verified at {}", mount_point.display());
        Ok(())
    }

    /// Verify the device has enough space.
    #[allow(
        clippy::unused_self,
        reason = "method for API consistency with other orchestrator methods"
    )]
    fn verify_device_space(&self, mount_point: &Path, required_bytes: u64) -> Result<()> {
        // Use sysinfo to check available space
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();

        for disk in &disks {
            if disk.mount_point() == mount_point {
                let available = disk.available_space();
                if available < required_bytes {
                    return Err(Error::Device(DeviceError::InsufficientSpace {
                        device: mount_point.display().to_string(),
                        required_bytes,
                        available_bytes: available,
                    }));
                }
                debug!(
                    "Device has {} bytes available, {} bytes required",
                    available, required_bytes
                );
                return Ok(());
            }
        }

        // If we can't find the disk, assume it has enough space
        // (the transfer will fail later if not)
        warn!("Could not verify device space, proceeding anyway");
        Ok(())
    }

    /// Calculate total bytes to transfer for all playlists.
    #[allow(
        clippy::unused_self,
        reason = "method for API consistency with other orchestrator methods"
    )]
    fn calculate_total_bytes(
        &self,
        playlist_manager: &PlaylistManager,
        playlists: &[String],
    ) -> Result<u64> {
        let mut total_bytes = 0u64;

        for playlist_name in playlists {
            let stats = playlist_manager.get_folder_statistics(playlist_name)?;
            total_bytes += stats.audio_size_bytes;
        }

        debug!("Total bytes to transfer: {}", total_bytes);
        Ok(total_bytes)
    }

    /// Perform device cleanup.
    fn cleanup_device<D: DeviceDetector>(
        &self,
        detector: &D,
        mount_point: &Path,
        options: &CleanupOptions,
    ) -> Result<CleanupResult> {
        // Get device info for verified cleanup
        let devices = detector.list_devices()?;
        let device = devices
            .iter()
            .find(|d| d.mount_point == mount_point)
            .ok_or_else(|| {
                Error::Device(DeviceError::NotFound {
                    name: mount_point.display().to_string(),
                })
            })?;

        self.cleanup_handler
            .cleanup_device_verified(detector, device, options)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    reason = "test code uses unwrap/expect for brevity and exact float comparisons for known values"
)]
mod tests {
    use super::*;
    use crate::device::DeviceInfo;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Mock device detector for testing.
    struct MockDeviceDetector {
        devices: Vec<DeviceInfo>,
    }

    impl MockDeviceDetector {
        fn new() -> Self {
            Self {
                devices: Vec::new(),
            }
        }

        fn with_device(mut self, mount_point: PathBuf) -> Self {
            self.devices.push(DeviceInfo {
                name: "Test Device".to_string(),
                mount_point,
                total_bytes: 1_000_000_000,
                available_bytes: 500_000_000,
                file_system: "FAT32".to_string(),
                is_removable: true,
            });
            self
        }
    }

    impl DeviceDetector for MockDeviceDetector {
        fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
            Ok(self.devices.clone())
        }

        fn is_device_connected(&self, mount_point: &Path) -> bool {
            self.devices.iter().any(|d| d.mount_point == mount_point)
        }

        fn refresh(&mut self) {}
    }

    fn setup_test_environment() -> (PlaylistManager, TempDir, TempDir) {
        let playlists_dir = TempDir::new().expect("create playlists dir");
        let device_dir = TempDir::new().expect("create device dir");

        let manager = PlaylistManager::new(playlists_dir.path().to_path_buf())
            .expect("create playlist manager");

        (manager, playlists_dir, device_dir)
    }

    #[test]
    fn test_sync_options_default() {
        let options = SyncOptions::default();
        assert!(options.cleanup_enabled);
        assert!(options.verify_device_between_phases);
        assert!(options.abort_on_cleanup_failure);
        assert!(options.skip_existing_matches);
    }

    #[test]
    fn test_sync_options_fast() {
        let options = SyncOptions::fast();
        assert!(options.cleanup_enabled);
        assert!(!options.verify_device_between_phases);
        assert!(!options.transfer_options.verify_integrity);
    }

    #[test]
    fn test_sync_options_reliable() {
        let options = SyncOptions::reliable();
        assert!(options.cleanup_enabled);
        assert!(options.verify_device_between_phases);
        assert!(options.transfer_options.verify_integrity);
        assert!(!options.skip_existing_matches);
    }

    #[test]
    fn test_sync_request_single() {
        let request = SyncRequest::single("My Playlist", "/mnt/usb");
        assert_eq!(request.playlists.len(), 1);
        assert_eq!(request.playlists[0], "My Playlist");
        assert_eq!(request.device_mount_point.to_str().unwrap(), "/mnt/usb");
    }

    #[test]
    fn test_sync_progress_verifying() {
        let progress = SyncProgress::verifying(3);
        assert_eq!(progress.phase, SyncPhase::Verifying);
        assert_eq!(progress.total_playlists, 3);
        assert_eq!(progress.overall_progress_percent, 0.0);
    }

    #[test]
    fn test_sync_progress_cleaning() {
        let mut progress = SyncProgress::verifying(1);
        progress.cleaning("Cleaning...");
        assert_eq!(progress.phase, SyncPhase::Cleaning);
        assert_eq!(progress.message, "Cleaning...");
    }

    #[test]
    fn test_sync_progress_completed() {
        let mut progress = SyncProgress::verifying(1);
        progress.completed(10.5);
        assert_eq!(progress.phase, SyncPhase::Completed);
        assert_eq!(progress.overall_progress_percent, 100.0);
        assert_eq!(progress.elapsed_secs, 10.5);
    }

    #[test]
    fn test_sync_result_summary() {
        let mut result = SyncResult::empty(1);
        result.total_files_transferred = 10;
        result.total_bytes_transferred = 1_000_000;
        result.finalize(5.0);

        let summary = result.summary();
        assert!(summary.contains("10 files transferred"));
    }

    #[test]
    fn test_sync_result_cancelled_summary() {
        let mut result = SyncResult::empty(1);
        result.was_cancelled = true;
        result.total_files_transferred = 5;
        result.finalize(3.0);

        let summary = result.summary();
        assert!(summary.contains("cancelled"));
    }

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = SyncOrchestrator::new();
        assert!(!orchestrator.is_cancelled());
    }

    #[test]
    fn test_orchestrator_cancellation() {
        let orchestrator = SyncOrchestrator::new();
        assert!(!orchestrator.is_cancelled());

        orchestrator.cancel();
        assert!(orchestrator.is_cancelled());
    }

    #[test]
    fn test_orchestrator_with_shared_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let orchestrator = SyncOrchestrator::with_cancellation(Arc::clone(&cancelled));

        cancelled.store(true, Ordering::SeqCst);
        assert!(orchestrator.is_cancelled());
    }

    #[test]
    fn test_sync_with_disconnected_device() {
        let (manager, _playlists_dir, device_dir) = setup_test_environment();
        let detector = MockDeviceDetector::new(); // No devices

        let orchestrator = SyncOrchestrator::new();
        let request = SyncRequest::single("Test", device_dir.path());
        let options = SyncOptions::default();

        let result = orchestrator.sync(
            &manager,
            &detector,
            request,
            &options,
            None::<fn(&SyncProgress)>,
        );

        result.unwrap_err();
    }

    #[test]
    fn test_sync_single_playlist() {
        let (manager, _playlists_dir, device_dir) = setup_test_environment();

        // Create a playlist with some tracks
        let playlist_path = manager
            .create_playlist("Test Playlist", None)
            .expect("create playlist");
        fs::write(playlist_path.join("track1.mp3"), "fake mp3 content 1").expect("write track1");
        fs::write(playlist_path.join("track2.mp3"), "fake mp3 content 2").expect("write track2");

        // Setup mock detector
        let detector = MockDeviceDetector::new().with_device(device_dir.path().to_path_buf());

        let orchestrator = SyncOrchestrator::new();
        let request = SyncRequest::single("Test Playlist", device_dir.path());
        let options = SyncOptions {
            cleanup_enabled: false, // Skip cleanup for this test
            ..Default::default()
        };

        let result = orchestrator
            .sync(
                &manager,
                &detector,
                request,
                &options,
                None::<fn(&SyncProgress)>,
            )
            .expect("sync should succeed");

        assert!(result.success);
        assert_eq!(result.total_files_transferred, 2);
        assert_eq!(result.transfer_results.len(), 1);

        // Verify files exist on device
        assert!(device_dir.path().join("track1.mp3").exists());
        assert!(device_dir.path().join("track2.mp3").exists());
    }

    #[test]
    fn test_sync_with_progress_callback() {
        let (manager, _playlists_dir, device_dir) = setup_test_environment();

        // Create a playlist
        let playlist_path = manager
            .create_playlist("Progress Test", None)
            .expect("create playlist");
        fs::write(playlist_path.join("track.mp3"), "content").expect("write track");

        let detector = MockDeviceDetector::new().with_device(device_dir.path().to_path_buf());

        let orchestrator = SyncOrchestrator::new();
        let request = SyncRequest::single("Progress Test", device_dir.path());
        let options = SyncOptions {
            cleanup_enabled: false,
            ..Default::default()
        };

        let progress_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let updates_clone = Arc::clone(&progress_updates);

        let result = orchestrator
            .sync(
                &manager,
                &detector,
                request,
                &options,
                Some(move |p: &SyncProgress| {
                    updates_clone.lock().unwrap().push(p.phase);
                }),
            )
            .expect("sync should succeed");

        assert!(result.success);

        let updates = progress_updates.lock().unwrap();
        assert!(!updates.is_empty());
        assert!(updates.contains(&SyncPhase::Verifying));
        assert!(updates.contains(&SyncPhase::Completed));
    }

    #[test]
    fn test_sync_cancelled_early() {
        let (manager, _playlists_dir, device_dir) = setup_test_environment();

        // Create a playlist
        manager
            .create_playlist("Cancel Test", None)
            .expect("create playlist");

        let detector = MockDeviceDetector::new().with_device(device_dir.path().to_path_buf());

        let orchestrator = SyncOrchestrator::new();
        orchestrator.cancel(); // Cancel before starting

        let request = SyncRequest::single("Cancel Test", device_dir.path());
        let options = SyncOptions::default();

        let result = orchestrator
            .sync(
                &manager,
                &detector,
                request,
                &options,
                None::<fn(&SyncProgress)>,
            )
            .expect("should return result even when cancelled");

        assert!(result.was_cancelled);
        assert_eq!(result.final_phase, SyncPhase::Cancelled);
    }

    #[test]
    fn test_sync_phase_display() {
        assert_eq!(SyncPhase::Verifying.to_string(), "Verifying");
        assert_eq!(SyncPhase::Cleaning.to_string(), "Cleaning");
        assert_eq!(SyncPhase::Transferring.to_string(), "Transferring");
        assert_eq!(SyncPhase::Completed.to_string(), "Completed");
        assert_eq!(SyncPhase::Failed.to_string(), "Failed");
        assert_eq!(SyncPhase::Cancelled.to_string(), "Cancelled");
    }

    // -------------------------------------------------------------------------
    // SyncOrchestrator Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_orchestrator_default() {
        let orchestrator = SyncOrchestrator::default();
        assert!(!orchestrator.is_cancelled());
    }

    #[test]
    fn test_orchestrator_cancellation_token() {
        let orchestrator = SyncOrchestrator::new();
        let token1 = orchestrator.cancellation_token();
        let token2 = orchestrator.cancellation_token();

        // Both tokens should refer to the same flag
        token1.store(true, Ordering::SeqCst);
        assert!(token2.load(Ordering::SeqCst));
        assert!(orchestrator.is_cancelled());
    }

    #[test]
    fn test_sync_multiple_playlists() {
        let (manager, _playlists_dir, device_dir) = setup_test_environment();

        // Create multiple playlists
        let playlist1_path = manager
            .create_playlist("Playlist 1", None)
            .expect("create playlist 1");
        fs::write(playlist1_path.join("track1.mp3"), "content 1").expect("write track");

        let playlist2_path = manager
            .create_playlist("Playlist 2", None)
            .expect("create playlist 2");
        fs::write(playlist2_path.join("track2.mp3"), "content 2").expect("write track");

        let detector = MockDeviceDetector::new().with_device(device_dir.path().to_path_buf());

        let orchestrator = SyncOrchestrator::new();
        let request = SyncRequest::new(
            vec!["Playlist 1".to_string(), "Playlist 2".to_string()],
            device_dir.path().to_path_buf(),
        );
        let options = SyncOptions {
            cleanup_enabled: false,
            ..Default::default()
        };

        let result = orchestrator
            .sync(
                &manager,
                &detector,
                request,
                &options,
                None::<fn(&SyncProgress)>,
            )
            .expect("sync should succeed");

        assert!(result.success);
        assert_eq!(result.transfer_results.len(), 2);
        assert!(device_dir.path().join("track1.mp3").exists());
        assert!(device_dir.path().join("track2.mp3").exists());
    }

    #[test]
    fn test_sync_empty_playlist_list() {
        let (manager, _playlists_dir, device_dir) = setup_test_environment();

        let detector = MockDeviceDetector::new().with_device(device_dir.path().to_path_buf());

        let orchestrator = SyncOrchestrator::new();
        let request = SyncRequest::new(Vec::new(), device_dir.path().to_path_buf());
        let options = SyncOptions {
            cleanup_enabled: false,
            ..Default::default()
        };

        let result = orchestrator
            .sync(
                &manager,
                &detector,
                request,
                &options,
                None::<fn(&SyncProgress)>,
            )
            .expect("sync should succeed");

        assert!(result.success);
        assert_eq!(result.total_files_transferred, 0);
    }

    #[test]
    fn test_sync_nonexistent_playlist() {
        let (manager, _playlists_dir, device_dir) = setup_test_environment();

        let detector = MockDeviceDetector::new().with_device(device_dir.path().to_path_buf());

        let orchestrator = SyncOrchestrator::new();
        let request = SyncRequest::single("Nonexistent Playlist", device_dir.path());
        let options = SyncOptions {
            cleanup_enabled: false,
            ..Default::default()
        };

        let result = orchestrator.sync(
            &manager,
            &detector,
            request,
            &options,
            None::<fn(&SyncProgress)>,
        );

        // Should fail because playlist doesn't exist
        result.unwrap_err();
    }

    // -------------------------------------------------------------------------
    // Mock Device Detector Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_mock_device_detector_empty() {
        let detector = MockDeviceDetector::new();
        let devices = detector.list_devices().expect("should list");
        assert!(devices.is_empty());
        assert!(!detector.is_device_connected(Path::new("/any/path")));
    }

    #[test]
    fn test_mock_device_detector_with_multiple_devices() {
        let device_dir1 = TempDir::new().expect("create dir 1");
        let device_dir2 = TempDir::new().expect("create dir 2");

        let detector = MockDeviceDetector::new()
            .with_device(device_dir1.path().to_path_buf())
            .with_device(device_dir2.path().to_path_buf());

        let devices = detector.list_devices().expect("should list");
        assert_eq!(devices.len(), 2);
        assert!(detector.is_device_connected(device_dir1.path()));
        assert!(detector.is_device_connected(device_dir2.path()));
    }

    // -------------------------------------------------------------------------
    // Edge Case Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sync_result_summary_format() {
        let mut result = SyncResult::empty(1);
        result.total_files_transferred = 100;
        result.total_files_skipped = 5;
        result.total_files_failed = 0;
        result.total_bytes_transferred = 1_000_000_000; // 1 GB
        result.finalize(100.0); // 100 seconds = 10 MB/s

        let summary = result.summary();
        assert!(summary.contains("100 files transferred"));
        assert!(summary.contains("5 skipped"));
        assert!(summary.contains("0 failed"));
        // Check speed is formatted in MB/s
        assert!(summary.contains("MB/s"));
    }

    #[test]
    fn test_sync_progress_initial_state() {
        let progress = SyncProgress::verifying(10);

        assert_eq!(progress.phase, SyncPhase::Verifying);
        assert_eq!(progress.overall_progress_percent, 0.0);
        assert_eq!(progress.phase_progress_percent, 0.0);
        assert!(progress.current_playlist.is_none());
        assert_eq!(progress.current_playlist_index, 0);
        assert_eq!(progress.total_playlists, 10);
        assert!(progress.current_file.is_none());
        assert!(progress.cleanup_result.is_none());
        assert!(progress.transfer_progress.is_none());
        assert_eq!(progress.total_bytes, 0);
        assert_eq!(progress.bytes_transferred, 0);
        assert_eq!(progress.transfer_speed_bps, 0.0);
        assert!(progress.estimated_remaining_secs.is_none());
        assert_eq!(progress.elapsed_secs, 0.0);
        assert!(progress.message.contains("Verifying"));
    }

    #[test]
    fn test_sync_result_final_phase_on_success() {
        let mut result = SyncResult::empty(1);
        result.total_files_transferred = 5;
        result.finalize(1.0);

        assert!(result.success);
        assert_eq!(result.final_phase, SyncPhase::Completed);
    }

    #[test]
    fn test_sync_result_zero_duration() {
        let mut result = SyncResult::empty(1);
        result.total_bytes_transferred = 1000;
        result.finalize(0.0);

        // Should handle zero duration gracefully
        assert_eq!(result.average_speed_bps, 0.0);
    }
}
