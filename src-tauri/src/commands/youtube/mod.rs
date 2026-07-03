//! `YouTube` URL validation and download commands.

use tokio::task::spawn_blocking;
use tracing::{debug, info};
use youtun4_core::Error;
use youtun4_core::youtube::{
    DownloadProgress, DownloadStatus, PlaylistInfo, RustyYtdlDownloader, YouTubeDownloader,
    YouTubeUrlValidation, validate_youtube_url,
};

use crate::runtime::TaskId;

use super::error::map_err;

/// Serializable download progress for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadProgressPayload {
    pub task_id: TaskId,
    pub current_index: usize,
    pub total_videos: usize,
    pub current_title: String,
    pub current_progress: f64,
    pub overall_progress: f64,
    pub status: String,
    pub current_bytes: u64,
    pub current_total_bytes: Option<u64>,
    pub total_bytes_downloaded: u64,
    pub download_speed_bps: f64,
    pub formatted_speed: String,
    pub estimated_remaining_secs: Option<f64>,
    pub formatted_eta: Option<String>,
    pub elapsed_secs: f64,
    pub formatted_elapsed: String,
    pub videos_completed: usize,
    pub videos_skipped: usize,
    pub videos_failed: usize,
}

impl DownloadProgressPayload {
    pub fn from_progress(task_id: TaskId, progress: &DownloadProgress) -> Self {
        Self {
            task_id,
            current_index: progress.current_index,
            total_videos: progress.total_videos,
            current_title: progress.current_title.clone(),
            current_progress: progress.current_progress,
            overall_progress: progress.overall_progress,
            status: match &progress.status {
                DownloadStatus::Starting => "starting".to_string(),
                DownloadStatus::Downloading => "downloading".to_string(),
                DownloadStatus::Converting => "converting".to_string(),
                DownloadStatus::Completed => "completed".to_string(),
                DownloadStatus::Failed(msg) => format!("failed: {msg}"),
                DownloadStatus::Skipped => "skipped".to_string(),
            },
            current_bytes: progress.current_bytes,
            current_total_bytes: progress.current_total_bytes,
            total_bytes_downloaded: progress.total_bytes_downloaded,
            download_speed_bps: progress.download_speed_bps,
            formatted_speed: progress.formatted_speed(),
            estimated_remaining_secs: progress.estimated_remaining_secs,
            formatted_eta: progress.formatted_eta(),
            elapsed_secs: progress.elapsed_secs,
            formatted_elapsed: progress.formatted_elapsed(),
            videos_completed: progress.videos_completed,
            videos_skipped: progress.videos_skipped,
            videos_failed: progress.videos_failed,
        }
    }
}

/// Category of YouTube-related errors for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YouTubeErrorCategory {
    Network,
    YouTubeService,
    InvalidUrl,
    PlaylistNotFound,
    VideoUnavailable,
    AgeRestricted,
    GeoRestricted,
    AudioExtraction,
    FileSystem,
    Cancelled,
    Unknown,
}

impl YouTubeErrorCategory {
    #[must_use]
    pub const fn title(self) -> &'static str {
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

    #[must_use]
    pub const fn description(self) -> &'static str {
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
}

