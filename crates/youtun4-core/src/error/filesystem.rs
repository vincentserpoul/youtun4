use std::path::PathBuf;
use thiserror::Error;

use super::core::Error;

/// Errors related to file system operations.
#[derive(Debug, Error)]
pub enum FileSystemError {
    /// File or directory not found.
    #[error("not found: {path}")]
    NotFound {
        /// Path that was not found.
        path: PathBuf,
    },

    /// Permission denied.
    #[error("permission denied: {path}")]
    PermissionDenied {
        /// Path where permission was denied.
        path: PathBuf,
    },

    /// Path already exists.
    #[error("already exists: {path}")]
    AlreadyExists {
        /// Path that already exists.
        path: PathBuf,
    },

    /// Failed to create directory.
    #[error("failed to create directory {path}: {reason}")]
    CreateDirFailed {
        /// Directory path.
        path: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Failed to read file.
    #[error("failed to read {path}: {reason}")]
    ReadFailed {
        /// File path.
        path: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Failed to write file.
    #[error("failed to write {path}: {reason}")]
    WriteFailed {
        /// File path.
        path: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Failed to delete file or directory.
    #[error("failed to delete {path}: {reason}")]
    DeleteFailed {
        /// Path to delete.
        path: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Failed to copy file.
    #[error("failed to copy from {source_path} to {destination}: {reason}")]
    CopyFailed {
        /// Source path.
        source_path: PathBuf,
        /// Destination path.
        destination: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Invalid path.
    #[error("invalid path: {path} - {reason}")]
    InvalidPath {
        /// The invalid path.
        path: PathBuf,
        /// Reason it's invalid.
        reason: String,
    },
}

// ============================================================================
// From implementations for PathBuf-based errors (common pattern)
// ============================================================================

/// Helper struct for creating file system errors from path operations.
#[derive(Debug)]
pub struct PathError {
    /// The path where the error occurred.
    pub path: PathBuf,
    /// The error message.
    pub message: String,
}

impl PathError {
    /// Create a new path error.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl From<PathError> for Error {
    fn from(e: PathError) -> Self {
        Self::FileSystem(FileSystemError::ReadFailed {
            path: e.path,
            reason: e.message,
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    #[test]
    fn test_fs_read_failed_error() {
        let err = Error::fs_read_failed("/path/to/file", "file not found");
        assert!(err.to_string().contains("/path/to/file"));
        assert_eq!(err.kind(), ErrorKind::FileSystem);
    }

    #[test]
    fn test_fs_write_failed_error() {
        let err = Error::fs_write_failed("/path/to/file", "disk full");
        assert!(err.to_string().contains("disk full"));
    }

    #[test]
    fn test_fs_permission_denied_error() {
        let err = Error::FileSystem(FileSystemError::PermissionDenied {
            path: PathBuf::from("/restricted/file"),
        });
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn test_fs_already_exists_error() {
        let err = Error::FileSystem(FileSystemError::AlreadyExists {
            path: PathBuf::from("/existing/file"),
        });
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_fs_create_dir_failed_error() {
        let err = Error::FileSystem(FileSystemError::CreateDirFailed {
            path: PathBuf::from("/new/directory"),
            reason: "parent doesn't exist".to_string(),
        });
        assert!(err.to_string().contains("create directory"));
    }

    #[test]
    fn test_fs_delete_failed_error() {
        let err = Error::FileSystem(FileSystemError::DeleteFailed {
            path: PathBuf::from("/file/to/delete"),
            reason: "file in use".to_string(),
        });
        assert!(err.to_string().contains("delete"));
        assert!(err.to_string().contains("file in use"));
    }

    #[test]
    fn test_fs_copy_failed_error() {
        let err = Error::FileSystem(FileSystemError::CopyFailed {
            source_path: PathBuf::from("/source"),
            destination: PathBuf::from("/dest"),
            reason: "disk full".to_string(),
        });
        assert!(err.to_string().contains("copy"));
    }

    #[test]
    fn test_fs_invalid_path_error() {
        let err = Error::FileSystem(FileSystemError::InvalidPath {
            path: PathBuf::from("/invalid\0path"),
            reason: "contains null character".to_string(),
        });
        assert!(err.to_string().contains("invalid path"));
    }

    #[test]
    fn test_path_error() {
        let path_err = PathError::new("/some/path", "test error");
        let err: Error = path_err.into();
        assert!(err.to_string().contains("/some/path"));
        assert_eq!(err.kind(), ErrorKind::FileSystem);
    }

    #[test]
    fn test_path_error_debug() {
        let path_err = PathError::new("/test/path", "test message");
        assert_eq!(path_err.path, PathBuf::from("/test/path"));
        assert_eq!(path_err.message, "test message");
    }
}
