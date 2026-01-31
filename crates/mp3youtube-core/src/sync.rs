//! Sync orchestrator for coordinating device cleanup and file transfer.
//!
//! This module provides the main synchronization logic that coordinates:
//! - Device verification (checking connection and available space)
//! - Device cleanup (deleting old content)
//! - File transfer (copying selected playlists to the device)
//!
//! The orchestrator handles the complete workflow with progress tracking,
//! cancellation support, and error recovery.
//!
//! # Example
//!
//! ```rust,ignore
//! use mp3youtube_core::sync::{SyncOrchestrator, SyncOptions, SyncRequest};
//! use std::path::PathBuf;
//!
//! let orchestrator = SyncOrchestrator::new();
//! let request = SyncRequest {
//!     playlists: vec!["My Playlist".to_string()],
//!     device_mount_point: PathBuf::from("/Volumes/MP3Player"),
//! };
//!
//! let result = orchestrator.sync(
//!     &playlist_manager,
//!     &device_manager,
//!     request,
//!     &SyncOptions::default(),
//!     Some(|progress| println!("Progress: {:?}", progress)),
//! )?;
//!
//! println!("Synced {} files", result.files_transferred);
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::cleanup::{CleanupOptions, CleanupResult, DeviceCleanupHandler};
use crate::device::DeviceDetector;
use crate::error::{DeviceError, Error, Result};
use crate::playlist::PlaylistManager;
use crate::transfer::{
    TransferEngine, TransferOptions, TransferProgress, TransferResult, TransferStatus,
};

// =============================================================================
// Sync Phase Definitions
// =============================================================================

/// Current phase of the synchronization process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPhase {
    /// Verifying device connection and space.
    Verifying,
    /// Cleaning up old content from device.
    Cleaning,
    /// Transferring files to device.
    Transferring,
    /// Synchronization completed.
    Completed,
    /// Synchronization failed.
    Failed,
    /// Synchronization was cancelled.
    Cancelled,
}

impl std::fmt::Display for SyncPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Verifying => write!(f, "Verifying"),
            Self::Cleaning => write!(f, "Cleaning"),
            Self::Transferring => write!(f, "Transferring"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

// =============================================================================
// Sync Options
// =============================================================================

/// Configuration options for the sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOptions {
    /// Whether to perform device cleanup before transfer.
    pub cleanup_enabled: bool,

    /// Cleanup options for the device cleanup phase.
    pub cleanup_options: CleanupOptions,

    /// Transfer options for the file transfer phase.
    pub transfer_options: TransferOptions,

    /// Whether to verify device is still connected before each phase.
    pub verify_device_between_phases: bool,

    /// Whether to abort the entire sync if cleanup fails.
    pub abort_on_cleanup_failure: bool,

    /// Whether to preserve existing files on device that match source files.
    pub skip_existing_matches: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            cleanup_enabled: true,
            cleanup_options: CleanupOptions::default(),
            transfer_options: TransferOptions::default(),
            verify_device_between_phases: true,
            abort_on_cleanup_failure: true,
            skip_existing_matches: true,
        }
    }
}

impl SyncOptions {
    /// Create options optimized for speed.
    #[must_use]
    pub fn fast() -> Self {
        Self {
            cleanup_enabled: true,
            cleanup_options: CleanupOptions::full_cleanup(),
            transfer_options: TransferOptions::fast(),
            verify_device_between_phases: false,
            abort_on_cleanup_failure: true,
            skip_existing_matches: true,
        }
    }

    /// Create options optimized for reliability.
    #[must_use]
    pub fn reliable() -> Self {
        Self {
            cleanup_enabled: true,
            cleanup_options: CleanupOptions::default(),
            transfer_options: TransferOptions::reliable(),
            verify_device_between_phases: true,
            abort_on_cleanup_failure: true,
            skip_existing_matches: false, // Re-transfer everything for verification
        }
    }

    /// Create options for a dry run (preview only).
    #[must_use]
    pub fn dry_run() -> Self {
        Self {
            cleanup_enabled: true,
            cleanup_options: CleanupOptions::dry_run(),
            transfer_options: TransferOptions::default(),
            verify_device_between_phases: false,
            abort_on_cleanup_failure: false,
            skip_existing_matches: true,
        }
    }

