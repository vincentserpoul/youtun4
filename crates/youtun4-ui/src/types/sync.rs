use serde::{Deserialize, Serialize};

use super::task::TaskId;

// =============================================================================
// Sync Types
// =============================================================================

/// Status of a sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Sync is preparing (gathering files, etc.).
    Preparing,
    /// Sync is actively transferring files.
    Transferring,
    /// Sync is verifying file integrity.
    Verifying,
    /// Sync completed successfully.
    Completed,
    /// Sync failed.
    Failed,
    /// Sync was cancelled.
    Cancelled,
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparing => write!(f, "Preparing"),
            Self::Transferring => write!(f, "Transferring"),
            Self::Verifying => write!(f, "Verifying"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Progress information for a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    /// Task ID for this sync operation.
    pub task_id: TaskId,
    /// Current status of the sync.
    pub status: SyncStatus,
    /// Playlist name being synced.
    pub playlist_name: String,
    /// Device mount point.
    pub device_mount_point: String,
    /// Index of the current file being transferred (1-based).
    pub current_file_index: usize,
    /// Total number of files to transfer.
    pub total_files: usize,
    /// Name of the current file being transferred.
    pub current_file_name: String,
    /// Bytes transferred for the current file.
    pub current_file_bytes: u64,
    /// Total size of the current file in bytes.
    pub current_file_total: u64,
    /// Total bytes transferred across all files.
    pub total_bytes_transferred: u64,
    /// Total bytes to transfer across all files.
    pub total_bytes: u64,
    /// Number of files successfully transferred.
    pub files_completed: usize,
    /// Number of files skipped (already exist).
    pub files_skipped: usize,
    /// Number of files that failed to transfer.
    pub files_failed: usize,
    /// Transfer speed in bytes per second.
    pub transfer_speed_bps: f64,
    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,
    /// Elapsed time in seconds.
    pub elapsed_secs: f64,
}

impl SyncProgress {
    /// Calculate the overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for display formatting"
    )]
    pub fn overall_progress_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            if self.total_files == 0 {
                return 100.0;
            }
            let completed = self.files_completed + self.files_skipped;
            return (completed as f64 / self.total_files as f64) * 100.0;
        }
        (self.total_bytes_transferred as f64 / self.total_bytes as f64) * 100.0
    }

    /// Calculate the current file progress as a percentage (0.0 - 100.0).
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for display formatting"
    )]
    pub fn current_file_progress_percent(&self) -> f64 {
        if self.current_file_total == 0 {
            return 100.0;
        }
        (self.current_file_bytes as f64 / self.current_file_total as f64) * 100.0
    }
}

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// Task ID for this sync operation.
    pub task_id: TaskId,
    /// Whether the sync was successful.
    pub success: bool,
    /// Whether the sync was cancelled.
    pub was_cancelled: bool,
    /// Total number of files processed.
    pub total_files: usize,
    /// Number of files successfully transferred.
    pub files_transferred: usize,
    /// Number of files skipped (already existed).
    pub files_skipped: usize,
    /// Number of files that failed to transfer.
    pub files_failed: usize,
    /// Total bytes transferred.
    pub bytes_transferred: u64,
    /// Total duration of the sync operation in seconds.
    pub duration_secs: f64,
    /// Error message if the sync failed.
    pub error_message: Option<String>,
}
