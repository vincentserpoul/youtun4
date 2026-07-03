use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

// =============================================================================
// Constants
// =============================================================================

/// Default chunk size for file transfers (64 KB).
/// This provides a good balance between performance and progress granularity.
pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;

/// Minimum chunk size allowed (4 KB).
pub const MIN_CHUNK_SIZE: usize = 4 * 1024;

/// Maximum chunk size allowed (1 MB).
pub const MAX_CHUNK_SIZE: usize = 1024 * 1024;

/// Default progress update interval (100ms).
/// Progress callbacks won't be called more frequently than this.
pub const DEFAULT_PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

// =============================================================================
// Transfer Options
// =============================================================================

/// Configuration options for file transfers.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool represents an independent transfer option"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOptions {
    /// Size of chunks for reading/writing files (in bytes).
    /// Larger chunks are faster but provide less granular progress updates.
    /// Default: 64 KB
    pub chunk_size: usize,

    /// Whether to verify file integrity after transfer using checksums.
    /// This adds overhead but ensures data integrity.
    /// Default: true
    pub verify_integrity: bool,

    /// Whether to skip files that already exist at the destination
    /// with matching size (and optionally checksum).
    /// Default: true
    pub skip_existing: bool,

    /// Whether to verify existing files by checksum (slower but more accurate).
    /// Only applies if `skip_existing` is true.
    /// Default: false
    pub verify_existing_checksum: bool,

    /// Whether to preserve file timestamps during transfer.
    /// Default: true
    pub preserve_timestamps: bool,

    /// Minimum interval between progress callbacks.
    /// Default: 100ms
    pub progress_interval: Duration,

    /// Whether to continue transferring other files if one fails.
    /// Default: true
    pub continue_on_error: bool,

    /// Maximum number of retry attempts for failed transfers.
    /// Default: 3
    pub max_retries: u32,

    /// Delay between retry attempts.
    /// Default: 1 second
    pub retry_delay: Duration,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            verify_integrity: true,
            skip_existing: true,
            verify_existing_checksum: false,
            preserve_timestamps: true,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            continue_on_error: true,
            max_retries: 3,
            retry_delay: Duration::from_secs(1),
        }
    }
}

impl TransferOptions {
    /// Create options for fast transfers without verification.
    #[must_use]
    pub fn fast() -> Self {
        Self {
            verify_integrity: false,
            verify_existing_checksum: false,
            chunk_size: MAX_CHUNK_SIZE,
            ..Default::default()
        }
    }

    /// Create options for reliable transfers with full verification.
    #[must_use]
    pub fn reliable() -> Self {
        Self {
            verify_integrity: true,
            verify_existing_checksum: true,
            max_retries: 5,
            ..Default::default()
        }
    }

    /// Validate options and return an error if invalid.
    pub fn validate(&self) -> Result<()> {
        if self.chunk_size < MIN_CHUNK_SIZE {
            return Err(Error::Configuration(format!(
                "chunk_size must be at least {MIN_CHUNK_SIZE} bytes"
            )));
        }
        if self.chunk_size > MAX_CHUNK_SIZE {
            return Err(Error::Configuration(format!(
                "chunk_size must be at most {MAX_CHUNK_SIZE} bytes"
            )));
        }
        Ok(())
    }
}

// =============================================================================
// Transfer Progress
// =============================================================================

/// Status of a transfer operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    /// Transfer is preparing (calculating sizes, etc.).
    Preparing,
    /// Transfer is actively copying files.
    Transferring,
    /// Transfer is verifying file integrity.
    Verifying,
    /// Transfer completed successfully.
    Completed,
    /// Transfer failed.
    Failed,
    /// Transfer was cancelled.
    Cancelled,
}

impl std::fmt::Display for TransferStatus {
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

/// Progress information for a transfer operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    /// Current status of the transfer.
    pub status: TransferStatus,

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

    /// Transfer speed in bytes per second (rolling average).
    pub transfer_speed_bps: f64,

    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,

    /// Elapsed time in seconds.
    pub elapsed_secs: f64,
}

impl TransferProgress {
    /// Create a new progress instance for the preparation phase.
    #[must_use]
    pub const fn preparing(total_files: usize, total_bytes: u64) -> Self {
        Self {
            status: TransferStatus::Preparing,
            current_file_index: 0,
            total_files,
            current_file_name: String::new(),
            current_file_bytes: 0,
            current_file_total: 0,
            total_bytes_transferred: 0,
            total_bytes,
            files_completed: 0,
            files_skipped: 0,
            files_failed: 0,
            transfer_speed_bps: 0.0,
            estimated_remaining_secs: None,
            elapsed_secs: 0.0,
        }
    }

