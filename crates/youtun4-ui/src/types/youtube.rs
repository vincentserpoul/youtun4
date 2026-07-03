use serde::{Deserialize, Serialize};

use super::task::TaskId;

// =============================================================================
// YouTube URL Validation Types
// =============================================================================

/// Type of `YouTube` URL detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum YouTubeUrlType {
    /// Standard playlist URL (youtube.com/playlist?list=...)
    Playlist,
    /// Watch URL with playlist parameter (youtube.com/watch?v=...&list=...)
    WatchWithPlaylist,
    /// Single video URL without playlist
    SingleVideo,
    /// Short URL (youtu.be/...)
    ShortUrl,
    /// Invalid or unrecognized URL
    #[default]
    Invalid,
}

impl std::fmt::Display for YouTubeUrlType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Playlist => write!(f, "Playlist"),
            Self::WatchWithPlaylist => write!(f, "Watch with Playlist"),
            Self::SingleVideo => write!(f, "Single Video"),
            Self::ShortUrl => write!(f, "Short URL"),
            Self::Invalid => write!(f, "Invalid"),
        }
    }
}

/// Result of `YouTube` URL validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeUrlValidation {
    /// Whether the URL is valid.
    pub is_valid: bool,
    /// The extracted playlist ID (if valid).
    pub playlist_id: Option<String>,
    /// The normalized/canonical URL.
    pub normalized_url: Option<String>,
    /// Error message if validation failed.
    pub error_message: Option<String>,
    /// The URL type detected.
    pub url_type: YouTubeUrlType,
}

impl YouTubeUrlValidation {
    /// Create a placeholder for pending validation.
    #[must_use]
    pub const fn pending() -> Self {
        Self {
            is_valid: false,
            playlist_id: None,
            normalized_url: None,
            error_message: None,
            url_type: YouTubeUrlType::Invalid,
        }
    }

    /// Check if this is a playlist URL (either standard or watch with playlist).
    #[must_use]
    pub const fn is_playlist_url(&self) -> bool {
        matches!(
            self.url_type,
            YouTubeUrlType::Playlist | YouTubeUrlType::WatchWithPlaylist
        )
    }
}

// =============================================================================
// YouTube Download Progress Types
// =============================================================================

/// Status of a download operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DownloadStatus {
    /// Download is starting.
    #[default]
    Starting,
    /// Actively downloading.
    Downloading,
    /// Converting audio.
    Converting,
    /// Download completed successfully.
    Completed,
    /// Download failed with error message.
    Failed(String),
    /// Download was skipped (file already exists).
    Skipped,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "Starting"),
            Self::Downloading => write!(f, "Downloading"),
            Self::Converting => write!(f, "Converting"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed(msg) => write!(f, "Failed: {msg}"),
            Self::Skipped => write!(f, "Skipped"),
        }
    }
}

/// Progress information for a `YouTube` download operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Task ID for this download operation.
    pub task_id: TaskId,
    /// Current video index (1-based).
    pub current_index: usize,
    /// Total number of videos.
    pub total_videos: usize,
    /// Current video title.
    pub current_title: String,
    /// Download progress for current video (0.0 - 1.0).
    pub current_progress: f64,
    /// Overall progress (0.0 - 1.0).
    pub overall_progress: f64,
    /// Status message.
    pub status: String,
    /// Bytes downloaded for the current file.
    pub current_bytes: u64,
    /// Total bytes for the current file (if known).
    pub current_total_bytes: Option<u64>,
    /// Total bytes downloaded across all files.
    pub total_bytes_downloaded: u64,
    /// Download speed in bytes per second.
    pub download_speed_bps: f64,
    /// Formatted download speed (e.g., "1.5 MB/s").
    pub formatted_speed: String,
    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,
    /// Formatted estimated time remaining (e.g., "2:30").
    pub formatted_eta: Option<String>,
    /// Elapsed time in seconds since download started.
    pub elapsed_secs: f64,
    /// Formatted elapsed time (e.g., "1:15").
    pub formatted_elapsed: String,
    /// Number of videos completed successfully.
    pub videos_completed: usize,
    /// Number of videos skipped (already exist).
    pub videos_skipped: usize,
    /// Number of videos that failed.
    pub videos_failed: usize,
}

impl DownloadProgress {
    /// Calculate the overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    #[allow(clippy::float_arithmetic, reason = "acceptable for display formatting")]
    pub fn overall_progress_percent(&self) -> f64 {
        self.overall_progress * 100.0
    }

    /// Calculate the current video progress as a percentage (0.0 - 100.0).
    #[must_use]
    #[allow(clippy::float_arithmetic, reason = "acceptable for display formatting")]
    pub fn current_progress_percent(&self) -> f64 {
        self.current_progress * 100.0
    }

