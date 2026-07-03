use serde::{Deserialize, Serialize};

// =============================================================================
// Transfer Types
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

    /// Format transfer speed as a human-readable string.
    #[must_use]
    #[allow(clippy::float_arithmetic, reason = "acceptable for display formatting")]
    pub fn formatted_speed(&self) -> String {
        let speed = self.transfer_speed_bps;
        if speed >= 1_000_000_000.0 {
            format!("{:.1} GB/s", speed / 1_000_000_000.0)
        } else if speed >= 1_000_000.0 {
            format!("{:.1} MB/s", speed / 1_000_000.0)
        } else if speed >= 1_000.0 {
            format!("{:.1} KB/s", speed / 1_000.0)
        } else {
            format!("{speed:.0} B/s")
        }
    }

    /// Format estimated remaining time as a human-readable string.
    #[must_use]
    #[allow(clippy::float_arithmetic, reason = "acceptable for display formatting")]
    pub fn formatted_remaining_time(&self) -> String {
        match self.estimated_remaining_secs {
            Some(secs) if secs >= 3600.0 => {
                let hours = secs / 3600.0;
                format!("{hours:.1}h remaining")
            }
            Some(secs) if secs >= 60.0 => {
                let mins = secs / 60.0;
                format!("{mins:.1}m remaining")
            }
            Some(secs) => {
                format!("{secs:.0}s remaining")
            }
            None => "calculating...".to_string(),
        }
    }
}

/// Information about a single transferred file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferredFile {
    /// Source file path.
    pub source: String,
    /// Destination file path.
    pub destination: String,
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
    pub source: String,
    /// Intended destination path.
    pub destination: String,
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
    /// Format average speed as a human-readable string.
    #[must_use]
    #[allow(clippy::float_arithmetic, reason = "acceptable for display formatting")]
    pub fn formatted_average_speed(&self) -> String {
        let speed = self.average_speed_bps;
        if speed >= 1_000_000_000.0 {
            format!("{:.1} GB/s", speed / 1_000_000_000.0)
        } else if speed >= 1_000_000.0 {
            format!("{:.1} MB/s", speed / 1_000_000.0)
        } else if speed >= 1_000.0 {
            format!("{:.1} KB/s", speed / 1_000.0)
        } else {
            format!("{speed:.0} B/s")
        }
    }

    /// Format bytes transferred as a human-readable string.
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for display formatting"
    )]
    pub fn formatted_bytes_transferred(&self) -> String {
        let bytes = self.bytes_transferred;
        if bytes >= 1_000_000_000 {
            format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
        } else if bytes >= 1_000_000 {
            format!("{:.2} MB", bytes as f64 / 1_000_000.0)
        } else if bytes >= 1_000 {
            format!("{:.2} KB", bytes as f64 / 1_000.0)
        } else {
            format!("{bytes} bytes")
        }
    }
}

/// Configuration options for file transfers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool represents an independent transfer configuration option"
)]
pub struct TransferOptions {
    /// Size of chunks for reading/writing files (in bytes).
    pub chunk_size: usize,

    /// Whether to verify file integrity after transfer using checksums.
    pub verify_integrity: bool,

    /// Whether to skip files that already exist at the destination.
    pub skip_existing: bool,

    /// Whether to verify existing files by checksum.
    pub verify_existing_checksum: bool,

    /// Whether to preserve file timestamps during transfer.
    pub preserve_timestamps: bool,

    /// Whether to continue transferring other files if one fails.
    pub continue_on_error: bool,

    /// Maximum number of retry attempts for failed transfers.
    pub max_retries: u32,
}
