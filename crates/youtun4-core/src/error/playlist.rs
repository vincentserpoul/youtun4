use thiserror::Error;

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
}