    /// Check if the download is actively in progress.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self.status.as_str(),
            "downloading" | "converting" | "starting"
        )
    }

    /// Check if the download has completed (successfully or with errors).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "cancelled")
            || self.status.starts_with("failed")
    }
}

/// Category of YouTube-related errors for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum YouTubeErrorCategory {
    /// Network connection issues (no internet, DNS failure, timeout).
    Network,
    /// `YouTube` service issues (rate limiting, service unavailable).
    YouTubeService,
    /// Invalid or malformed URL.
    InvalidUrl,
    /// Playlist not found or is private.
    PlaylistNotFound,
    /// Video is unavailable (private, deleted, region-locked).
    VideoUnavailable,
    /// Age-restricted content requiring authentication.
    AgeRestricted,
    /// Geographic restriction on content.
    GeoRestricted,
    /// Failed to extract or download audio stream.
    AudioExtraction,
    /// File system error (disk full, permission denied).
    FileSystem,
    /// Operation was cancelled by user.
    Cancelled,
    /// Unknown or unclassified error.
    #[default]
    Unknown,
}

impl YouTubeErrorCategory {
    /// Get a user-friendly title for this error category.
    #[must_use]
    pub const fn title(&self) -> &'static str {
        match self {
            Self::Network => "Network Error",
            Self::YouTubeService => "YouTube Service Error",
            Self::InvalidUrl => "Invalid URL",
            Self::PlaylistNotFound => "Playlist Not Found",
            Self::VideoUnavailable => "Video Unavailable",
            Self::AgeRestricted => "Age-Restricted Content",
            Self::GeoRestricted => "Geographic Restriction",
            Self::AudioExtraction => "Audio Extraction Failed",
            Self::FileSystem => "File System Error",
            Self::Cancelled => "Cancelled",
            Self::Unknown => "Error",
        }
    }

    /// Get a user-friendly description for this error category.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Network => {
                "Could not connect to YouTube. Please check your internet connection and try again."
            }
            Self::YouTubeService => {
                "YouTube is temporarily unavailable or has rate-limited requests. Please wait a moment and try again."
            }
            Self::InvalidUrl => "The provided URL is not a valid YouTube playlist URL.",
            Self::PlaylistNotFound => {
                "The playlist could not be found. It may be private, deleted, or the URL is incorrect."
            }
            Self::VideoUnavailable => {
                "One or more videos in the playlist are unavailable (private, deleted, or restricted)."
            }
            Self::AgeRestricted => {
                "This content is age-restricted and requires authentication to access."
            }
            Self::GeoRestricted => "This content is not available in your geographic region.",
            Self::AudioExtraction => {
                "Failed to extract audio from the video. The format may not be supported."
            }
            Self::FileSystem => "Could not save the file. Please check disk space and permissions.",
            Self::Cancelled => "The download was cancelled.",
            Self::Unknown => "An unexpected error occurred.",
        }
    }

    /// Check if this error category is retryable.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network | Self::YouTubeService | Self::AudioExtraction
        )
    }
}

impl std::fmt::Display for YouTubeErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title())
    }
}

/// Result of a download operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    /// Task ID for this download operation.
    pub task_id: TaskId,
    /// Whether the overall download was successful.
    pub success: bool,
    /// Number of videos successfully downloaded.
    pub successful_count: usize,
    /// Number of videos that failed.
    pub failed_count: usize,
    /// Number of videos skipped (already exist).
    pub skipped_count: usize,
    /// Total number of videos in the playlist.
    pub total_count: usize,
    /// Individual video results.
    pub results: Vec<VideoDownloadResult>,
    /// Error message if the overall operation failed.
    pub error_message: Option<String>,
    /// Category of error for UI display (if failed).
    pub error_category: Option<YouTubeErrorCategory>,
    /// User-friendly error title.
    pub error_title: Option<String>,
    /// User-friendly error description with suggested action.
    pub error_description: Option<String>,
}

impl DownloadResult {
    /// Get the display error message, preferring the user-friendly description.
    #[must_use]
    pub fn display_error(&self) -> Option<String> {
        if let Some(ref desc) = self.error_description {
            Some(desc.clone())
        } else {
            self.error_message.clone()
        }
    }

    /// Get the error title for display.
    #[must_use]
    pub fn display_error_title(&self) -> String {
        self.error_title
            .clone()
            .or_else(|| self.error_category.map(|c| c.title().to_string()))
            .unwrap_or_else(|| "Download Failed".to_string())
    }

    /// Check if the error is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        self.error_category.is_some_and(|c| c.is_retryable())
    }
}

/// Result of downloading a single video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDownloadResult {
    /// Video ID.
    pub video_id: String,
    /// Video title.
    pub title: String,
    /// Whether the download was successful.
    pub success: bool,
    /// Output file path (if successful).
    pub output_path: Option<String>,
    /// Error message (if failed).
    pub error: Option<String>,
}
