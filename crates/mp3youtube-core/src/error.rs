//! Error types for MP3YouTube core operations.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using the crate's Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in MP3YouTube core operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Device not found or not connected.
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Device is not mounted or accessible.
    #[error("Device not mounted: {0}")]
    DeviceNotMounted(String),

    /// Playlist already exists.
    #[error("Playlist already exists: {0}")]
    PlaylistAlreadyExists(String),

    /// Playlist not found.
    #[error("Playlist not found: {0}")]
    PlaylistNotFound(String),

    /// Invalid playlist name.
    #[error("Invalid playlist name: {0}")]
    InvalidPlaylistName(String),

    /// Invalid YouTube URL.
    #[error("Invalid YouTube URL: {0}")]
    InvalidYouTubeUrl(String),

    /// YouTube URL is not a playlist.
    #[error("URL is not a YouTube playlist: {0}")]
    NotAPlaylist(String),

    /// YouTube download failed.
    #[error("YouTube download failed: {0}")]
    DownloadFailed(String),

    /// File system operation failed.
    #[error("File system error at {path}: {message}")]
    FileSystem {
        /// Path where the error occurred.
        path: PathBuf,
        /// Error message.
        message: String,
    },

    /// Sync operation failed.
    #[error("Sync failed: {0}")]
    SyncFailed(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// IO error wrapper.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::DeviceNotFound("test-device".to_string());
        assert_eq!(err.to_string(), "Device not found: test-device");
    }

    #[test]
    fn test_playlist_not_found_display() {
        let err = Error::PlaylistNotFound("my-playlist".to_string());
        assert_eq!(err.to_string(), "Playlist not found: my-playlist");
    }

    #[test]
    fn test_file_system_error_display() {
        let err = Error::FileSystem {
            path: PathBuf::from("/test/path"),
            message: "permission denied".to_string(),
        };
        assert!(err.to_string().contains("/test/path"));
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }
}