    /// Calculate the overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for progress percentage calculation"
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
        reason = "acceptable for progress percentage calculation"
    )]
    pub fn current_file_progress_percent(&self) -> f64 {
        if self.current_file_total == 0 {
            return 100.0;
        }
        (self.current_file_bytes as f64 / self.current_file_total as f64) * 100.0
    }
}

// =============================================================================
// Transfer Result
// =============================================================================

/// Information about a single transferred file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferredFile {
    /// Source file path.
    pub source: PathBuf,
    /// Destination file path.
    pub destination: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
    /// SHA-256 checksum (if verification was enabled).
    pub checksum: Option<String>,
    /// Transfer duration for this file.
    pub duration_secs: f64,
    /// Whether the file was skipped (already existed).
    pub skipped: bool,
}

/// Information about a failed transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedTransfer {
    /// Source file path.
    pub source: PathBuf,
    /// Intended destination path.
    pub destination: PathBuf,
    /// Error message.
    pub error: String,
    /// Number of retry attempts made.
    pub retry_count: u32,
}

/// Result of a transfer operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
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

    /// Total bytes skipped.
    pub bytes_skipped: u64,

    /// Total duration of the transfer operation.
    pub duration_secs: f64,

    /// Average transfer speed in bytes per second.
    pub average_speed_bps: f64,

    /// List of successfully transferred files.
    pub transferred_files: Vec<TransferredFile>,

    /// List of failed transfers.
    pub failed_transfers: Vec<FailedTransfer>,

    /// Whether the transfer was cancelled.
    pub was_cancelled: bool,

    /// Whether all files were transferred successfully.
    pub success: bool,
}

impl TransferResult {
    /// Create an empty result.
    pub(super) const fn empty() -> Self {
        Self {
            total_files: 0,
            files_transferred: 0,
            files_skipped: 0,
            files_failed: 0,
            bytes_transferred: 0,
            bytes_skipped: 0,
            duration_secs: 0.0,
            average_speed_bps: 0.0,
            transferred_files: Vec::new(),
            failed_transfers: Vec::new(),
            was_cancelled: false,
            success: true,
        }
    }
}

// =============================================================================
// File Transfer Item
// =============================================================================