    /// Set whether to enable cleanup.
    #[must_use]
    pub const fn with_cleanup(mut self, enabled: bool) -> Self {
        self.cleanup_enabled = enabled;
        self
    }

    /// Set the cleanup options.
    #[must_use]
    pub fn with_cleanup_options(mut self, options: CleanupOptions) -> Self {
        self.cleanup_options = options;
        self
    }

    /// Set the transfer options.
    #[must_use]
    pub const fn with_transfer_options(mut self, options: TransferOptions) -> Self {
        self.transfer_options = options;
        self
    }
}

// =============================================================================
// Sync Request
// =============================================================================

/// Request to synchronize playlists to a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Names of playlists to sync.
    pub playlists: Vec<String>,

    /// Mount point of the target device.
    pub device_mount_point: PathBuf,
}

impl SyncRequest {
    /// Create a new sync request.
    #[must_use]
    pub const fn new(playlists: Vec<String>, device_mount_point: PathBuf) -> Self {
        Self {
            playlists,
            device_mount_point,
        }
    }

    /// Create a sync request for a single playlist.
    pub fn single(playlist: impl Into<String>, device_mount_point: impl Into<PathBuf>) -> Self {
        Self {
            playlists: vec![playlist.into()],
            device_mount_point: device_mount_point.into(),
        }
    }
}

// =============================================================================
// Sync Progress
// =============================================================================

/// Progress information for the sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    /// Current phase of the sync operation.
    pub phase: SyncPhase,

    /// Overall progress percentage (0.0 - 100.0).
    pub overall_progress_percent: f64,

    /// Current phase progress percentage (0.0 - 100.0).
    pub phase_progress_percent: f64,

    /// Name of the playlist currently being processed.
    pub current_playlist: Option<String>,

    /// Index of the current playlist (1-based).
    pub current_playlist_index: usize,

    /// Total number of playlists to sync.
    pub total_playlists: usize,

    /// Name of the current file being processed.
    pub current_file: Option<String>,

    /// Cleanup result (if cleanup phase is complete).
    pub cleanup_result: Option<CleanupResult>,

    /// Transfer progress (if in transfer phase).
    pub transfer_progress: Option<TransferProgress>,

    /// Total bytes to transfer.
    pub total_bytes: u64,

    /// Bytes transferred so far.
    pub bytes_transferred: u64,

    /// Transfer speed in bytes per second.
    pub transfer_speed_bps: f64,

    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,

    /// Elapsed time in seconds.
    pub elapsed_secs: f64,

    /// Status message.
    pub message: String,
}

impl SyncProgress {
    /// Create a new progress instance for the verifying phase.
    #[must_use]
    pub fn verifying(total_playlists: usize) -> Self {
        Self {
            phase: SyncPhase::Verifying,
            overall_progress_percent: 0.0,
            phase_progress_percent: 0.0,
            current_playlist: None,
            current_playlist_index: 0,
            total_playlists,
            current_file: None,
            cleanup_result: None,
            transfer_progress: None,
            total_bytes: 0,
            bytes_transferred: 0,
            transfer_speed_bps: 0.0,
            estimated_remaining_secs: None,
            elapsed_secs: 0.0,
            message: "Verifying device...".to_string(),
        }
    }

    /// Update progress for the cleaning phase.
    pub fn cleaning(&mut self, message: impl Into<String>) {
        self.phase = SyncPhase::Cleaning;
        self.phase_progress_percent = 0.0;
        self.message = message.into();
    }

    /// Update progress for the transferring phase.
    pub fn transferring(&mut self, playlist: impl Into<String>, index: usize) {
        self.phase = SyncPhase::Transferring;
        self.current_playlist = Some(playlist.into());
        self.current_playlist_index = index;
        self.message = format!(
            "Transferring playlist {}/{}...",
            index, self.total_playlists
        );
    }

