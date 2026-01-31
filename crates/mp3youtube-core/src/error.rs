//! Error types for `MP3YouTube` core operations.
//!
//! This module provides a comprehensive error handling framework using `thiserror`
//! for defining custom error types with meaningful messages, and integrates with
//! `anyhow` for error context propagation.
//!
//! # Error Categories
//!
//! - **Device errors**: USB device detection, mounting, capacity, and access issues
//! - **Download errors**: `YouTube` downloading, network, and conversion failures
//! - **Transfer errors**: File sync, copy, and integrity verification issues
//! - **Playlist errors**: Playlist management operations
//! - **File management errors**: File system operations
//!
//! # Example
//!
//! ```rust
//! use mp3youtube_core::error::{Error, Result, ErrorContext};
//!
//! fn do_operation() -> Result<()> {
//!     // Operations that might fail...
//!     Ok(())
//! }
//! ```

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using the crate's Error type.
pub type Result<T> = std::result::Result<T, Error>;

// ============================================================================
// Device Errors
// ============================================================================

/// Errors related to device detection and management.
#[derive(Debug, Error)]
pub enum DeviceError {
    /// Device not found or not connected.
    #[error("device not found: {name}")]
    NotFound {
        /// Device name or identifier.
        name: String,
    },

    /// Device is not mounted or accessible.
    #[error("device not mounted at {mount_point}")]
    NotMounted {
        /// Expected mount point.
        mount_point: PathBuf,
    },

    /// Device was disconnected during operation.
    #[error("device '{name}' was disconnected during operation")]
    Disconnected {
        /// Device name.
        name: String,
    },

    /// Device is read-only.
    #[error("device '{name}' is read-only")]
    ReadOnly {
        /// Device name.
        name: String,
    },