/// Information about a file to be transferred.
#[derive(Debug, Clone)]
pub struct TransferItem {
    /// Source file path.
    pub source: PathBuf,
    /// Destination file path.
    pub destination: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in test code for brevity"
)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_options_default() {
        let opts = TransferOptions::default();
        assert_eq!(opts.chunk_size, DEFAULT_CHUNK_SIZE);
        assert!(opts.verify_integrity);
        assert!(opts.skip_existing);
        assert!(!opts.verify_existing_checksum);
        assert!(opts.preserve_timestamps);
        assert!(opts.continue_on_error);
        assert_eq!(opts.max_retries, 3);
    }

    #[test]
    fn test_transfer_options_fast() {
        let opts = TransferOptions::fast();
        assert_eq!(opts.chunk_size, MAX_CHUNK_SIZE);
        assert!(!opts.verify_integrity);
        assert!(!opts.verify_existing_checksum);
    }

    #[test]
    fn test_transfer_options_reliable() {
        let opts = TransferOptions::reliable();
        assert!(opts.verify_integrity);
        assert!(opts.verify_existing_checksum);
        assert_eq!(opts.max_retries, 5);
    }

    #[test]
    fn test_transfer_options_validate() {
        let mut opts = TransferOptions::default();
        opts.validate().unwrap();

        opts.chunk_size = MIN_CHUNK_SIZE - 1;
        assert!(opts.validate().is_err());

        opts.chunk_size = MAX_CHUNK_SIZE + 1;
        assert!(opts.validate().is_err());
    }

    #[test]
    fn test_transfer_progress_overall_percent() {
        let progress = TransferProgress {
            status: TransferStatus::Transferring,
            current_file_index: 1,
            total_files: 10,
            current_file_name: "test.mp3".to_string(),
            current_file_bytes: 0,
            current_file_total: 1000,
            total_bytes_transferred: 5000,
            total_bytes: 10000,
            files_completed: 5,
            files_skipped: 0,
            files_failed: 0,
            transfer_speed_bps: 1000.0,
            estimated_remaining_secs: Some(5.0),
            elapsed_secs: 5.0,
        };

        assert!((progress.overall_progress_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_transfer_progress_current_file_percent() {
        let progress = TransferProgress {
            status: TransferStatus::Transferring,
            current_file_index: 1,
            total_files: 1,
            current_file_name: "test.mp3".to_string(),
            current_file_bytes: 250,
            current_file_total: 1000,
            total_bytes_transferred: 250,
            total_bytes: 1000,
            files_completed: 0,
            files_skipped: 0,
            files_failed: 0,
            transfer_speed_bps: 1000.0,
            estimated_remaining_secs: Some(0.75),
            elapsed_secs: 0.25,
        };

        assert!((progress.current_file_progress_percent() - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_transfer_status_display() {
        assert_eq!(TransferStatus::Preparing.to_string(), "Preparing");
        assert_eq!(TransferStatus::Transferring.to_string(), "Transferring");
        assert_eq!(TransferStatus::Verifying.to_string(), "Verifying");
        assert_eq!(TransferStatus::Completed.to_string(), "Completed");
        assert_eq!(TransferStatus::Failed.to_string(), "Failed");
        assert_eq!(TransferStatus::Cancelled.to_string(), "Cancelled");
    }

    #[test]
    fn test_transfer_progress_percentage_zero_bytes() {
        let progress = TransferProgress::preparing(5, 0);

        // With zero total bytes but files, should calculate by file count
        let mut progress = progress;
        progress.files_completed = 2;
        progress.files_skipped = 1;
        progress.total_files = 5;

        assert!((progress.overall_progress_percent() - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_transfer_progress_current_file_zero_total() {
        let progress = TransferProgress {
            status: TransferStatus::Transferring,
            current_file_index: 1,
            total_files: 1,
            current_file_name: "empty.mp3".to_string(),
            current_file_bytes: 0,
            current_file_total: 0,
            total_bytes_transferred: 0,
            total_bytes: 0,
            files_completed: 0,
            files_skipped: 0,
            files_failed: 0,
            transfer_speed_bps: 0.0,
            estimated_remaining_secs: None,
            elapsed_secs: 0.0,
        };

        // Zero-byte file should show 100%
        assert!((progress.current_file_progress_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_transfer_result_empty() {
        let result = TransferResult::empty();

        assert_eq!(result.total_files, 0);
        assert_eq!(result.files_transferred, 0);
        assert_eq!(result.files_skipped, 0);
        assert_eq!(result.files_failed, 0);
        assert_eq!(result.bytes_transferred, 0);
        assert!(!result.was_cancelled);
        assert!(result.success);
    }

    #[test]
    fn test_transferred_file_struct() {
        let transferred = TransferredFile {
            source: PathBuf::from("/source/file.mp3"),
            destination: PathBuf::from("/dest/file.mp3"),
            size_bytes: 1000,
            checksum: Some("abc123".to_string()),
            duration_secs: 0.5,
            skipped: false,
        };

        let json = serde_json::to_string(&transferred).expect("serialize");
        let deserialized: TransferredFile = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(transferred.source, deserialized.source);
        assert_eq!(transferred.size_bytes, deserialized.size_bytes);
        assert_eq!(transferred.checksum, deserialized.checksum);
    }

    #[test]
    fn test_failed_transfer_struct() {
        let failed = FailedTransfer {
            source: PathBuf::from("/source/file.mp3"),
            destination: PathBuf::from("/dest/file.mp3"),
            error: "Permission denied".to_string(),
            retry_count: 3,
        };

        let json = serde_json::to_string(&failed).expect("serialize");
        let deserialized: FailedTransfer = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(failed.source, deserialized.source);
        assert_eq!(failed.error, deserialized.error);
        assert_eq!(failed.retry_count, deserialized.retry_count);
    }

    #[test]
    fn test_transfer_item_struct() {
        let item = TransferItem {
            source: PathBuf::from("/source/file.mp3"),
            destination: PathBuf::from("/dest/file.mp3"),
            size_bytes: 5000,
        };

        assert_eq!(item.source, PathBuf::from("/source/file.mp3"));
        assert_eq!(item.destination, PathBuf::from("/dest/file.mp3"));
        assert_eq!(item.size_bytes, 5000);
    }

    #[test]
    fn test_transfer_constants() {
        assert_eq!(DEFAULT_CHUNK_SIZE, 64 * 1024);
        assert_eq!(MIN_CHUNK_SIZE, 4 * 1024);
        assert_eq!(MAX_CHUNK_SIZE, 1024 * 1024);
        assert_eq!(DEFAULT_PROGRESS_INTERVAL, Duration::from_millis(100));
    }
}