    /// Update with transfer progress.
    pub fn update_transfer_progress(&mut self, progress: &TransferProgress, playlist_weight: f64) {
        self.transfer_progress = Some(progress.clone());
        self.current_file = Some(progress.current_file_name.clone());
        self.bytes_transferred = progress.total_bytes_transferred;
        self.transfer_speed_bps = progress.transfer_speed_bps;
        self.estimated_remaining_secs = progress.estimated_remaining_secs;
        self.elapsed_secs = progress.elapsed_secs;

        // Calculate phase progress
        self.phase_progress_percent = progress.overall_progress_percent();

        // Calculate overall progress (cleanup is 10%, transfer is 90%)
        let cleanup_weight = 0.1;
        let transfer_weight = 0.9;

        let playlist_progress = self.phase_progress_percent / 100.0;
        let playlists_done = (self.current_playlist_index - 1) as f64;
        let total = self.total_playlists as f64;

        self.overall_progress_percent = cleanup_weight * 100.0
            + transfer_weight
                * ((playlists_done + playlist_progress * playlist_weight) / total)
                * 100.0;
    }

    /// Mark as completed.
    pub fn completed(&mut self, duration_secs: f64) {
        self.phase = SyncPhase::Completed;
        self.overall_progress_percent = 100.0;
        self.phase_progress_percent = 100.0;
        self.elapsed_secs = duration_secs;
        self.estimated_remaining_secs = Some(0.0);
        self.message = "Sync completed successfully".to_string();
    }

    /// Mark as failed.
    pub fn failed(&mut self, error_message: impl Into<String>) {
        self.phase = SyncPhase::Failed;
        self.message = error_message.into();
    }

    /// Mark as cancelled.
    pub fn cancelled(&mut self) {
        self.phase = SyncPhase::Cancelled;
        self.message = "Sync was cancelled".to_string();
    }
}

// =============================================================================
// Sync Result
// =============================================================================

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// Whether the sync completed successfully.
    pub success: bool,

    /// Whether the sync was cancelled.
    pub was_cancelled: bool,

    /// Final phase reached.
    pub final_phase: SyncPhase,

    /// Cleanup result (if cleanup was performed).
    pub cleanup_result: Option<CleanupResult>,

    /// Transfer results for each playlist.
    pub transfer_results: Vec<PlaylistTransferResult>,

    /// Total files transferred across all playlists.
    pub total_files_transferred: usize,

    /// Total files skipped across all playlists.
    pub total_files_skipped: usize,

    /// Total files failed across all playlists.
    pub total_files_failed: usize,

    /// Total bytes transferred.
    pub total_bytes_transferred: u64,

    /// Total duration of the sync operation.
    pub duration_secs: f64,

    /// Average transfer speed in bytes per second.
    pub average_speed_bps: f64,

    /// Error message if the sync failed.
    pub error_message: Option<String>,
}

/// Result of transferring a single playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTransferResult {
    /// Name of the playlist.
    pub playlist_name: String,

    /// Transfer result for this playlist.
    pub transfer_result: TransferResult,
}

impl SyncResult {
    /// Create an empty result.
    fn empty(total_playlists: usize) -> Self {
        Self {
            success: false,
            was_cancelled: false,
            final_phase: SyncPhase::Verifying,
            cleanup_result: None,
            transfer_results: Vec::with_capacity(total_playlists),
            total_files_transferred: 0,
            total_files_skipped: 0,
            total_files_failed: 0,
            total_bytes_transferred: 0,
            duration_secs: 0.0,
            average_speed_bps: 0.0,
            error_message: None,
        }
    }

    /// Add a transfer result for a playlist.
    fn add_transfer_result(&mut self, playlist_name: String, result: TransferResult) {
        self.total_files_transferred += result.files_transferred;
        self.total_files_skipped += result.files_skipped;
        self.total_files_failed += result.files_failed;
        self.total_bytes_transferred += result.bytes_transferred;

        self.transfer_results.push(PlaylistTransferResult {
            playlist_name,
            transfer_result: result,
        });
    }

    /// Finalize the result.
    fn finalize(&mut self, duration_secs: f64) {
        self.duration_secs = duration_secs;
        self.average_speed_bps = if duration_secs > 0.0 {
            self.total_bytes_transferred as f64 / duration_secs
        } else {
            0.0
        };

        self.success =
            self.total_files_failed == 0 && !self.was_cancelled && self.error_message.is_none();

        if self.success {
            self.final_phase = SyncPhase::Completed;
        }
    }