    /// Insufficient space on device.
    #[error(
        "insufficient space on device '{device}': {available_bytes} bytes available, {required_bytes} bytes required"
    )]
    InsufficientSpace {
        /// Device name.
        device: String,
        /// Available space in bytes.
        available_bytes: u64,
        /// Required space in bytes.
        required_bytes: u64,
    },

    /// Permission denied for device access.
    #[error("permission denied for device at {path}: {reason}")]
    PermissionDenied {
        /// Device path.
        path: PathBuf,
        /// Reason for denial.
        reason: String,
    },

    /// Unsupported file system type.
    #[error("unsupported file system '{file_system}' on device '{device}'")]
    UnsupportedFileSystem {
        /// Device name.
        device: String,
        /// File system type.
        file_system: String,
    },

    /// Device enumeration failed.
    #[error("failed to enumerate devices: {reason}")]
    EnumerationFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Mount operation failed.
    #[error("failed to mount device '{device}' at {mount_point}: {reason}")]
    MountFailed {
        /// Device name or identifier.
        device: String,
        /// Target mount point.
        mount_point: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Unmount operation failed.
    #[error("failed to unmount device at {mount_point}: {reason}")]
    UnmountFailed {
        /// Mount point to unmount.
        mount_point: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Device is busy and cannot be unmounted.
    #[error("device at {mount_point} is busy: {reason}")]
    DeviceBusy {
        /// Mount point of the busy device.
        mount_point: PathBuf,
        /// Reason or processes using the device.
        reason: String,
    },

    /// Mount point already exists or is in use.
    #[error("mount point {mount_point} already exists or is in use")]
    MountPointInUse {
        /// The mount point that's already in use.
        mount_point: PathBuf,
    },

    /// Platform not supported for mount operations.
    #[error("mount operations not supported on platform: {platform}")]
    PlatformNotSupported {
        /// Platform identifier.
        platform: String,
    },
}

// ============================================================================
// Download Errors
// ============================================================================

/// Errors related to downloading from `YouTube`.
#[derive(Debug, Error)]
pub enum DownloadError {
    /// Invalid `YouTube` URL format.
    #[error("invalid YouTube URL: {url} - {reason}")]
    InvalidUrl {
        /// The invalid URL.
        url: String,
        /// Reason it's invalid.
        reason: String,
    },

    /// URL is not a playlist.
    #[error("URL is not a playlist: {url}")]
    NotAPlaylist {
        /// The URL.
        url: String,
    },

    /// Network connection failed.
    #[error("network error: {message}")]
    Network {
        /// Error message.
        message: String,
        /// Whether the error is retryable.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Download timed out.
    #[error("download timed out after {timeout_secs} seconds for '{title}'")]
    Timeout {
        /// Video title.
        title: String,
        /// Timeout duration in seconds.
        timeout_secs: u64,
    },

    /// Rate limited by `YouTube`.
    #[error("rate limited by YouTube, retry after {retry_after_secs} seconds")]
    RateLimited {
        /// Suggested retry delay in seconds.
        retry_after_secs: u64,
    },

    /// Video unavailable.
    #[error("video unavailable: {video_id} - {reason}")]
    VideoUnavailable {
        /// Video ID.
        video_id: String,
        /// Reason for unavailability.
        reason: String,
    },

    /// Audio extraction failed.
    #[error("failed to extract audio from '{title}': {reason}")]
    AudioExtractionFailed {
        /// Video title.
        title: String,
        /// Reason for failure.
        reason: String,
    },

    /// Conversion to MP3 failed.
    #[error("failed to convert '{title}' to MP3: {reason}")]
    ConversionFailed {
        /// Video title.
        title: String,
        /// Reason for failure.
        reason: String,
    },

    /// Playlist parsing failed.
    #[error("failed to parse playlist '{playlist_id}': {reason}")]
    PlaylistParseFailed {
        /// Playlist ID.
        playlist_id: String,
        /// Reason for failure.
        reason: String,
    },

    /// Download was cancelled.
    #[error("download cancelled by user")]
    Cancelled,
}

// ============================================================================
// Transfer Errors
// ============================================================================

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

// ============================================================================
// Playlist Errors
// ============================================================================

/// Errors related to playlist management.
#[derive(Debug, Error)]
pub enum PlaylistError {
    /// Playlist already exists.
    #[error("playlist already exists: {name}")]
    AlreadyExists {
        /// Playlist name.
        name: String,
    },

    /// Playlist not found.
    #[error("playlist not found: {name}")]
    NotFound {
        /// Playlist name.
        name: String,
    },

    /// Invalid playlist name.
    #[error("invalid playlist name '{name}': {reason}")]
    InvalidName {
        /// The invalid name.
        name: String,
        /// Reason it's invalid.
        reason: String,
    },

    /// Playlist metadata is corrupted.
    #[error("playlist metadata corrupted for '{name}': {reason}")]
    MetadataCorrupted {
        /// Playlist name.
        name: String,
        /// Reason/details about corruption.
        reason: String,
    },

    /// Playlist is empty.
    #[error("playlist '{name}' is empty")]
    Empty {
        /// Playlist name.
        name: String,
    },

    /// Track not found in playlist.
    #[error("track '{track}' not found in playlist '{playlist}'")]
    TrackNotFound {
        /// Playlist name.
        playlist: String,
        /// Track name.
        track: String,
    },
}

// ============================================================================
// Cache Errors
// ============================================================================

/// Errors related to cache operations.
#[derive(Debug, Error)]
pub enum CacheError {
    /// Cache directory not found or not accessible.
    #[error("cache directory not found: {path}")]
    DirectoryNotFound {
        /// Path to the cache directory.
        path: PathBuf,
    },

    /// Cache entry not found.
    #[error("cache entry not found: {key}")]
    EntryNotFound {
        /// Cache key that was not found.
        key: String,
    },

    /// Cache entry is corrupted or invalid.
    #[error("cache entry corrupted: {key} - {reason}")]
    EntryCorrupted {
        /// Cache key.
        key: String,
        /// Reason for corruption.
        reason: String,
    },

    /// Cache entry has expired.
    #[error("cache entry expired: {key}")]
    EntryExpired {
        /// Cache key.
        key: String,
    },

    /// Failed to serialize cache entry.
    #[error("failed to serialize cache entry '{key}': {reason}")]
    SerializationFailed {
        /// Cache key.
        key: String,
        /// Reason for failure.
        reason: String,
    },

    /// Failed to deserialize cache entry.
    #[error("failed to deserialize cache entry '{key}': {reason}")]
    DeserializationFailed {
        /// Cache key.
        key: String,
        /// Reason for failure.
        reason: String,
    },

    /// Cache is full and cannot accept new entries.
    #[error("cache is full: {current_size} bytes used, max {max_size} bytes")]
    CacheFull {
        /// Current cache size in bytes.
        current_size: u64,
        /// Maximum allowed cache size in bytes.
        max_size: u64,
    },

    /// Cache cleanup failed.
    #[error("cache cleanup failed: {reason}")]
    CleanupFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Cache initialization failed.
    #[error("cache initialization failed: {reason}")]
    InitializationFailed {
        /// Reason for failure.
        reason: String,
    },
}

// ============================================================================
// File System Errors
// ============================================================================

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
// Main Error Enum
// ============================================================================

/// Errors that can occur in `MP3YouTube` core operations.
///
/// This is the main error type that aggregates all domain-specific errors
/// and provides a unified error handling interface.
#[derive(Debug, Error)]
pub enum Error {
    // -------------------------------------------------------------------------
    // Domain-specific error variants (delegating to specialized enums)
    // -------------------------------------------------------------------------
    /// Device-related error.
    #[error("{0}")]
    Device(#[from] DeviceError),

    /// Download-related error.
    #[error("{0}")]
    Download(#[from] DownloadError),

    /// Transfer-related error.
    #[error("{0}")]
    Transfer(#[from] TransferError),

    /// Playlist-related error.
    #[error("{0}")]
    Playlist(#[from] PlaylistError),

    /// File system error.
    #[error("{0}")]
    FileSystem(#[from] FileSystemError),

    /// Cache-related error.
    #[error("{0}")]
    Cache(#[from] CacheError),

    // -------------------------------------------------------------------------
    // Legacy variants (for backward compatibility during migration)
    // -------------------------------------------------------------------------
    /// Device not found or not connected.
    #[deprecated(note = "Use Error::Device(DeviceError::NotFound) instead")]
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Device is not mounted or accessible.
    #[deprecated(note = "Use Error::Device(DeviceError::NotMounted) instead")]
    #[error("Device not mounted: {0}")]
    DeviceNotMounted(String),

    /// Playlist already exists.
    #[deprecated(note = "Use Error::Playlist(PlaylistError::AlreadyExists) instead")]
    #[error("Playlist already exists: {0}")]
    PlaylistAlreadyExists(String),

    /// Playlist not found.
    #[deprecated(note = "Use Error::Playlist(PlaylistError::NotFound) instead")]
    #[error("Playlist not found: {0}")]
    PlaylistNotFound(String),

    /// Invalid playlist name.
    #[deprecated(note = "Use Error::Playlist(PlaylistError::InvalidName) instead")]
    #[error("Invalid playlist name: {0}")]
    InvalidPlaylistName(String),

    /// Invalid `YouTube` URL.
    #[deprecated(note = "Use Error::Download(DownloadError::InvalidUrl) instead")]
    #[error("Invalid YouTube URL: {0}")]
    InvalidYouTubeUrl(String),

    /// `YouTube` URL is not a playlist.
    #[deprecated(note = "Use Error::Download(DownloadError::NotAPlaylist) instead")]
    #[error("URL is not a YouTube playlist: {0}")]
    NotAPlaylist(String),

    /// `YouTube` download failed.
    #[deprecated(note = "Use specific DownloadError variant instead")]
    #[error("YouTube download failed: {0}")]
    DownloadFailed(String),

    /// Sync operation failed.
    #[deprecated(note = "Use Error::Transfer variant instead")]
    #[error("Sync failed: {0}")]
    SyncFailed(String),

    // -------------------------------------------------------------------------
    // General errors
    // -------------------------------------------------------------------------
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// IO error wrapper.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Operation was cancelled.
    #[error("Operation cancelled")]
    Cancelled,

    /// Internal error (unexpected state).
    #[error("Internal error: {0}")]
    Internal(String),

    /// Error with additional context.
    #[error("{context}: {source}")]
    WithContext {
        /// Context description.
        context: String,
        /// Underlying error.
        #[source]
        source: Box<Self>,
    },
}

// ============================================================================
// Error Context Extension Trait
// ============================================================================

/// Extension trait for adding context to errors.
///
/// This provides similar functionality to `anyhow::Context` but works with
/// our typed `Error` enum.
///
/// # Example
///
/// ```rust
/// use mp3youtube_core::error::{Result, ErrorContext};
///
/// fn read_config() -> Result<String> {
///     std::fs::read_to_string("config.json")
///         .map_err(|e| e.into())
///         .context("Failed to read configuration file")
/// }
/// ```
pub trait ErrorContext<T> {
    /// Add context to an error.
    fn context<C: Into<String>>(self, context: C) -> Result<T>;

    /// Add context to an error using a closure (lazy evaluation).
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: Into<String>,
        F: FnOnce() -> C;
}

impl<T> ErrorContext<T> for Result<T> {
    fn context<C: Into<String>>(self, context: C) -> Self {
        self.map_err(|e| Error::WithContext {
            context: context.into(),
            source: Box::new(e),
        })
    }

    fn with_context<C, F>(self, f: F) -> Self
    where
        C: Into<String>,
        F: FnOnce() -> C,
    {
        self.map_err(|e| Error::WithContext {
            context: f().into(),
            source: Box::new(e),
        })
    }
}

// ============================================================================
// Error Kind for Categorization
// ============================================================================

/// High-level error categories for error handling decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Device-related errors.
    Device,
    /// Download-related errors.
    Download,
    /// Transfer-related errors.
    Transfer,
    /// Playlist-related errors.
    Playlist,
    /// File system errors.
    FileSystem,
    /// Cache errors.
    Cache,
    /// Configuration errors.
    Configuration,
    /// IO errors.
    Io,
    /// Serialization errors.
    Serialization,
    /// Cancelled operation.
    Cancelled,
    /// Internal/unexpected errors.
    Internal,
}

impl Error {
    /// Get the kind/category of this error.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Device(_) => ErrorKind::Device,
            Self::Download(_) => ErrorKind::Download,
            Self::Transfer(_) => ErrorKind::Transfer,
            Self::Playlist(_) => ErrorKind::Playlist,
            Self::FileSystem(_) => ErrorKind::FileSystem,
            Self::Cache(_) => ErrorKind::Cache,
            Self::Configuration(_) => ErrorKind::Configuration,
            Self::Io(_) => ErrorKind::Io,
            Self::Serialization(_) => ErrorKind::Serialization,
            Self::Cancelled => ErrorKind::Cancelled,
            Self::Internal(_) => ErrorKind::Internal,
            Self::WithContext { source, .. } => source.kind(),
            // Legacy variants
            #[allow(deprecated)]
            Self::DeviceNotFound(_) | Self::DeviceNotMounted(_) => ErrorKind::Device,
            #[allow(deprecated)]
            Self::PlaylistAlreadyExists(_)
            | Self::PlaylistNotFound(_)
            | Self::InvalidPlaylistName(_) => ErrorKind::Playlist,
            #[allow(deprecated)]
            Self::InvalidYouTubeUrl(_) | Self::NotAPlaylist(_) | Self::DownloadFailed(_) => {
                ErrorKind::Download
            }
            #[allow(deprecated)]
            Self::SyncFailed(_) => ErrorKind::Transfer,
        }
    }

    /// Check if this error is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Download(DownloadError::Network { .. }) => true,
            Self::Download(DownloadError::Timeout { .. }) => true,
            Self::Download(DownloadError::RateLimited { .. }) => true,
            Self::Device(DeviceError::Disconnected { .. }) => true,
            Self::Transfer(TransferError::Interrupted { .. }) => true,
            Self::Io(e) => matches!(
                e.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::ConnectionReset
            ),
            Self::WithContext { source, .. } => source.is_retryable(),
            _ => false,
        }
    }

    /// Check if this is a user-facing error (should be shown to user).
    #[must_use]
    pub const fn is_user_facing(&self) -> bool {
        !matches!(self, Self::Internal(_))
    }

    /// Get the retry delay in seconds, if applicable.
    #[must_use]
    pub fn retry_delay_secs(&self) -> Option<u64> {
        match self {
            Self::Download(DownloadError::RateLimited { retry_after_secs }) => {
                Some(*retry_after_secs)
            }
            Self::Download(DownloadError::Timeout { .. }) => Some(5),
            Self::Download(DownloadError::Network { .. }) => Some(3),
            Self::WithContext { source, .. } => source.retry_delay_secs(),
            _ => None,
        }
    }
}

// ============================================================================
// Convenience Constructors
// ============================================================================

impl Error {
    /// Create a device not found error.
    #[must_use]
    pub fn device_not_found(name: impl Into<String>) -> Self {
        Self::Device(DeviceError::NotFound { name: name.into() })
    }

    /// Create a device not mounted error.
    #[must_use]
    pub fn device_not_mounted(mount_point: impl Into<PathBuf>) -> Self {
        Self::Device(DeviceError::NotMounted {
            mount_point: mount_point.into(),
        })
    }

    /// Create an insufficient space error.
    #[must_use]
    pub fn insufficient_space(device: impl Into<String>, available: u64, required: u64) -> Self {
        Self::Device(DeviceError::InsufficientSpace {
            device: device.into(),
            available_bytes: available,
            required_bytes: required,
        })
    }

    /// Create a playlist not found error.
    #[must_use]
    pub fn playlist_not_found(name: impl Into<String>) -> Self {
        Self::Playlist(PlaylistError::NotFound { name: name.into() })
    }

    /// Create a playlist already exists error.
    #[must_use]
    pub fn playlist_exists(name: impl Into<String>) -> Self {
        Self::Playlist(PlaylistError::AlreadyExists { name: name.into() })
    }

    /// Create an invalid playlist name error.
    #[must_use]
    pub fn invalid_playlist_name(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Playlist(PlaylistError::InvalidName {
            name: name.into(),
            reason: reason.into(),
        })
    }

    /// Create an invalid `YouTube` URL error.
    #[must_use]
    pub fn invalid_youtube_url(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Download(DownloadError::InvalidUrl {
            url: url.into(),
            reason: reason.into(),
        })
    }

    /// Create a not a playlist error.
    #[must_use]
    pub fn not_a_playlist(url: impl Into<String>) -> Self {
        Self::Download(DownloadError::NotAPlaylist { url: url.into() })
    }

    /// Create a network error.
    #[must_use]
    pub fn network_error(message: impl Into<String>) -> Self {
        Self::Download(DownloadError::Network {
            message: message.into(),
            source: None,
        })
    }

    /// Create a file system read error.
    #[must_use]
    pub fn fs_read_failed(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::FileSystem(FileSystemError::ReadFailed {
            path: path.into(),
            reason: reason.into(),
        })
    }

    /// Create a file system write error.
    #[must_use]
    pub fn fs_write_failed(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::FileSystem(FileSystemError::WriteFailed {
            path: path.into(),
            reason: reason.into(),
        })
    }

    /// Create an internal error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Create a mount failed error.
    #[must_use]
    pub fn mount_failed(
        device: impl Into<String>,
        mount_point: impl Into<PathBuf>,
        reason: impl Into<String>,
    ) -> Self {
        Self::Device(DeviceError::MountFailed {
            device: device.into(),
            mount_point: mount_point.into(),
            reason: reason.into(),
        })
    }

    /// Create an unmount failed error.
    #[must_use]
    pub fn unmount_failed(mount_point: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::Device(DeviceError::UnmountFailed {
            mount_point: mount_point.into(),
            reason: reason.into(),
        })
    }

    /// Create a device busy error.
    #[must_use]
    pub fn device_busy(mount_point: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::Device(DeviceError::DeviceBusy {
            mount_point: mount_point.into(),
            reason: reason.into(),
        })
    }

    /// Create a platform not supported error.
    #[must_use]
    pub fn platform_not_supported(platform: impl Into<String>) -> Self {
        Self::Device(DeviceError::PlatformNotSupported {
            platform: platform.into(),
        })
    }

    /// Create a cache entry not found error.
    #[must_use]
    pub fn cache_entry_not_found(key: impl Into<String>) -> Self {
        Self::Cache(CacheError::EntryNotFound { key: key.into() })
    }

    /// Create a cache full error.
    #[must_use]
    pub const fn cache_full(current_size: u64, max_size: u64) -> Self {
        Self::Cache(CacheError::CacheFull {
            current_size,
            max_size,
        })
    }

    /// Create a cache initialization failed error.
    #[must_use]
    pub fn cache_init_failed(reason: impl Into<String>) -> Self {
        Self::Cache(CacheError::InitializationFailed {
            reason: reason.into(),
        })
    }

    /// Create a cache cleanup failed error.
    #[must_use]
    pub fn cache_cleanup_failed(reason: impl Into<String>) -> Self {
        Self::Cache(CacheError::CleanupFailed {
            reason: reason.into(),
        })
    }
}

// ============================================================================
// From implementations for PathBuf-based errors (common pattern)
// ============================================================================

/// Helper struct for creating file system errors from path operations.
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Device Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_device_not_found_error() {
        let err = Error::device_not_found("my-device");
        assert_eq!(err.to_string(), "device not found: my-device");
        assert_eq!(err.kind(), ErrorKind::Device);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_device_not_mounted_error() {
        let err = Error::device_not_mounted("/mnt/usb");
        assert_eq!(err.to_string(), "device not mounted at /mnt/usb");
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    fn test_device_disconnected_error() {
        let err = Error::Device(DeviceError::Disconnected {
            name: "USB Drive".to_string(),
        });
        assert!(err.to_string().contains("USB Drive"));
        assert!(err.to_string().contains("disconnected"));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_insufficient_space_error() {
        let err = Error::insufficient_space("USB Drive", 1_000_000, 5_000_000);
        let msg = err.to_string();
        assert!(msg.contains("USB Drive"));
        assert!(msg.contains("1000000"));
        assert!(msg.contains("5000000"));
    }

    // -------------------------------------------------------------------------
    // Download Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_invalid_youtube_url_error() {
        let err = Error::invalid_youtube_url("http://example.com", "not a YouTube URL");
        assert!(err.to_string().contains("http://example.com"));
        assert!(err.to_string().contains("not a YouTube URL"));
        assert_eq!(err.kind(), ErrorKind::Download);
    }

    #[test]
    fn test_not_a_playlist_error() {
        let err = Error::not_a_playlist("https://youtube.com/watch?v=abc");
        assert!(err.to_string().contains("not a playlist"));
    }

    #[test]
    fn test_network_error() {
        let err = Error::network_error("connection refused");
        assert!(err.to_string().contains("connection refused"));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_rate_limited_error() {
        let err = Error::Download(DownloadError::RateLimited {
            retry_after_secs: 60,
        });
        assert!(err.to_string().contains("rate limited"));
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay_secs(), Some(60));
    }

    #[test]
    fn test_timeout_error() {
        let err = Error::Download(DownloadError::Timeout {
            title: "My Video".to_string(),
            timeout_secs: 30,
        });
        assert!(err.to_string().contains("30 seconds"));
        assert!(err.is_retryable());
    }

    // -------------------------------------------------------------------------
    // Transfer Error Tests
    // -------------------------------------------------------------------------

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

    // -------------------------------------------------------------------------
    // Playlist Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_playlist_not_found_error() {
        let err = Error::playlist_not_found("My Playlist");
        assert_eq!(err.to_string(), "playlist not found: My Playlist");
        assert_eq!(err.kind(), ErrorKind::Playlist);
    }

    #[test]
    fn test_playlist_exists_error() {
        let err = Error::playlist_exists("My Playlist");
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_invalid_playlist_name_error() {
        let err = Error::invalid_playlist_name("bad/name", "contains invalid character");
        assert!(err.to_string().contains("bad/name"));
        assert!(err.to_string().contains("invalid"));
    }

    // -------------------------------------------------------------------------
    // File System Error Tests
    // -------------------------------------------------------------------------

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

    // -------------------------------------------------------------------------
    // Error Context Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_error_context() {
        let result: Result<()> = Err(Error::playlist_not_found("test"));
        let with_context = result.context("while loading user data");

        let err = with_context.unwrap_err();
        assert!(err.to_string().contains("while loading user data"));
        assert!(err.to_string().contains("playlist not found"));
    }

    #[test]
    fn test_error_with_context_lazy() {
        let result: Result<()> = Err(Error::internal("oops"));
        let with_context = result.with_context(|| format!("during operation {}", 42));

        let err = with_context.unwrap_err();
        assert!(err.to_string().contains("during operation 42"));
    }

    #[test]
    fn test_nested_context_preserves_kind() {
        let inner_err = Error::playlist_not_found("test");
        let err = Error::WithContext {
            context: "outer".to_string(),
            source: Box::new(inner_err),
        };

        // Kind should propagate through context
        assert_eq!(err.kind(), ErrorKind::Playlist);
    }

    // -------------------------------------------------------------------------
    // Error Kind Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_error_kind_device() {
        let err = Error::device_not_found("test");
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    fn test_error_kind_download() {
        let err = Error::network_error("test");
        assert_eq!(err.kind(), ErrorKind::Download);
    }

    #[test]
    fn test_error_kind_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err: Error = io_err.into();
        assert_eq!(err.kind(), ErrorKind::Io);
    }

    // -------------------------------------------------------------------------
    // From Implementations Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn test_from_device_error() {
        let device_err = DeviceError::NotFound {
            name: "test".to_string(),
        };
        let err: Error = device_err.into();
        assert!(matches!(err, Error::Device(_)));
    }

    #[test]
    fn test_from_download_error() {
        let download_err = DownloadError::Cancelled;
        let err: Error = download_err.into();
        assert!(matches!(err, Error::Download(_)));
    }

    #[test]
    fn test_from_transfer_error() {
        let transfer_err = TransferError::SourceNotFound {
            path: PathBuf::from("/test"),
        };
        let err: Error = transfer_err.into();
        assert!(matches!(err, Error::Transfer(_)));
    }

    #[test]
    fn test_from_playlist_error() {
        let playlist_err = PlaylistError::NotFound {
            name: "test".to_string(),
        };
        let err: Error = playlist_err.into();
        assert!(matches!(err, Error::Playlist(_)));
    }

    #[test]
    fn test_from_filesystem_error() {
        let fs_err = FileSystemError::NotFound {
            path: PathBuf::from("/test"),
        };
        let err: Error = fs_err.into();
        assert!(matches!(err, Error::FileSystem(_)));
    }

    // -------------------------------------------------------------------------
    // Retryable Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_retryable_network_error() {
        let err = Error::network_error("connection reset");
        assert!(err.is_retryable());
    }

    #[test]
    fn test_not_retryable_playlist_error() {
        let err = Error::playlist_not_found("test");
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_retryable_io_error_interrupted() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Interrupted, "interrupted");
        let err: Error = io_err.into();
        assert!(err.is_retryable());
    }

    #[test]
    fn test_retryable_with_context() {
        let inner = Error::network_error("test");
        let err = Error::WithContext {
            context: "outer".to_string(),
            source: Box::new(inner),
        };
        // Retryable should propagate through context
        assert!(err.is_retryable());
    }

    // -------------------------------------------------------------------------
    // Legacy Variant Tests (for backward compatibility)
    // -------------------------------------------------------------------------

    #[test]
    #[allow(deprecated)]
    fn test_legacy_device_not_found() {
        let err = Error::DeviceNotFound("test".to_string());
        assert_eq!(err.to_string(), "Device not found: test");
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_playlist_not_found() {
        let err = Error::PlaylistNotFound("my-playlist".to_string());
        assert_eq!(err.to_string(), "Playlist not found: my-playlist");
        assert_eq!(err.kind(), ErrorKind::Playlist);
    }

    // -------------------------------------------------------------------------
    // User-Facing Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_user_facing_errors() {
        assert!(Error::playlist_not_found("test").is_user_facing());
        assert!(Error::device_not_found("test").is_user_facing());
        assert!(!Error::internal("unexpected state").is_user_facing());
    }

    // -------------------------------------------------------------------------
    // PathError Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_path_error() {
        let path_err = PathError::new("/some/path", "test error");
        let err: Error = path_err.into();
        assert!(err.to_string().contains("/some/path"));
        assert_eq!(err.kind(), ErrorKind::FileSystem);
    }

    // =============================================================================
    // Additional Error Tests - Edge Cases and Complete Coverage
    // =============================================================================

    // -------------------------------------------------------------------------
    // DeviceError Comprehensive Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_device_read_only_error() {
        let err = Error::Device(DeviceError::ReadOnly {
            name: "SD Card".to_string(),
        });
        assert!(err.to_string().contains("read-only"));
        assert!(err.to_string().contains("SD Card"));
        assert_eq!(err.kind(), ErrorKind::Device);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_device_permission_denied_error() {
        let err = Error::Device(DeviceError::PermissionDenied {
            path: PathBuf::from("/Volumes/Restricted"),
            reason: "root access required".to_string(),
        });
        assert!(err.to_string().contains("permission denied"));
        assert!(err.to_string().contains("root access"));
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    fn test_device_unsupported_filesystem_error() {
        let err = Error::Device(DeviceError::UnsupportedFileSystem {
            device: "USB Drive".to_string(),
            file_system: "NTFS".to_string(),
        });
        assert!(err.to_string().contains("unsupported file system"));
        assert!(err.to_string().contains("NTFS"));
    }

    #[test]
    fn test_device_enumeration_failed_error() {
        let err = Error::Device(DeviceError::EnumerationFailed {
            reason: "system error".to_string(),
        });
        assert!(err.to_string().contains("enumerate"));
        assert!(err.to_string().contains("system error"));
    }

    #[test]
    fn test_mount_failed_error() {
        let err = Error::mount_failed("disk2", "/Volumes/USB", "device busy");
        assert!(err.to_string().contains("mount"));
        assert!(err.to_string().contains("disk2"));
        assert!(err.to_string().contains("device busy"));
    }

    #[test]
    fn test_unmount_failed_error() {
        let err = Error::unmount_failed("/Volumes/USB", "device in use");
        assert!(err.to_string().contains("unmount"));
        assert!(err.to_string().contains("device in use"));
    }

    #[test]
    fn test_device_busy_error() {
        let err = Error::device_busy("/Volumes/USB", "process PID 1234 using device");
        assert!(err.to_string().contains("busy"));
        assert!(err.to_string().contains("PID 1234"));
    }

    #[test]
    fn test_mount_point_in_use_error() {
        let err = Error::Device(DeviceError::MountPointInUse {
            mount_point: PathBuf::from("/Volumes/USB"),
        });
        assert!(err.to_string().contains("mount point"));
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_platform_not_supported_error() {
        let err = Error::platform_not_supported("wasm");
        assert!(err.to_string().contains("not supported"));
        assert!(err.to_string().contains("wasm"));
    }

    // -------------------------------------------------------------------------
    // DownloadError Comprehensive Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_video_unavailable_error() {
        let err = Error::Download(DownloadError::VideoUnavailable {
            video_id: "dQw4w9WgXcQ".to_string(),
            reason: "video is private".to_string(),
        });
        assert!(err.to_string().contains("unavailable"));
        assert!(err.to_string().contains("dQw4w9WgXcQ"));
        assert!(err.to_string().contains("private"));
    }

    #[test]
    fn test_audio_extraction_failed_error() {
        let err = Error::Download(DownloadError::AudioExtractionFailed {
            title: "My Song".to_string(),
            reason: "no audio stream found".to_string(),
        });
        assert!(err.to_string().contains("extract audio"));
        assert!(err.to_string().contains("My Song"));
    }

    #[test]
    fn test_conversion_failed_error() {
        let err = Error::Download(DownloadError::ConversionFailed {
            title: "My Song".to_string(),
            reason: "ffmpeg not found".to_string(),
        });
        assert!(err.to_string().contains("convert"));
        assert!(err.to_string().contains("MP3"));
    }

    #[test]
    fn test_playlist_parse_failed_error() {
        let err = Error::Download(DownloadError::PlaylistParseFailed {
            playlist_id: "PLabc123".to_string(),
            reason: "invalid response".to_string(),
        });
        assert!(err.to_string().contains("parse playlist"));
        assert!(err.to_string().contains("PLabc123"));
    }

    #[test]
    fn test_download_cancelled_error() {
        let err = Error::Download(DownloadError::Cancelled);
        assert!(err.to_string().contains("cancelled"));
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_network_error_with_source() {
        let io_err =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused");
        let err = Error::Download(DownloadError::Network {
            message: "failed to connect".to_string(),
            source: Some(Box::new(io_err)),
        });
        assert!(err.to_string().contains("network"));
        assert!(err.is_retryable());
    }

    // -------------------------------------------------------------------------
    // TransferError Comprehensive Tests
    // -------------------------------------------------------------------------

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

    // -------------------------------------------------------------------------
    // PlaylistError Comprehensive Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_playlist_metadata_corrupted_error() {
        let err = Error::Playlist(PlaylistError::MetadataCorrupted {
            name: "My Playlist".to_string(),
            reason: "invalid JSON".to_string(),
        });
        assert!(err.to_string().contains("corrupted"));
        assert!(err.to_string().contains("My Playlist"));
    }

    #[test]
    fn test_playlist_empty_error() {
        let err = Error::Playlist(PlaylistError::Empty {
            name: "Empty Playlist".to_string(),
        });
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_track_not_found_error() {
        let err = Error::Playlist(PlaylistError::TrackNotFound {
            playlist: "My Playlist".to_string(),
            track: "missing_song.mp3".to_string(),
        });
        assert!(err.to_string().contains("not found"));
        assert!(err.to_string().contains("missing_song.mp3"));
    }

    // -------------------------------------------------------------------------
    // FileSystemError Comprehensive Tests
    // -------------------------------------------------------------------------

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

    // -------------------------------------------------------------------------
    // General Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_configuration_error() {
        let err = Error::Configuration("invalid config value".to_string());
        assert!(err.to_string().contains("Configuration"));
        assert_eq!(err.kind(), ErrorKind::Configuration);
    }

    #[test]
    fn test_cancelled_error() {
        let err = Error::Cancelled;
        assert!(err.to_string().contains("cancelled"));
        assert_eq!(err.kind(), ErrorKind::Cancelled);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_internal_error() {
        let err = Error::internal("unexpected state in state machine");
        assert!(err.to_string().contains("Internal"));
        assert!(err.to_string().contains("unexpected state"));
        assert_eq!(err.kind(), ErrorKind::Internal);
        assert!(!err.is_user_facing());
    }

    #[test]
    fn test_serialization_error() {
        let json_err = serde_json::from_str::<String>("invalid json").unwrap_err();
        let err: Error = json_err.into();
        assert_eq!(err.kind(), ErrorKind::Serialization);
    }

    // -------------------------------------------------------------------------
    // Error Kind Comprehensive Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_all_error_kinds() {
        assert_eq!(ErrorKind::Device, ErrorKind::Device);
        assert_eq!(ErrorKind::Download, ErrorKind::Download);
        assert_eq!(ErrorKind::Transfer, ErrorKind::Transfer);
        assert_eq!(ErrorKind::Playlist, ErrorKind::Playlist);
        assert_eq!(ErrorKind::FileSystem, ErrorKind::FileSystem);
        assert_eq!(ErrorKind::Configuration, ErrorKind::Configuration);
        assert_eq!(ErrorKind::Io, ErrorKind::Io);
        assert_eq!(ErrorKind::Serialization, ErrorKind::Serialization);
        assert_eq!(ErrorKind::Cancelled, ErrorKind::Cancelled);
        assert_eq!(ErrorKind::Internal, ErrorKind::Internal);
    }

    #[test]
    fn test_error_kind_clone() {
        let kind = ErrorKind::Device;
        let cloned = kind;
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_error_kind_debug() {
        let kind = ErrorKind::Device;
        let debug_str = format!("{kind:?}");
        assert!(debug_str.contains("Device"));
    }

    // -------------------------------------------------------------------------
    // Retry Delay Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_retry_delay_network_error() {
        let err = Error::network_error("test");
        assert_eq!(err.retry_delay_secs(), Some(3));
    }

    #[test]
    fn test_retry_delay_timeout_error() {
        let err = Error::Download(DownloadError::Timeout {
            title: "test".to_string(),
            timeout_secs: 30,
        });
        assert_eq!(err.retry_delay_secs(), Some(5));
    }

    #[test]
    fn test_retry_delay_non_retryable() {
        let err = Error::playlist_not_found("test");
        assert_eq!(err.retry_delay_secs(), None);
    }

    #[test]
    fn test_retry_delay_with_context() {
        let inner = Error::Download(DownloadError::RateLimited {
            retry_after_secs: 120,
        });
        let err = Error::WithContext {
            context: "during sync".to_string(),
            source: Box::new(inner),
        };
        assert_eq!(err.retry_delay_secs(), Some(120));
    }

    // -------------------------------------------------------------------------
    // Retryable IO Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_retryable_io_error_timed_out() {
        let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
        let err: Error = io_err.into();
        assert!(err.is_retryable());
    }

    #[test]
    fn test_retryable_io_error_connection_reset() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset");
        let err: Error = io_err.into();
        assert!(err.is_retryable());
    }

    #[test]
    fn test_not_retryable_io_error_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err: Error = io_err.into();
        assert!(!err.is_retryable());
    }

    // -------------------------------------------------------------------------
    // Path Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_path_error_debug() {
        let path_err = PathError::new("/test/path", "test message");
        assert_eq!(path_err.path, PathBuf::from("/test/path"));
        assert_eq!(path_err.message, "test message");
    }

    // -------------------------------------------------------------------------
    // Legacy Error Tests
    // -------------------------------------------------------------------------

    #[test]
    #[allow(deprecated)]
    fn test_legacy_device_not_mounted() {
        let err = Error::DeviceNotMounted("USB".to_string());
        assert!(err.to_string().contains("Device not mounted"));
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_playlist_already_exists() {
        let err = Error::PlaylistAlreadyExists("My Playlist".to_string());
        assert!(err.to_string().contains("already exists"));
        assert_eq!(err.kind(), ErrorKind::Playlist);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_invalid_playlist_name() {
        let err = Error::InvalidPlaylistName("bad/name".to_string());
        assert!(err.to_string().contains("Invalid playlist name"));
        assert_eq!(err.kind(), ErrorKind::Playlist);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_invalid_youtube_url() {
        let err = Error::InvalidYouTubeUrl("not-a-url".to_string());
        assert!(err.to_string().contains("Invalid YouTube URL"));
        assert_eq!(err.kind(), ErrorKind::Download);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_not_a_playlist() {
        let err = Error::NotAPlaylist("single-video-url".to_string());
        assert!(err.to_string().contains("not a YouTube playlist"));
        assert_eq!(err.kind(), ErrorKind::Download);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_download_failed() {
        let err = Error::DownloadFailed("network error".to_string());
        assert!(err.to_string().contains("download failed"));
        assert_eq!(err.kind(), ErrorKind::Download);
    }

    #[test]
    #[allow(deprecated)]
    fn test_legacy_sync_failed() {
        let err = Error::SyncFailed("device disconnected".to_string());
        assert!(err.to_string().contains("Sync failed"));
        assert_eq!(err.kind(), ErrorKind::Transfer);
    }

    // -------------------------------------------------------------------------
    // Error Context Edge Cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_deeply_nested_context() {
        let inner = Error::playlist_not_found("test");
        let ctx1 = Error::WithContext {
            context: "level 1".to_string(),
            source: Box::new(inner),
        };
        let ctx2 = Error::WithContext {
            context: "level 2".to_string(),
            source: Box::new(ctx1),
        };
        let ctx3 = Error::WithContext {
            context: "level 3".to_string(),
            source: Box::new(ctx2),
        };

        // Kind should still propagate through all levels
        assert_eq!(ctx3.kind(), ErrorKind::Playlist);
    }

    #[test]
    fn test_context_with_retryable_error() {
        let inner = Error::Device(DeviceError::Disconnected {
            name: "USB".to_string(),
        });
        let with_ctx = Error::WithContext {
            context: "during transfer".to_string(),
            source: Box::new(inner),
        };

        assert!(with_ctx.is_retryable());
        assert!(with_ctx.is_user_facing());
    }
}
