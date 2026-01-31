//! File transfer engine for efficiently copying MP3 files to USB devices.
//!
//! This module provides:
//! - Chunked file transfers with progress tracking
//! - Integrity verification using checksums
//! - Resumable transfers (detecting already-transferred files)
//! - Batch transfer operations with detailed statistics
//!
//! # Example
//!
//! ```rust,ignore
//! use youtun4_core::transfer::{TransferEngine, TransferOptions, TransferProgress};
//! use std::path::PathBuf;
//!
//! let mut engine = TransferEngine::new();
//! let options = TransferOptions::default();
//!
//! let result = engine.transfer_files(
//!     &[PathBuf::from("/source/song.mp3")],
//!     &PathBuf::from("/mnt/usb"),
//!     &options,
//!     Some(|progress| println!("Progress: {:?}", progress)),
//! ).await?;
//!
//! println!("Transferred {} files", result.files_transferred);
//! ```

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use crate::error::{Error, FileSystemError, Result, TransferError};
use crate::playlist::is_audio_file;

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
    const fn empty() -> Self {
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
// Transfer Engine
// =============================================================================

/// Engine for performing file transfers with progress tracking and verification.
pub struct TransferEngine {
    /// Cancellation flag.
    cancelled: Arc<AtomicBool>,
}

impl TransferEngine {
    /// Create a new transfer engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new transfer engine with a shared cancellation flag.
    #[must_use]
    pub const fn with_cancellation(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }

    /// Request cancellation of the transfer.
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

    /// Transfer files from source paths to a destination directory.
    ///
    /// # Arguments
    ///
    /// * `source_files` - List of source file paths to transfer
    /// * `destination_dir` - Destination directory path
    /// * `options` - Transfer configuration options
    /// * `progress_callback` - Optional callback for progress updates
    ///
    /// # Errors
    ///
    /// Returns an error if the transfer fails completely. Partial failures
    /// are recorded in the result's `failed_transfers` field if `continue_on_error`
    /// is enabled.
    #[allow(clippy::too_many_lines)]
    pub fn transfer_files<F>(
        &mut self,
        source_files: &[PathBuf],
        destination_dir: &Path,
        options: &TransferOptions,
        mut progress_callback: Option<F>,
    ) -> Result<TransferResult>
    where
        F: FnMut(&TransferProgress),
    {
        // Validate options
        options.validate()?;

        // Validate destination
        if !destination_dir.exists() {
            return Err(Error::FileSystem(FileSystemError::NotFound {
                path: destination_dir.to_path_buf(),
            }));
        }
        if !destination_dir.is_dir() {
            return Err(Error::FileSystem(FileSystemError::InvalidPath {
                path: destination_dir.to_path_buf(),
                reason: "destination is not a directory".to_string(),
            }));
        }

        let start_time = Instant::now();
        let mut result = TransferResult::empty();
        result.total_files = source_files.len();

        // Build transfer items
        let items = self.build_transfer_items(source_files, destination_dir)?;
        let total_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();

        // Initialize progress
        let mut progress = TransferProgress::preparing(items.len(), total_bytes);
        progress.total_files = items.len();
        progress.total_bytes = total_bytes;

        if let Some(cb) = progress_callback.as_mut() {
            cb(&progress);
        }

        // Check for cancellation
        if self.is_cancelled() {
            result.was_cancelled = true;
            result.success = false;
            return Ok(result);
        }

        progress.status = TransferStatus::Transferring;
        let mut last_progress_update = Instant::now();
        let mut bytes_since_last_update: u64 = 0;
        let mut speed_samples: Vec<f64> = Vec::with_capacity(10);

        // Transfer each file
        for (index, item) in items.iter().enumerate() {
            // Check for cancellation
            if self.is_cancelled() {
                info!("Transfer cancelled at file {}/{}", index + 1, items.len());
                result.was_cancelled = true;
                result.success = false;
                break;
            }

            progress.current_file_index = index + 1;
            progress.current_file_name = item.source.file_name().map_or_else(
                || "unknown".to_string(),
                |n| n.to_string_lossy().to_string(),
            );
            progress.current_file_bytes = 0;
            progress.current_file_total = item.size_bytes;

            // Check if file should be skipped
            if options.skip_existing
                && item.destination.exists()
                && let Ok(dest_meta) = fs::metadata(&item.destination)
            {
                let sizes_match = dest_meta.len() == item.size_bytes;
                let checksums_match = if options.verify_existing_checksum && sizes_match {
                    self.verify_checksum(&item.source, &item.destination)?
                } else {
                    sizes_match
                };

                if checksums_match {
                    debug!("Skipping existing file: {}", item.destination.display());
                    result.files_skipped += 1;
                    result.bytes_skipped += item.size_bytes;
                    progress.files_skipped += 1;
                    progress.total_bytes_transferred += item.size_bytes;

                    result.transferred_files.push(TransferredFile {
                        source: item.source.clone(),
                        destination: item.destination.clone(),
                        size_bytes: item.size_bytes,
                        checksum: None,
                        duration_secs: 0.0,
                        skipped: true,
                    });

                    continue;
                }
            }

            // Transfer the file with retries
            let file_start = Instant::now();
            let mut retry_count = 0;
            let mut transfer_success = false;
            let mut checksum: Option<String> = None;
            let mut last_error: Option<String> = None;

            while retry_count <= options.max_retries && !transfer_success {
                if retry_count > 0 {
                    debug!(
                        "Retry {} of {} for file: {}",
                        retry_count,
                        options.max_retries,
                        item.source.display()
                    );
                    std::thread::sleep(options.retry_delay);
                }

                match self.transfer_single_file(
                    item,
                    options,
                    &mut progress,
                    &mut progress_callback,
                    &mut last_progress_update,
                    &mut bytes_since_last_update,
                    &mut speed_samples,
                    start_time,
                ) {
                    Ok(file_checksum) => {
                        checksum = file_checksum;
                        transfer_success = true;
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                        retry_count += 1;
                        if retry_count > options.max_retries {
                            error!(
                                "Failed to transfer file after {} retries: {}",
                                options.max_retries,
                                item.source.display()
                            );
                        }
                    }
                }
            }

            if transfer_success {
                let duration = file_start.elapsed().as_secs_f64();
                result.files_transferred += 1;
                result.bytes_transferred += item.size_bytes;
                progress.files_completed += 1;

                result.transferred_files.push(TransferredFile {
                    source: item.source.clone(),
                    destination: item.destination.clone(),
                    size_bytes: item.size_bytes,
                    checksum,
                    duration_secs: duration,
                    skipped: false,
                });
            } else {
                result.files_failed += 1;
                progress.files_failed += 1;

                result.failed_transfers.push(FailedTransfer {
                    source: item.source.clone(),
                    destination: item.destination.clone(),
                    error: last_error.unwrap_or_else(|| "Unknown error".to_string()),
                    retry_count,
                });

                if !options.continue_on_error {
                    result.success = false;
                    break;
                }
            }
        }

        // Finalize result
        let total_duration = start_time.elapsed().as_secs_f64();
        result.duration_secs = total_duration;
        result.average_speed_bps = if total_duration > 0.0 {
            result.bytes_transferred as f64 / total_duration
        } else {
            0.0
        };

        result.success = result.success && result.files_failed == 0 && !result.was_cancelled;

        // Send final progress update
        progress.status = if result.was_cancelled {
            TransferStatus::Cancelled
        } else if result.success {
            TransferStatus::Completed
        } else {
            TransferStatus::Failed
        };
        progress.elapsed_secs = total_duration;

        if let Some(cb) = progress_callback.as_mut() {
            cb(&progress);
        }

        info!(
            "Transfer complete: {} transferred, {} skipped, {} failed in {:.2}s ({:.2} MB/s)",
            result.files_transferred,
            result.files_skipped,
            result.files_failed,
            total_duration,
            result.average_speed_bps / (1024.0 * 1024.0)
        );

        Ok(result)
    }

    /// Build transfer items from source paths.
    fn build_transfer_items(
        &self,
        source_files: &[PathBuf],
        destination_dir: &Path,
    ) -> Result<Vec<TransferItem>> {
        let mut items = Vec::with_capacity(source_files.len());

        for source in source_files {
            if !source.exists() {
                return Err(Error::Transfer(TransferError::SourceNotFound {
                    path: source.clone(),
                }));
            }

            let file_name = source.file_name().ok_or_else(|| {
                Error::FileSystem(FileSystemError::InvalidPath {
                    path: source.clone(),
                    reason: "source file has no name".to_string(),
                })
            })?;

            let destination = destination_dir.join(file_name);
            let size_bytes = fs::metadata(source)
                .map_err(|e| {
                    Error::FileSystem(FileSystemError::ReadFailed {
                        path: source.clone(),
                        reason: e.to_string(),
                    })
                })?
                .len();

            items.push(TransferItem {
                source: source.clone(),
                destination,
                size_bytes,
            });
        }

        Ok(items)
    }

    /// Transfer a single file with chunked writing and progress updates.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn transfer_single_file<F>(
        &self,
        item: &TransferItem,
        options: &TransferOptions,
        progress: &mut TransferProgress,
        progress_callback: &mut Option<F>,
        last_progress_update: &mut Instant,
        bytes_since_last_update: &mut u64,
        speed_samples: &mut Vec<f64>,
        start_time: Instant,
    ) -> Result<Option<String>>
    where
        F: FnMut(&TransferProgress),
    {
        debug!(
            "Transferring: {} -> {}",
            item.source.display(),
            item.destination.display()
        );

        // Open source file
        let source_file = File::open(&item.source).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: item.source.clone(),
                reason: e.to_string(),
            })
        })?;
        let mut reader = BufReader::with_capacity(options.chunk_size, source_file);

        // Create destination file
        let dest_file = File::create(&item.destination).map_err(|e| {
            Error::Transfer(TransferError::DestinationNotWritable {
                path: item.destination.clone(),
                reason: e.to_string(),
            })
        })?;
        let mut writer = BufWriter::with_capacity(options.chunk_size, dest_file);

        // Initialize hasher for checksum calculation
        let mut hasher = if options.verify_integrity {
            Some(Sha256::new())
        } else {
            None
        };

        // Transfer in chunks
        let mut buffer = vec![0u8; options.chunk_size];
        let mut bytes_written: u64 = 0;

        loop {
            // Check for cancellation
            if self.is_cancelled() {
                // Clean up partial file
                drop(writer);
                let _ = fs::remove_file(&item.destination);
                return Err(Error::Cancelled);
            }

            // Read chunk
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                Error::Transfer(TransferError::Interrupted {
                    file: item.source.display().to_string(),
                    reason: e.to_string(),
                })
            })?;

            if bytes_read == 0 {
                break; // EOF
            }

            // Write chunk
            writer.write_all(&buffer[..bytes_read]).map_err(|e| {
                Error::Transfer(TransferError::CopyFailed {
                    source_path: item.source.clone(),
                    destination: item.destination.clone(),
                    reason: e.to_string(),
                })
            })?;

            // Update checksum
            if let Some(ref mut h) = hasher {
                h.update(&buffer[..bytes_read]);
            }

            bytes_written += bytes_read as u64;
            progress.current_file_bytes = bytes_written;
            progress.total_bytes_transferred += bytes_read as u64;
            *bytes_since_last_update += bytes_read as u64;

            // Update progress if interval has passed
            let now = Instant::now();
            if now.duration_since(*last_progress_update) >= options.progress_interval {
                let elapsed = now.duration_since(*last_progress_update).as_secs_f64();
                let current_speed = *bytes_since_last_update as f64 / elapsed;

                // Update rolling average speed
                speed_samples.push(current_speed);
                if speed_samples.len() > 10 {
                    speed_samples.remove(0);
                }
                let avg_speed: f64 = speed_samples.iter().sum::<f64>() / speed_samples.len() as f64;

                progress.transfer_speed_bps = avg_speed;
                progress.elapsed_secs = start_time.elapsed().as_secs_f64();

                // Calculate estimated remaining time
                let bytes_remaining = progress.total_bytes - progress.total_bytes_transferred;
                if avg_speed > 0.0 {
                    progress.estimated_remaining_secs = Some(bytes_remaining as f64 / avg_speed);
                }

                if let Some(cb) = progress_callback.as_mut() {
                    cb(progress);
                }

                *last_progress_update = now;
                *bytes_since_last_update = 0;
            }
        }

        // Flush writer
        writer.flush().map_err(|e| {
            Error::Transfer(TransferError::CopyFailed {
                source_path: item.source.clone(),
                destination: item.destination.clone(),
                reason: format!("Failed to flush: {e}"),
            })
        })?;

        // Get checksum
        let checksum = hasher.map(|h| format!("{:x}", h.finalize()));

        // Verify integrity if enabled
        if options.verify_integrity {
            progress.status = TransferStatus::Verifying;
            if let Some(cb) = progress_callback.as_mut() {
                cb(progress);
            }

            if !self.verify_file_integrity(&item.destination, checksum.as_ref())? {
                // Clean up corrupted file
                let _ = fs::remove_file(&item.destination);
                return Err(Error::Transfer(TransferError::IntegrityCheckFailed {
                    file: item.destination.clone(),
                    expected: checksum.unwrap_or_default(),
                    actual: "verification failed".to_string(),
                }));
            }
            progress.status = TransferStatus::Transferring;
        }

        // Preserve timestamps if enabled
        if options.preserve_timestamps
            && let Ok(source_meta) = fs::metadata(&item.source)
            && let Ok(modified) = source_meta.modified()
        {
            let _ = filetime::set_file_mtime(
                &item.destination,
                filetime::FileTime::from_system_time(modified),
            );
        }

        Ok(checksum)
    }

    /// Verify file integrity by re-reading and computing checksum.
    fn verify_file_integrity(
        &self,
        path: &Path,
        expected_checksum: Option<&String>,
    ) -> Result<bool> {
        let Some(expected) = expected_checksum else {
            return Ok(true); // No checksum to verify
        };

        let actual = self.compute_file_checksum(path)?;
        Ok(actual == *expected)
    }

    /// Compute SHA-256 checksum of a file.
    pub fn compute_file_checksum(&self, path: &Path) -> Result<String> {
        let file = File::open(path).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; DEFAULT_CHUNK_SIZE];

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                Error::FileSystem(FileSystemError::ReadFailed {
                    path: path.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Verify that source and destination files have matching checksums.
    fn verify_checksum(&self, source: &Path, destination: &Path) -> Result<bool> {
        let source_checksum = self.compute_file_checksum(source)?;
        let dest_checksum = self.compute_file_checksum(destination)?;
        Ok(source_checksum == dest_checksum)
    }

    /// Transfer a playlist directory to a device.
    ///
    /// This is a convenience method that:
    /// 1. Scans the source directory for audio files
    /// 2. Filters to only audio files
    /// 3. Transfers them to the destination
    ///
    /// # Arguments
    ///
    /// * `source_dir` - Source directory containing audio files
    /// * `destination_dir` - Destination directory on the device
    /// * `options` - Transfer options
    /// * `progress_callback` - Optional progress callback
    pub fn transfer_playlist<F>(
        &mut self,
        source_dir: &Path,
        destination_dir: &Path,
        options: &TransferOptions,
        progress_callback: Option<F>,
    ) -> Result<TransferResult>
    where
        F: FnMut(&TransferProgress),
    {
        // Validate source directory
        if !source_dir.exists() {
            return Err(Error::FileSystem(FileSystemError::NotFound {
                path: source_dir.to_path_buf(),
            }));
        }
        if !source_dir.is_dir() {
            return Err(Error::FileSystem(FileSystemError::InvalidPath {
                path: source_dir.to_path_buf(),
                reason: "source is not a directory".to_string(),
            }));
        }

        // Collect audio files
        let mut audio_files: Vec<PathBuf> = Vec::new();
        let entries = fs::read_dir(source_dir).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: source_dir.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.is_file() && is_audio_file(&path) {
                audio_files.push(path);
            }
        }

        if audio_files.is_empty() {
            warn!(
                "No audio files found in source directory: {}",
                source_dir.display()
            );
            return Ok(TransferResult::empty());
        }

        // Sort files by name for consistent ordering
        audio_files.sort();

        info!(
            "Transferring {} audio files from {} to {}",
            audio_files.len(),
            source_dir.display(),
            destination_dir.display()
        );

        self.transfer_files(&audio_files, destination_dir, options, progress_callback)
    }

    /// Transfer files and save a checksum manifest to the destination.
    ///
    /// This is a convenience method that:
    /// 1. Transfers files with integrity verification enabled
    /// 2. Creates a checksum manifest from the transfer result
    /// 3. Saves the manifest to the destination directory
    ///
    /// # Arguments
    ///
    /// * `source_files` - List of source file paths to transfer
    /// * `destination_dir` - Destination directory path
    /// * `options` - Transfer configuration options (`verify_integrity` should be true)
    /// * `progress_callback` - Optional callback for progress updates
    ///
    /// # Returns
    ///
    /// A tuple of (`TransferResult`, `ChecksumManifest`)
    ///
    /// # Errors
    ///
    /// Returns an error if the transfer fails or the manifest cannot be saved.
    pub fn transfer_files_with_manifest<F>(
        &mut self,
        source_files: &[PathBuf],
        destination_dir: &Path,
        options: &TransferOptions,
        progress_callback: Option<F>,
    ) -> Result<(TransferResult, crate::integrity::ChecksumManifest)>
    where
        F: FnMut(&TransferProgress),
    {
        // Ensure integrity verification is enabled for manifest generation
        let mut opts = options.clone();
        opts.verify_integrity = true;

        let result =
            self.transfer_files(source_files, destination_dir, &opts, progress_callback)?;

        // Create manifest from transfer result
        let manifest = crate::integrity::ChecksumManifest::from_transfer_result(&result);

        // Save manifest to destination if we transferred any files
        if !manifest.is_empty() {
            manifest.save_to_directory(destination_dir)?;
            info!(
                "Saved checksum manifest with {} files to {}",
                manifest.len(),
                destination_dir.display()
            );
        }

        Ok((result, manifest))
    }

    /// Transfer a playlist directory and save a checksum manifest.
    ///
    /// Combines `transfer_playlist` with automatic manifest generation.
    ///
    /// # Arguments
    ///
    /// * `source_dir` - Source directory containing audio files
    /// * `destination_dir` - Destination directory on the device
    /// * `options` - Transfer options
    /// * `progress_callback` - Optional progress callback
    ///
    /// # Returns
    ///
    /// A tuple of (`TransferResult`, `ChecksumManifest`)
    pub fn transfer_playlist_with_manifest<F>(
        &mut self,
        source_dir: &Path,
        destination_dir: &Path,
        options: &TransferOptions,
        progress_callback: Option<F>,
    ) -> Result<(TransferResult, crate::integrity::ChecksumManifest)>
    where
        F: FnMut(&TransferProgress),
    {
        // Validate source directory
        if !source_dir.exists() {
            return Err(Error::FileSystem(FileSystemError::NotFound {
                path: source_dir.to_path_buf(),
            }));
        }
        if !source_dir.is_dir() {
            return Err(Error::FileSystem(FileSystemError::InvalidPath {
                path: source_dir.to_path_buf(),
                reason: "source is not a directory".to_string(),
            }));
        }

        // Collect audio files
        let mut audio_files: Vec<PathBuf> = Vec::new();
        let entries = fs::read_dir(source_dir).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: source_dir.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.is_file() && is_audio_file(&path) {
                audio_files.push(path);
            }
        }

        if audio_files.is_empty() {
            warn!(
                "No audio files found in source directory: {}",
                source_dir.display()
            );
            return Ok((
                TransferResult::empty(),
                crate::integrity::ChecksumManifest::new(),
            ));
        }

        // Sort files by name for consistent ordering
        audio_files.sort();

        info!(
            "Transferring {} audio files from {} to {} with manifest",
            audio_files.len(),
            source_dir.display(),
            destination_dir.display()
        );

        self.transfer_files_with_manifest(&audio_files, destination_dir, options, progress_callback)
    }
}