    /// Get a summary of the sync result.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.was_cancelled {
            format!(
                "Sync cancelled: {} files transferred before cancellation",
                self.total_files_transferred
            )
        } else if let Some(ref error) = self.error_message {
            format!("Sync failed: {error}")
        } else {
            format!(
                "Sync completed: {} files transferred, {} skipped, {} failed in {:.2}s ({:.2} MB/s)",
                self.total_files_transferred,
                self.total_files_skipped,
                self.total_files_failed,
                self.duration_secs,
                self.average_speed_bps / (1024.0 * 1024.0)
            )
        }
    }
}

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
    #[allow(clippy::too_many_lines)]
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

        // Send initial progress
        if let Some(cb) = &progress_callback {
            cb(&progress);
        }

        // Check for cancellation
        if self.is_cancelled() {
            progress.cancelled();
            result.was_cancelled = true;
            result.final_phase = SyncPhase::Cancelled;
            if let Some(cb) = &progress_callback {
                cb(&progress);
            }
            return Ok(result);
        }

        // Phase 1: Verify device
        info!("Phase 1: Verifying device...");
        if let Err(e) = self.verify_device(device_detector, &request.device_mount_point) {
            error!("Device verification failed: {}", e);
            progress.failed(format!("Device verification failed: {e}"));
            result.error_message = Some(e.to_string());
            result.final_phase = SyncPhase::Failed;
            if let Some(cb) = &progress_callback {
                cb(&progress);
            }
            return Err(e);
        }

        // Calculate total bytes to transfer
        let total_bytes = self.calculate_total_bytes(playlist_manager, &request.playlists)?;
        progress.total_bytes = total_bytes;

        // Verify device has enough space
        self.verify_device_space(&request.device_mount_point, total_bytes)?;

        // Check for cancellation
        if self.is_cancelled() {
            progress.cancelled();
            result.was_cancelled = true;
            result.final_phase = SyncPhase::Cancelled;
            if let Some(cb) = &progress_callback {
                cb(&progress);
            }
            return Ok(result);
        }

        // Phase 2: Cleanup (if enabled)
        if options.cleanup_enabled {
            info!("Phase 2: Cleaning device...");
            progress.cleaning("Cleaning device contents...");
            progress.overall_progress_percent = 5.0;
            if let Some(cb) = &progress_callback {
                cb(&progress);
            }

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
                        progress.failed(format!("Cleanup failed: {e}"));
                        result.error_message = Some(e.to_string());
                        result.final_phase = SyncPhase::Failed;
                        if let Some(cb) = &progress_callback {
                            cb(&progress);
                        }
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
                progress.failed(format!("Device disconnected: {e}"));
                result.error_message = Some(e.to_string());
                result.final_phase = SyncPhase::Failed;
                if let Some(cb) = &progress_callback {
                    cb(&progress);
                }
                return Err(e);
            }
        }

        // Check for cancellation
        if self.is_cancelled() {
            progress.cancelled();
            result.was_cancelled = true;
            result.final_phase = SyncPhase::Cancelled;
            if let Some(cb) = &progress_callback {
                cb(&progress);
            }
            return Ok(result);
        }

        // Phase 3: Transfer playlists
        info!("Phase 3: Transferring playlists...");
        let mut transfer_engine = TransferEngine::with_cancellation(Arc::clone(&self.cancelled));

        for (index, playlist_name) in request.playlists.iter().enumerate() {
            let playlist_index = index + 1;

            // Check for cancellation
            if self.is_cancelled() {
                progress.cancelled();
                result.was_cancelled = true;
                result.final_phase = SyncPhase::Cancelled;
                if let Some(cb) = &progress_callback {
                    cb(&progress);
                }
                return Ok(result);
            }

            // Verify device before each playlist (if enabled)
            if options.verify_device_between_phases
                && index > 0
                && let Err(e) = self.verify_device(device_detector, &request.device_mount_point)
            {
                error!("Device disconnected during transfer: {}", e);
                progress.failed(format!("Device disconnected: {e}"));
                result.error_message = Some(e.to_string());
                result.final_phase = SyncPhase::Failed;
                if let Some(cb) = &progress_callback {
                    cb(&progress);
                }
                return Err(e);
            }

            info!(
                "Transferring playlist {}/{}: {}",
                playlist_index, total_playlists, playlist_name
            );
            progress.transferring(playlist_name.clone(), playlist_index);
            if let Some(cb) = &progress_callback {
                cb(&progress);
            }

            // Get playlist path
            let playlist_path = playlist_manager.get_playlist_path(playlist_name)?;

            // Calculate weight for this playlist in overall progress
            let playlist_weight = if total_playlists > 0 {
                1.0 / total_playlists as f64
            } else {
                1.0
            };

            // Perform transfer (without nested callback to avoid borrow issues)
            // We'll update progress after the transfer completes
            let transfer_result = transfer_engine.transfer_playlist(
                &playlist_path,
                &request.device_mount_point,
                &options.transfer_options,
                None::<fn(&TransferProgress)>,
            );

            match transfer_result {
                Ok(transfer_result) => {
                    info!(
                        "Playlist '{}' transferred: {} files, {} bytes",
                        playlist_name,
                        transfer_result.files_transferred,
                        transfer_result.bytes_transferred
                    );

                    // Update progress after transfer completes
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
                        total_bytes: transfer_result.bytes_transferred
                            + transfer_result.bytes_skipped,
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
                        result.add_transfer_result(playlist_name.clone(), transfer_result);
                        result.final_phase = SyncPhase::Cancelled;
                        if let Some(cb) = &progress_callback {
                            cb(&progress);
                        }
                        return Ok(result);
                    }

                    result.add_transfer_result(playlist_name.clone(), transfer_result);

                    // Update progress callback
                    if let Some(cb) = &progress_callback {
                        cb(&progress);
                    }
                }
                Err(e) => {
                    if matches!(e, Error::Cancelled) {
                        progress.cancelled();
                        result.was_cancelled = true;
                        result.final_phase = SyncPhase::Cancelled;
                        if let Some(cb) = &progress_callback {
                            cb(&progress);
                        }
                        return Ok(result);
                    }

                    error!("Failed to transfer playlist '{}': {}", playlist_name, e);
                    progress.failed(format!("Transfer failed for '{playlist_name}': {e}"));
                    result.error_message = Some(e.to_string());
                    result.final_phase = SyncPhase::Failed;
                    if let Some(cb) = &progress_callback {
                        cb(&progress);
                    }
                    return Err(e);
                }
            }
        }

        // Finalize result
        let duration_secs = start_time.elapsed().as_secs_f64();
        result.finalize(duration_secs);
        progress.completed(duration_secs);

        info!("{}", result.summary());

        if let Some(cb) = &progress_callback {
            cb(&progress);
        }

        Ok(result)
    }

    /// Verify that the device is connected and accessible.
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
mod tests {
    use super::*;
    use crate::device::DeviceInfo;
    use std::fs;
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

