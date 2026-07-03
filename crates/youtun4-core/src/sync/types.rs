use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cleanup::{CleanupOptions, CleanupResult};
use crate::transfer::{TransferOptions, TransferProgress, TransferResult};

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
#[allow(
    clippy::struct_excessive_bools,
    reason = "these are independent configuration flags, not a state machine"
)]
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

        #[allow(
            clippy::float_arithmetic,
            clippy::cast_precision_loss,
            reason = "progress calculation requires floating-point math; precision loss is acceptable for progress percentages"
        )]
        {
            let playlist_progress = self.phase_progress_percent / 100.0;
            let playlists_done = (self.current_playlist_index - 1) as f64;
            let total = self.total_playlists as f64;

            self.overall_progress_percent = cleanup_weight * 100.0
                + transfer_weight
                    * ((playlists_done + playlist_progress * playlist_weight) / total)
                    * 100.0;
        }
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
    pub(super) fn empty(total_playlists: usize) -> Self {
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
    pub(super) fn add_transfer_result(&mut self, playlist_name: String, result: TransferResult) {
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
    pub(super) fn finalize(&mut self, duration_secs: f64) {
        self.duration_secs = duration_secs;
        #[allow(
            clippy::cast_precision_loss,
            clippy::float_arithmetic,
            reason = "precision loss acceptable for speed calculation"
        )]
        let speed = if duration_secs > 0.0 {
            self.total_bytes_transferred as f64 / duration_secs
        } else {
            0.0
        };
        self.average_speed_bps = speed;

        self.success =
            self.total_files_failed == 0 && !self.was_cancelled && self.error_message.is_none();

        if self.success {
            self.final_phase = SyncPhase::Completed;
        }
    }

    /// Get a summary of the sync result.
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        reason = "unit conversion for display formatting"
    )]
    pub fn summary(&self) -> String {
        if self.was_cancelled {
            format!(
                "Sync cancelled: {} files transferred before cancellation",
                self.total_files_transferred
            )
        } else if let Some(ref error) = self.error_message {
            format!("Sync failed: {error}")
        } else {
            let speed_mbps = self.average_speed_bps / (1024.0 * 1024.0);
            format!(
                "Sync completed: {} files transferred, {} skipped, {} failed in {:.2}s ({:.2} MB/s)",
                self.total_files_transferred,
                self.total_files_skipped,
                self.total_files_failed,
                self.duration_secs,
                speed_mbps
            )
        }
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
        use crate::transfer::TransferStatus;

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
}