/// Classify an error into a category for user display.
#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "catch-all for non-download/filesystem errors"
)]
pub fn classify_error(error: &Error) -> YouTubeErrorCategory {
    match error {
        Error::Download(download_err) => {
            use youtun4_core::error::DownloadError;
            match download_err {
                DownloadError::InvalidUrl { .. } => YouTubeErrorCategory::InvalidUrl,
                DownloadError::NotAPlaylist { .. } => YouTubeErrorCategory::InvalidUrl,
                DownloadError::Network { .. } => YouTubeErrorCategory::Network,
                DownloadError::Timeout { .. } => YouTubeErrorCategory::Network,
                DownloadError::RateLimited { .. } => YouTubeErrorCategory::YouTubeService,
                DownloadError::VideoUnavailable { reason, .. } => {
                    let reason_lower = reason.to_lowercase();
                    if reason_lower.contains("age") || reason_lower.contains("sign in") {
                        YouTubeErrorCategory::AgeRestricted
                    } else if reason_lower.contains("country")
                        || reason_lower.contains("region")
                        || reason_lower.contains("geo")
                    {
                        YouTubeErrorCategory::GeoRestricted
                    } else {
                        YouTubeErrorCategory::VideoUnavailable
                    }
                }
                DownloadError::AudioExtractionFailed { .. } => {
                    YouTubeErrorCategory::AudioExtraction
                }
                DownloadError::ConversionFailed { .. } => YouTubeErrorCategory::AudioExtraction,
                DownloadError::PlaylistParseFailed { reason, .. } => {
                    let reason_lower = reason.to_lowercase();
                    if reason_lower.contains("not found")
                        || reason_lower.contains("404")
                        || reason_lower.contains("private")
                    {
                        YouTubeErrorCategory::PlaylistNotFound
                    } else if reason_lower.contains("network")
                        || reason_lower.contains("connection")
                        || reason_lower.contains("timeout")
                    {
                        YouTubeErrorCategory::Network
                    } else {
                        YouTubeErrorCategory::YouTubeService
                    }
                }
                DownloadError::Cancelled => YouTubeErrorCategory::Cancelled,
            }
        }
        Error::FileSystem(_) => YouTubeErrorCategory::FileSystem,
        _ => YouTubeErrorCategory::Unknown,
    }
}

/// Download result payload for completion events.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadResultPayload {
    pub task_id: TaskId,
    pub success: bool,
    pub successful_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub total_count: usize,
    pub results: Vec<VideoDownloadResult>,
    pub error_message: Option<String>,
    pub error_category: Option<YouTubeErrorCategory>,
    pub error_title: Option<String>,
    pub error_description: Option<String>,
}

/// Result of downloading a single video.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VideoDownloadResult {
    pub video_id: String,
    pub title: String,
    pub success: bool,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

/// Validate a `YouTube` URL and extract playlist information.
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "tauri::command requires owned values from IPC"
)]
pub fn validate_youtube_playlist_url(url: String) -> YouTubeUrlValidation {
    debug!("Validating YouTube URL: {}", url);
    let result = validate_youtube_url(&url);

    if result.is_valid {
        info!(
            "URL validation succeeded: playlist_id={:?}, type={:?}",
            result.playlist_id, result.url_type
        );
    } else {
        info!("URL validation failed: {:?}", result.error_message);
    }

    result
}

/// Check if a URL is a valid `YouTube` playlist URL.
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "tauri::command requires owned values from IPC"
)]
pub fn is_valid_youtube_playlist_url(url: String) -> bool {
    let result = validate_youtube_url(&url);
    result.is_valid
}

/// Extract the playlist ID from a `YouTube` URL.
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "tauri::command requires owned values from IPC"
)]
pub fn extract_youtube_playlist_id(url: String) -> std::result::Result<String, String> {
    debug!("Extracting playlist ID from URL: {}", url);
    let result = validate_youtube_url(&url);

    if result.is_valid {
        #[allow(clippy::unwrap_used, reason = "guarded by is_valid check above")]
        let playlist_id = result.playlist_id.unwrap();
        info!("Extracted playlist ID: {}", playlist_id);
        Ok(playlist_id)
    } else {
        let error_message = result
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string());
        info!("Failed to extract playlist ID: {}", error_message);
        Err(error_message)
    }
}

/// Check if the downloader is available.
#[tauri::command]
#[allow(
    clippy::unnecessary_wraps,
    reason = "tauri::command requires Result return type for IPC compatibility"
)]
pub fn check_yt_dlp_available() -> std::result::Result<String, String> {
    info!("Checking downloader availability (pure Rust - always available)");
    Ok("rusty_ytdl (pure Rust)".to_string())
}

/// Fetch playlist information from a `YouTube` URL.
#[tauri::command]
pub async fn fetch_youtube_playlist_info(url: String) -> std::result::Result<PlaylistInfo, String> {
    info!("Fetching playlist info for URL: {}", url);

    let result = spawn_blocking(move || {
        let downloader = RustyYtdlDownloader::new();
        downloader.parse_playlist_url(&url)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    result.map_err(map_err)
}

mod download;
mod metadata;

pub use download::*;