impl Default for TransferEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut file = File::create(&path).expect("create file");
        file.write_all(content).expect("write content");
        path
    }

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
        assert!(opts.validate().is_ok());

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
    fn test_transfer_single_file() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let content = b"Hello, World! This is test content for file transfer.";
        let source_path = create_test_file(source_dir.path(), "test.mp3", content);

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert_eq!(result.files_transferred, 1);
        assert_eq!(result.files_skipped, 0);
        assert_eq!(result.files_failed, 0);
        assert_eq!(result.bytes_transferred, content.len() as u64);
        assert!(result.success);

        // Verify file exists at destination
        let dest_path = dest_dir.path().join("test.mp3");
        assert!(dest_path.exists());

        // Verify content matches
        let dest_content = fs::read(&dest_path).expect("read dest");
        assert_eq!(dest_content, content);
    }

    #[test]
    fn test_transfer_skip_existing() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let content = b"Test content";
        let source_path = create_test_file(source_dir.path(), "test.mp3", content);

        // Create existing file at destination
        create_test_file(dest_dir.path(), "test.mp3", content);

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert_eq!(result.files_transferred, 0);
        assert_eq!(result.files_skipped, 1);
        assert_eq!(result.bytes_skipped, content.len() as u64);
        assert!(result.success);
    }

    #[test]
    fn test_transfer_multiple_files() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let sources = vec![
            create_test_file(source_dir.path(), "song1.mp3", b"Content 1"),
            create_test_file(source_dir.path(), "song2.mp3", b"Content 2 longer"),
            create_test_file(source_dir.path(), "song3.mp3", b"Content 3 even longer"),
        ];

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_files(
                &sources,
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert_eq!(result.files_transferred, 3);
        assert_eq!(result.files_failed, 0);
        assert!(result.success);

        // Verify all files exist
        for source in &sources {
            let dest = dest_dir.path().join(source.file_name().unwrap());
            assert!(dest.exists());
        }
    }

    #[test]
    fn test_transfer_with_progress_callback() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let content = vec![0u8; 100_000]; // 100 KB
        let source_path = create_test_file(source_dir.path(), "large.mp3", &content);

        let mut engine = TransferEngine::new();
        let mut options = TransferOptions::default();
        options.progress_interval = Duration::from_millis(1); // Fast updates for testing

        let progress_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let updates_clone = Arc::clone(&progress_updates);

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                Some(move |p: &TransferProgress| {
                    updates_clone.lock().unwrap().push(p.clone());
                }),
            )
            .expect("transfer should succeed");

        assert!(result.success);

        let updates = progress_updates.lock().unwrap();
        assert!(!updates.is_empty());

        // Should have at least preparing and completed status
        assert!(
            updates
                .iter()
                .any(|p| p.status == TransferStatus::Preparing)
        );
        assert!(
            updates
                .last()
                .is_some_and(|p| p.status == TransferStatus::Completed)
        );
    }

    #[test]
    fn test_transfer_cancellation() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let sources: Vec<PathBuf> = (0..10)
            .map(|i| create_test_file(source_dir.path(), &format!("song{i}.mp3"), b"Content"))
            .collect();

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        // Cancel immediately
        engine.cancel();

        let result = engine
            .transfer_files(
                &sources,
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should return result");

        assert!(result.was_cancelled);
        assert!(!result.success);
    }

    #[test]
    fn test_compute_file_checksum() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let content = b"Test content for checksum calculation";
        let path = create_test_file(temp_dir.path(), "test.txt", content);

        let engine = TransferEngine::new();
        let checksum = engine
            .compute_file_checksum(&path)
            .expect("compute checksum");

        // SHA-256 should produce 64 hex characters
        assert_eq!(checksum.len(), 64);

        // Same content should produce same checksum
        let path2 = create_test_file(temp_dir.path(), "test2.txt", content);
        let checksum2 = engine
            .compute_file_checksum(&path2)
            .expect("compute checksum");
        assert_eq!(checksum, checksum2);

        // Different content should produce different checksum
        let path3 = create_test_file(temp_dir.path(), "test3.txt", b"Different content");
        let checksum3 = engine
            .compute_file_checksum(&path3)
            .expect("compute checksum");
        assert_ne!(checksum, checksum3);
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
    fn test_transfer_integrity_verification() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let content = b"Content for integrity verification test";
        let source_path = create_test_file(source_dir.path(), "test.mp3", content);

        let mut engine = TransferEngine::new();
        let options = TransferOptions::reliable(); // Full verification

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert!(result.success);
        assert_eq!(result.files_transferred, 1);

        // Verify checksum was recorded
        let transferred = &result.transferred_files[0];
        assert!(transferred.checksum.is_some());
    }

    #[test]
    fn test_transfer_source_not_found() {
        let dest_dir = TempDir::new().expect("create dest dir");
        let non_existent = PathBuf::from("/non/existent/file.mp3");

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine.transfer_files(
            &[non_existent],
            dest_dir.path(),
            &options,
            None::<fn(&TransferProgress)>,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_playlist_directory() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        // Create audio files
        create_test_file(source_dir.path(), "song1.mp3", b"MP3 content 1");
        create_test_file(source_dir.path(), "song2.mp3", b"MP3 content 2");
        create_test_file(source_dir.path(), "readme.txt", b"Not audio"); // Should be skipped

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_playlist(
                source_dir.path(),
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        // Only audio files should be transferred
        assert_eq!(result.files_transferred, 2);
        assert!(result.success);

        // Verify only mp3 files exist at destination
        assert!(dest_dir.path().join("song1.mp3").exists());
        assert!(dest_dir.path().join("song2.mp3").exists());
        assert!(!dest_dir.path().join("readme.txt").exists());
    }

    // =============================================================================
    // Additional Transfer Engine Tests - Edge Cases
    // =============================================================================

    #[test]
    fn test_transfer_empty_source_list() {
        let dest_dir = TempDir::new().expect("create dest dir");
        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_files(
                &[],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert_eq!(result.total_files, 0);
        assert_eq!(result.files_transferred, 0);
        assert!(result.success);
    }

    #[test]
    fn test_transfer_to_nonexistent_destination() {
        let source_dir = TempDir::new().expect("create source dir");
        create_test_file(source_dir.path(), "test.mp3", b"content");

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();
        let sources = vec![source_dir.path().join("test.mp3")];

        let result = engine.transfer_files(
            &sources,
            Path::new("/nonexistent/destination/path"),
            &options,
            None::<fn(&TransferProgress)>,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_large_file_chunking() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        // Create a file larger than chunk size
        let large_content = vec![0u8; 256 * 1024]; // 256 KB
        let source_path = create_test_file(source_dir.path(), "large.mp3", &large_content);

        let mut engine = TransferEngine::new();
        let mut options = TransferOptions::default();
        options.chunk_size = 8 * 1024; // 8 KB chunks

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert!(result.success);
        assert_eq!(result.bytes_transferred, large_content.len() as u64);

        // Verify content integrity
        let dest_content = fs::read(dest_dir.path().join("large.mp3")).expect("read dest");
        assert_eq!(dest_content, large_content);
    }

    #[test]
    fn test_transfer_with_different_file_sizes() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let sources = vec![
            create_test_file(source_dir.path(), "tiny.mp3", b"x"),
            create_test_file(source_dir.path(), "small.mp3", b"small content here"),
            create_test_file(source_dir.path(), "medium.mp3", &vec![0u8; 10_000]),
        ];

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_files(
                &sources,
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert_eq!(result.files_transferred, 3);
        assert!(result.success);
        assert!(result.bytes_transferred > 10_000);
    }

    #[test]
    fn test_transfer_overwrites_existing_different_size() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let source_content = b"new content that is longer";
        let source_path = create_test_file(source_dir.path(), "test.mp3", source_content);

        // Create existing file at destination with different content
        create_test_file(dest_dir.path(), "test.mp3", b"old");

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        // File should be transferred (not skipped) because size differs
        assert_eq!(result.files_transferred, 1);
        assert_eq!(result.files_skipped, 0);

        // Verify content was overwritten
        let dest_content = fs::read(dest_dir.path().join("test.mp3")).expect("read dest");
        assert_eq!(dest_content, source_content);
    }

    #[test]
    fn test_transfer_no_skip_existing_option() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let content = b"same content";
        let source_path = create_test_file(source_dir.path(), "test.mp3", content);
        create_test_file(dest_dir.path(), "test.mp3", content);

        let mut engine = TransferEngine::new();
        let mut options = TransferOptions::default();
        options.skip_existing = false;

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        // File should be transferred, not skipped
        assert_eq!(result.files_transferred, 1);
        assert_eq!(result.files_skipped, 0);
    }

    #[test]
    fn test_transfer_with_checksum_verification_existing() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let content = b"content for checksum test";
        let source_path = create_test_file(source_dir.path(), "test.mp3", content);
        create_test_file(dest_dir.path(), "test.mp3", content);

        let mut engine = TransferEngine::new();
        let options = TransferOptions::reliable(); // Includes verify_existing_checksum

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        // File should be skipped because checksums match
        assert_eq!(result.files_skipped, 1);
        assert_eq!(result.files_transferred, 0);
    }

    #[test]
    fn test_transfer_without_integrity_verification() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let content = b"test content";
        let source_path = create_test_file(source_dir.path(), "test.mp3", content);

        let mut engine = TransferEngine::new();
        let options = TransferOptions::fast(); // No integrity verification

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert!(result.success);
        // No checksum should be recorded
        assert!(result.transferred_files[0].checksum.is_none());
    }

    #[test]
    fn test_transfer_result_statistics() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let sources = vec![
            create_test_file(source_dir.path(), "file1.mp3", &vec![0u8; 1000]),
            create_test_file(source_dir.path(), "file2.mp3", &vec![0u8; 2000]),
            create_test_file(source_dir.path(), "file3.mp3", &vec![0u8; 3000]),
        ];

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_files(
                &sources,
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        assert_eq!(result.total_files, 3);
        assert_eq!(result.files_transferred, 3);
        assert_eq!(result.bytes_transferred, 6000);
        assert!(result.duration_secs > 0.0);
        assert!(result.average_speed_bps > 0.0);
        assert_eq!(result.transferred_files.len(), 3);
    }

    #[test]
    fn test_transfer_progress_updates_during_transfer() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let content = vec![0u8; 50_000];
        let source_path = create_test_file(source_dir.path(), "test.mp3", &content);

        let mut engine = TransferEngine::new();
        let mut options = TransferOptions::default();
        options.progress_interval = Duration::from_nanos(1); // Very fast updates

        let progress_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count_clone = Arc::clone(&progress_count);

        let result = engine
            .transfer_files(
                &[source_path],
                dest_dir.path(),
                &options,
                Some(move |_: &TransferProgress| {
                    count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }),
            )
            .expect("transfer should succeed");

        assert!(result.success);
        assert!(progress_count.load(std::sync::atomic::Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_transfer_cancellation_token_shared() {
        let engine = TransferEngine::new();
        let token1 = engine.cancellation_token();
        let token2 = engine.cancellation_token();

        assert!(!engine.is_cancelled());

        // Cancelling via one token should affect the engine
        token1.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(engine.is_cancelled());
        assert!(token2.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_transfer_with_shared_cancellation_flag() {
        use std::sync::atomic::AtomicBool;

        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_clone = Arc::clone(&cancelled);

        let engine = TransferEngine::with_cancellation(cancelled);

        // Cancel via the shared flag
        cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(engine.is_cancelled());
    }

    #[test]
    fn test_transfer_playlist_empty_directory() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        // Create only non-audio files
        create_test_file(source_dir.path(), "readme.txt", b"Not audio");
        create_test_file(source_dir.path(), "data.json", b"{}");

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine
            .transfer_playlist(
                source_dir.path(),
                dest_dir.path(),
                &options,
                None::<fn(&TransferProgress)>,
            )
            .expect("transfer should succeed");

        // No audio files to transfer
        assert_eq!(result.files_transferred, 0);
        assert!(result.success);
    }

    #[test]
    fn test_transfer_playlist_nonexistent_source() {
        let dest_dir = TempDir::new().expect("create dest dir");
        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine.transfer_playlist(
            Path::new("/nonexistent/source"),
            dest_dir.path(),
            &options,
            None::<fn(&TransferProgress)>,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_playlist_source_is_file() {
        let source_dir = TempDir::new().expect("create source dir");
        let dest_dir = TempDir::new().expect("create dest dir");

        let file_path = create_test_file(source_dir.path(), "file.mp3", b"content");

        let mut engine = TransferEngine::new();
        let options = TransferOptions::default();

        let result = engine.transfer_playlist(
            &file_path, // Not a directory
            dest_dir.path(),
            &options,
            None::<fn(&TransferProgress)>,
        );

        assert!(result.is_err());
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

    #[test]
    fn test_compute_checksum_consistent() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let content = b"consistent content for hashing";
        let path = create_test_file(temp_dir.path(), "test.txt", content);

        let engine = TransferEngine::new();
        let checksum1 = engine.compute_file_checksum(&path).expect("first checksum");
        let checksum2 = engine
            .compute_file_checksum(&path)
            .expect("second checksum");

        assert_eq!(checksum1, checksum2);
        assert_eq!(checksum1.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn test_compute_checksum_nonexistent_file() {
        let engine = TransferEngine::new();
        let result = engine.compute_file_checksum(Path::new("/nonexistent/file.mp3"));

        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_default_engine() {
        let engine = TransferEngine::default();
        assert!(!engine.is_cancelled());
    }
}
