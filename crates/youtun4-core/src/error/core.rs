use std::path::PathBuf;
use thiserror::Error;

use super::{
    cache::CacheError, device::DeviceError, download::DownloadError, filesystem::FileSystemError,
    playlist::PlaylistError, transfer::TransferError,
};
use crate::error::Result;

// ============================================================================
// Main Error Enum
// ============================================================================

/// Errors that can occur in `Youtun4` core operations.
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
/// use youtun4_core::error::{Result, ErrorContext};
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
            #[allow(deprecated, reason = "legacy variant support")]
            Self::DeviceNotFound(_) | Self::DeviceNotMounted(_) => ErrorKind::Device,
            #[allow(deprecated, reason = "legacy variant support")]
            Self::PlaylistAlreadyExists(_)
            | Self::PlaylistNotFound(_)
            | Self::InvalidPlaylistName(_) => ErrorKind::Playlist,
            #[allow(deprecated, reason = "legacy variant support")]
            Self::InvalidYouTubeUrl(_) | Self::NotAPlaylist(_) | Self::DownloadFailed(_) => {
                ErrorKind::Download
            }
            #[allow(deprecated, reason = "legacy variant support")]
            Self::SyncFailed(_) => ErrorKind::Transfer,
        }
    }

    /// Check if this error is retryable.
    #[must_use]
    #[allow(
        deprecated,
        reason = "matching deprecated variants for exhaustiveness until they are removed"
    )]
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
            Self::Device(_)
            | Self::Download(_)
            | Self::Transfer(_)
            | Self::Playlist(_)
            | Self::FileSystem(_)
            | Self::Cache(_)
            | Self::DeviceNotFound(_)
            | Self::DeviceNotMounted(_)
            | Self::PlaylistAlreadyExists(_)
            | Self::PlaylistNotFound(_)
            | Self::InvalidPlaylistName(_)
            | Self::InvalidYouTubeUrl(_)
            | Self::NotAPlaylist(_)
            | Self::DownloadFailed(_)
            | Self::SyncFailed(_)
            | Self::Configuration(_)
            | Self::Serialization(_)
            | Self::Cancelled
            | Self::Internal(_) => false,
        }
    }

    /// Check if this is a user-facing error (should be shown to user).
    #[must_use]
    pub const fn is_user_facing(&self) -> bool {
        !matches!(self, Self::Internal(_))
    }

    /// Get the retry delay in seconds, if applicable.
    #[must_use]
    #[allow(
        deprecated,
        reason = "matching deprecated variants for exhaustiveness until they are removed"
    )]
    pub fn retry_delay_secs(&self) -> Option<u64> {
        match self {
            Self::Download(DownloadError::RateLimited { retry_after_secs }) => {
                Some(*retry_after_secs)
            }
            Self::Download(DownloadError::Timeout { .. }) => Some(5),
            Self::Download(DownloadError::Network { .. }) => Some(3),
            Self::WithContext { source, .. } => source.retry_delay_secs(),
            Self::Device(_)
            | Self::Download(_)
            | Self::Transfer(_)
            | Self::Playlist(_)
            | Self::FileSystem(_)
            | Self::Cache(_)
            | Self::DeviceNotFound(_)
            | Self::DeviceNotMounted(_)
            | Self::PlaylistAlreadyExists(_)
            | Self::PlaylistNotFound(_)
            | Self::InvalidPlaylistName(_)
            | Self::InvalidYouTubeUrl(_)
            | Self::NotAPlaylist(_)
            | Self::DownloadFailed(_)
            | Self::SyncFailed(_)
            | Self::Configuration(_)
            | Self::Io(_)
            | Self::Serialization(_)
            | Self::Cancelled
            | Self::Internal(_) => None,
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;

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
    #[allow(deprecated, reason = "testing legacy variants")]
    fn test_legacy_device_not_found() {
        let err = Error::DeviceNotFound("test".to_string());
        assert_eq!(err.to_string(), "Device not found: test");
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    #[allow(deprecated, reason = "testing legacy variants")]
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

    // =============================================================================
    // Additional Error Tests - Edge Cases and Complete Coverage
    // =============================================================================

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
    // Legacy Error Tests
    // -------------------------------------------------------------------------

    #[test]
    #[allow(deprecated, reason = "testing legacy variants")]
    fn test_legacy_device_not_mounted() {
        let err = Error::DeviceNotMounted("USB".to_string());
        assert!(err.to_string().contains("Device not mounted"));
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    #[allow(deprecated, reason = "testing legacy variants")]
    fn test_legacy_playlist_already_exists() {
        let err = Error::PlaylistAlreadyExists("My Playlist".to_string());
        assert!(err.to_string().contains("already exists"));
        assert_eq!(err.kind(), ErrorKind::Playlist);
    }

    #[test]
    #[allow(deprecated, reason = "testing legacy variants")]
    fn test_legacy_invalid_playlist_name() {
        let err = Error::InvalidPlaylistName("bad/name".to_string());
        assert!(err.to_string().contains("Invalid playlist name"));
        assert_eq!(err.kind(), ErrorKind::Playlist);
    }

    #[test]
    #[allow(deprecated, reason = "testing legacy variants")]
    fn test_legacy_invalid_youtube_url() {
        let err = Error::InvalidYouTubeUrl("not-a-url".to_string());
        assert!(err.to_string().contains("Invalid YouTube URL"));
        assert_eq!(err.kind(), ErrorKind::Download);
    }

    #[test]
    #[allow(deprecated, reason = "testing legacy variants")]
    fn test_legacy_not_a_playlist() {
        let err = Error::NotAPlaylist("single-video-url".to_string());
        assert!(err.to_string().contains("not a YouTube playlist"));
        assert_eq!(err.kind(), ErrorKind::Download);
    }

    #[test]
    #[allow(deprecated, reason = "testing legacy variants")]
    fn test_legacy_download_failed() {
        let err = Error::DownloadFailed("network error".to_string());
        assert!(err.to_string().contains("download failed"));
        assert_eq!(err.kind(), ErrorKind::Download);
    }

    #[test]
    #[allow(deprecated, reason = "testing legacy variants")]
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
