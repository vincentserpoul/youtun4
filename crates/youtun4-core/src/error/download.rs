use thiserror::Error;

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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;
    use crate::error::Error;

    #[test]
    fn test_invalid_youtube_url_error() {
        let err = Error::invalid_youtube_url("http://example.com", "not a YouTube URL");
        assert!(err.to_string().contains("http://example.com"));
        assert!(err.to_string().contains("not a YouTube URL"));
        assert_eq!(err.kind(), crate::error::ErrorKind::Download);
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
}
