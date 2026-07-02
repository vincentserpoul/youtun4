use std::path::PathBuf;
use thiserror::Error;

/// Errors related to file transfer and sync operations.
#[derive(Debug, Error)]
pub enum TransferError {
    /// Transfer was interrupted.
    #[error("transfer interrupted while copying '{file}': {reason}")]
    Interrupted {
        /// File being transferred.
        file: String,
        /// Reason for interruption.
        reason: String,
    },

    /// File integrity verification failed.
    #[error("integrity check failed for '{file}': expected {expected}, got {actual}")]
    IntegrityCheckFailed {
        /// File path.
        file: PathBuf,
        /// Expected checksum/hash.
        expected: String,
        /// Actual checksum/hash.
        actual: String,
    },

    /// Partial transfer (some files failed).
    #[error("partial transfer: {successful} of {total} files transferred, {failed} failed")]
    PartialTransfer {
        /// Number of successful transfers.
        successful: usize,
        /// Total number of files.
        total: usize,
        /// Number of failed transfers.
        failed: usize,
        /// Individual file errors.
        errors: Vec<String>,
    },

    /// Source file not found.
    #[error("source file not found: {path}")]
    SourceNotFound {
        /// Path to the source file.
        path: PathBuf,
    },

    /// Destination is not writable.
    #[error("cannot write to destination: {path} - {reason}")]
    DestinationNotWritable {
        /// Destination path.
        path: PathBuf,
        /// Reason.
        reason: String,
    },

    /// File copy failed.
    #[error("failed to copy '{source_path}' to '{destination}': {reason}")]
    CopyFailed {
        /// Source path.
        source_path: PathBuf,
        /// Destination path.
        destination: PathBuf,
        /// Reason for failure.
        reason: String,
    },
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;
    use crate::error::{Error, ErrorKind};

    #[test]
    fn test_transfer_interrupted_error() {
        let err = Error::Transfer(TransferError::Interrupted {
            file: "song.mp3".to_string(),
            reason: "device disconnected".to_string(),
        });
        assert!(err.to_string().contains("song.mp3"));
        assert!(err.is_retryable());
        assert_eq!(err.kind(), ErrorKind::Transfer);
    }

    #[test]
    fn test_partial_transfer_error() {
        let err = Error::Transfer(TransferError::PartialTransfer {
            successful: 8,
            total: 10,
            failed: 2,
            errors: vec!["file1.mp3: disk full".to_string()],
        });
        assert!(err.to_string().contains("8 of 10"));
    }

    #[test]
    fn test_integrity_check_failed_error() {
        let err = Error::Transfer(TransferError::IntegrityCheckFailed {
            file: PathBuf::from("/mnt/usb/song.mp3"),
            expected: "abc123".to_string(),
            actual: "def456".to_string(),
        });
        assert!(err.to_string().contains("integrity"));
    }

    #[test]
    fn test_source_not_found_error() {
        let err = Error::Transfer(TransferError::SourceNotFound {
            path: PathBuf::from("/path/to/missing/file.mp3"),
        });
        assert!(err.to_string().contains("source file not found"));
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_destination_not_writable_error() {
        let err = Error::Transfer(TransferError::DestinationNotWritable {
            path: PathBuf::from("/readonly/path"),
            reason: "read-only filesystem".to_string(),
        });
        assert!(err.to_string().contains("cannot write"));
        assert!(err.to_string().contains("read-only"));
    }

    #[test]
    fn test_copy_failed_error() {
        let err = Error::Transfer(TransferError::CopyFailed {
            source_path: PathBuf::from("/source/file.mp3"),
            destination: PathBuf::from("/dest/file.mp3"),
            reason: "disk full".to_string(),
        });
        assert!(err.to_string().contains("failed to copy"));
        assert!(err.to_string().contains("disk full"));
    }
}