        assert!(result.is_err());
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
        let mut options = SyncOptions::default();
        options.cleanup_enabled = false; // Skip cleanup for this test

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
        let mut options = SyncOptions::default();
        options.cleanup_enabled = false;

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

    // =============================================================================
    // Additional Sync Tests - Edge Cases and Complete Coverage
    // =============================================================================

    // -------------------------------------------------------------------------
    // SyncOptions Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sync_options_dry_run() {
        let options = SyncOptions::dry_run();
        assert!(options.cleanup_options.dry_run);
        assert!(!options.abort_on_cleanup_failure); // Should continue even if cleanup "fails" in dry run
    }

    #[test]
    fn test_sync_options_with_cleanup() {
        let options = SyncOptions::default().with_cleanup(false);
        assert!(!options.cleanup_enabled);

        let options_enabled = options.with_cleanup(true);
        assert!(options_enabled.cleanup_enabled);
    }

    #[test]
    fn test_sync_options_with_cleanup_options() {
        let cleanup_opts = CleanupOptions::dry_run();
        let options = SyncOptions::default().with_cleanup_options(cleanup_opts);
        assert!(options.cleanup_options.dry_run);
    }

    #[test]
    fn test_sync_options_with_transfer_options() {
        let transfer_opts = TransferOptions::fast();
        let options = SyncOptions::default().with_transfer_options(transfer_opts);
        assert!(!options.transfer_options.verify_integrity);
    }

    #[test]
    fn test_sync_options_serialization() {
        let options = SyncOptions::default();
        let json = serde_json::to_string(&options).expect("serialize");
        let deserialized: SyncOptions = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(options.cleanup_enabled, deserialized.cleanup_enabled);
        assert_eq!(
            options.verify_device_between_phases,
            deserialized.verify_device_between_phases
        );
    }

    // -------------------------------------------------------------------------
    // SyncRequest Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sync_request_new() {
        let request = SyncRequest::new(
            vec!["Playlist1".to_string(), "Playlist2".to_string()],
            PathBuf::from("/mnt/usb"),
        );
        assert_eq!(request.playlists.len(), 2);
        assert_eq!(request.device_mount_point, PathBuf::from("/mnt/usb"));
    }

    #[test]
    fn test_sync_request_serialization() {
        let request = SyncRequest::single("My Playlist", "/mnt/device");
        let json = serde_json::to_string(&request).expect("serialize");
        let deserialized: SyncRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(request.playlists, deserialized.playlists);
        assert_eq!(request.device_mount_point, deserialized.device_mount_point);
    }

    // -------------------------------------------------------------------------
    // SyncProgress Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sync_progress_transferring() {
        let mut progress = SyncProgress::verifying(5);
        progress.transferring("Test Playlist", 2);

        assert_eq!(progress.phase, SyncPhase::Transferring);
        assert_eq!(progress.current_playlist, Some("Test Playlist".to_string()));
        assert_eq!(progress.current_playlist_index, 2);
        assert!(progress.message.contains("2/5"));
    }

    #[test]
    fn test_sync_progress_failed() {
        let mut progress = SyncProgress::verifying(1);
        progress.failed("Something went wrong");

        assert_eq!(progress.phase, SyncPhase::Failed);
        assert_eq!(progress.message, "Something went wrong");
    }

    #[test]
    fn test_sync_progress_cancelled() {
        let mut progress = SyncProgress::verifying(1);
        progress.cancelled();

        assert_eq!(progress.phase, SyncPhase::Cancelled);
        assert!(progress.message.contains("cancelled"));
    }

    #[test]
    fn test_sync_progress_serialization() {
        let progress = SyncProgress::verifying(3);
        let json = serde_json::to_string(&progress).expect("serialize");
        let deserialized: SyncProgress = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(progress.phase, deserialized.phase);
        assert_eq!(progress.total_playlists, deserialized.total_playlists);
    }

    #[test]
    fn test_sync_progress_update_transfer_progress() {
        let mut progress = SyncProgress::verifying(2);
        progress.transferring("Playlist 1", 1);
        progress.total_bytes = 10000;

        let transfer_progress = TransferProgress {
            status: TransferStatus::Transferring,
            current_file_index: 2,
            total_files: 5,
            current_file_name: "song.mp3".to_string(),
            current_file_bytes: 500,
            current_file_total: 1000,
            total_bytes_transferred: 5000,
            total_bytes: 10000,
            files_completed: 1,
            files_skipped: 0,
            files_failed: 0,
            transfer_speed_bps: 1000.0,
            estimated_remaining_secs: Some(5.0),
            elapsed_secs: 5.0,
        };

        progress.update_transfer_progress(&transfer_progress, 0.5);

        assert_eq!(progress.current_file, Some("song.mp3".to_string()));
        assert_eq!(progress.bytes_transferred, 5000);
        assert_eq!(progress.transfer_speed_bps, 1000.0);
        assert!(progress.overall_progress_percent > 0.0);
    }

    // -------------------------------------------------------------------------
    // SyncResult Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sync_result_summary_with_error() {
        let mut result = SyncResult::empty(1);
        result.error_message = Some("Device disconnected".to_string());
        result.finalize(5.0);

        let summary = result.summary();
        assert!(summary.contains("failed"));
        assert!(summary.contains("Device disconnected"));
    }

    #[test]
    fn test_sync_result_success_determination() {
        let mut result = SyncResult::empty(1);
        result.total_files_transferred = 10;
        result.finalize(5.0);
        assert!(result.success);

        let mut result_failed = SyncResult::empty(1);
        result_failed.total_files_failed = 1;
        result_failed.finalize(5.0);
        assert!(!result_failed.success);

        let mut result_cancelled = SyncResult::empty(1);
        result_cancelled.was_cancelled = true;
        result_cancelled.finalize(5.0);
        assert!(!result_cancelled.success);

        let mut result_error = SyncResult::empty(1);
        result_error.error_message = Some("test error".to_string());
        result_error.finalize(5.0);
        assert!(!result_error.success);
    }

    #[test]
    fn test_sync_result_add_transfer_result() {
        let mut result = SyncResult::empty(2);

        let transfer1 = TransferResult {
            total_files: 5,
            files_transferred: 4,
            files_skipped: 1,
            files_failed: 0,
            bytes_transferred: 1000,
            bytes_skipped: 100,
            duration_secs: 1.0,
            average_speed_bps: 1000.0,
            transferred_files: Vec::new(),
            failed_transfers: Vec::new(),
            was_cancelled: false,
            success: true,
        };

        result.add_transfer_result("Playlist1".to_string(), transfer1);

        assert_eq!(result.total_files_transferred, 4);
        assert_eq!(result.total_files_skipped, 1);
        assert_eq!(result.total_bytes_transferred, 1000);
        assert_eq!(result.transfer_results.len(), 1);
        assert_eq!(result.transfer_results[0].playlist_name, "Playlist1");
    }

    #[test]
    fn test_sync_result_average_speed_calculation() {
        let mut result = SyncResult::empty(1);
        result.total_bytes_transferred = 10_000_000; // 10 MB
        result.finalize(10.0); // 10 seconds

        assert_eq!(result.average_speed_bps, 1_000_000.0); // 1 MB/s
    }

    #[test]
    fn test_sync_result_serialization() {
        let mut result = SyncResult::empty(1);
        result.total_files_transferred = 10;
        result.finalize(5.0);

        let json = serde_json::to_string(&result).expect("serialize");
        let deserialized: SyncResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            result.total_files_transferred,
            deserialized.total_files_transferred
        );
        assert_eq!(result.success, deserialized.success);
    }

    // -------------------------------------------------------------------------
    // PlaylistTransferResult Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_playlist_transfer_result_serialization() {
        let transfer = TransferResult {
            total_files: 5,
            files_transferred: 5,
            files_skipped: 0,
            files_failed: 0,
            bytes_transferred: 5000,
            bytes_skipped: 0,
            duration_secs: 2.0,
            average_speed_bps: 2500.0,
            transferred_files: Vec::new(),
            failed_transfers: Vec::new(),
            was_cancelled: false,
            success: true,
        };

        let playlist_result = PlaylistTransferResult {
            playlist_name: "Test Playlist".to_string(),
            transfer_result: transfer,
        };

        let json = serde_json::to_string(&playlist_result).expect("serialize");
        let deserialized: PlaylistTransferResult =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(playlist_result.playlist_name, deserialized.playlist_name);
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
        let mut options = SyncOptions::default();
        options.cleanup_enabled = false;

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
        let mut options = SyncOptions::default();
        options.cleanup_enabled = false;

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
        let mut options = SyncOptions::default();
        options.cleanup_enabled = false;

        let result = orchestrator.sync(
            &manager,
            &detector,
            request,
            &options,
            None::<fn(&SyncProgress)>,
        );

        // Should fail because playlist doesn't exist
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // SyncPhase Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sync_phase_equality() {
        assert_eq!(SyncPhase::Verifying, SyncPhase::Verifying);
        assert_ne!(SyncPhase::Verifying, SyncPhase::Cleaning);
        assert_ne!(SyncPhase::Completed, SyncPhase::Failed);
    }

    #[test]
    fn test_sync_phase_serialization() {
        let phases = vec![
            SyncPhase::Verifying,
            SyncPhase::Cleaning,
            SyncPhase::Transferring,
            SyncPhase::Completed,
            SyncPhase::Failed,
            SyncPhase::Cancelled,
        ];

        for phase in phases {
            let json = serde_json::to_string(&phase).expect("serialize");
            let deserialized: SyncPhase = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(phase, deserialized);
        }
    }

    #[test]
    fn test_sync_phase_copy() {
        let phase = SyncPhase::Transferring;
        let copied = phase;
        assert_eq!(phase, copied);
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
