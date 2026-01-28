//! YouTube playlist downloading module.
//!
//! Handles downloading audio from YouTube playlists and converting to MP3.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{Error, Result};

/// Information about a YouTube video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    /// Video ID.
    pub id: String,
    /// Video title.
    pub title: String,
    /// Video duration in seconds.
    pub duration_secs: Option<u64>,
    /// Channel/uploader name.
    pub channel: Option<String>,
}

/// Information about a YouTube playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistInfo {
    /// Playlist ID.
    pub id: String,
    /// Playlist title.
    pub title: String,
    /// Number of videos in the playlist.
    pub video_count: usize,
    /// Videos in the playlist.
    pub videos: Vec<VideoInfo>,
}

/// Progress callback for download operations.
pub type ProgressCallback = Box<dyn Fn(DownloadProgress) + Send + Sync>;

/// Download progress information.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
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
    pub status: DownloadStatus,
}

/// Download status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    /// Starting download.
    Starting,
    /// Downloading audio.
    Downloading,
    /// Converting to MP3.
    Converting,
    /// Completed successfully.
    Completed,
    /// Failed with error message.
    Failed(String),
    /// Skipped (e.g., already exists).
    Skipped,
}

/// YouTube downloader trait for testability.
#[cfg_attr(test, mockall::automock)]
pub trait YouTubeDownloader: Send + Sync {
    /// Parse a YouTube URL and extract playlist information.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or not a playlist.
    fn parse_playlist_url(&self, url: &str) -> Result<PlaylistInfo>;

    /// Download all videos from a playlist as MP3 files.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails.
    fn download_playlist(
        &self,
        playlist_info: &PlaylistInfo,
        output_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<Vec<DownloadResult>>;
}

/// Result of downloading a single video.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// Video info.
    pub video: VideoInfo,
    /// Whether the download was successful.
    pub success: bool,
    /// Output file path (if successful).
    pub output_path: Option<std::path::PathBuf>,
    /// Error message (if failed).
    pub error: Option<String>,
}

/// Parse a YouTube playlist URL and extract the playlist ID.
///
/// Supports the following URL formats:
/// - `https://www.youtube.com/playlist?list=PLxxxxxxxx`
/// - `https://youtube.com/playlist?list=PLxxxxxxxx`
/// - `https://www.youtube.com/watch?v=xxxxx&list=PLxxxxxxxx`
///
/// # Errors
///
/// Returns an error if the URL is not a valid YouTube playlist URL.
pub fn extract_playlist_id(url: &str) -> Result<String> {
    // Basic URL validation
    if !url.contains("youtube.com") && !url.contains("youtu.be") {
        return Err(Error::InvalidYouTubeUrl(
            "URL must be a YouTube URL".to_string(),
        ));
    }

    // Try to find list= parameter
    let url_lower = url.to_lowercase();
    if let Some(list_pos) = url_lower.find("list=") {
        let start = list_pos + 5;
        let rest = &url[start..];

        // Extract until next & or end of string
        let end = rest.find('&').unwrap_or(rest.len());
        let playlist_id = &rest[..end];

        if playlist_id.is_empty() {
            return Err(Error::NotAPlaylist(
                "Empty playlist ID in URL".to_string(),
            ));
        }

        return Ok(playlist_id.to_string());
    }

    Err(Error::NotAPlaylist(
        "URL does not contain a playlist ID".to_string(),
    ))
}

/// Sanitize a string for use as a filename.
#[must_use]
pub fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];

    let sanitized: String = name
        .chars()
        .map(|c| {
            if invalid_chars.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Trim whitespace and dots from ends
    let trimmed = sanitized.trim().trim_matches('.');

    // Limit length (leaving room for extension)
    if trimmed.len() > 200 {
        trimmed[..200].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Default YouTube downloader implementation.
/// Note: This is a placeholder that will need a proper YouTube downloading library.
pub struct DefaultYouTubeDownloader;

impl DefaultYouTubeDownloader {
    /// Create a new downloader.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for DefaultYouTubeDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTubeDownloader for DefaultYouTubeDownloader {
    fn parse_playlist_url(&self, url: &str) -> Result<PlaylistInfo> {
        let playlist_id = extract_playlist_id(url)?;

        // TODO: Implement actual YouTube API/scraping to get playlist info
        // For now, return a placeholder that will be implemented with rustube or similar
        info!("Parsing playlist: {}", playlist_id);

        Ok(PlaylistInfo {
            id: playlist_id,
            title: "Placeholder".to_string(),
            video_count: 0,
            videos: vec![],
        })
    }

    fn download_playlist(
        &self,
        playlist_info: &PlaylistInfo,
        output_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<Vec<DownloadResult>> {
        info!(
            "Downloading playlist '{}' to {}",
            playlist_info.title,
            output_dir.display()
        );

        let mut results = Vec::new();

        for (index, video) in playlist_info.videos.iter().enumerate() {
            let current_progress = DownloadProgress {
                current_index: index + 1,
                total_videos: playlist_info.videos.len(),
                current_title: video.title.clone(),
                current_progress: 0.0,
                overall_progress: index as f64 / playlist_info.videos.len() as f64,
                status: DownloadStatus::Starting,
            };

            if let Some(ref callback) = progress {
                callback(current_progress);
            }

            // TODO: Implement actual download logic with rustube
            debug!("Would download: {} ({})", video.title, video.id);

            let filename = format!("{}.mp3", sanitize_filename(&video.title));
            let output_path = output_dir.join(&filename);

            // Placeholder result
            results.push(DownloadResult {
                video: video.clone(),
                success: false,
                output_path: Some(output_path),
                error: Some("Download not yet implemented".to_string()),
            });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_playlist_id_standard_url() {
        let url = "https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf";
        let result = extract_playlist_id(url);
        assert!(result.is_ok());
        assert_eq!(
            result.expect("Should have ID"),
            "PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf"
        );
    }

    #[test]
    fn test_extract_playlist_id_watch_url_with_list() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOtest";
        let result = extract_playlist_id(url);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should have ID"), "PLrAXtmErZgOtest");
    }

    #[test]
    fn test_extract_playlist_id_no_playlist() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        let result = extract_playlist_id(url);
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::NotAPlaylist(_))));
    }

    #[test]
    fn test_extract_playlist_id_not_youtube() {
        let url = "https://www.vimeo.com/video/123";
        let result = extract_playlist_id(url);
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::InvalidYouTubeUrl(_))));
    }

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(sanitize_filename("Hello World"), "Hello World");
    }

    #[test]
    fn test_sanitize_filename_invalid_chars() {
        assert_eq!(sanitize_filename("Hello/World"), "Hello_World");
        assert_eq!(sanitize_filename("Test:File"), "Test_File");
        assert_eq!(sanitize_filename("A*B?C"), "A_B_C");
    }

    #[test]
    fn test_sanitize_filename_trim() {
        assert_eq!(sanitize_filename("  Hello  "), "Hello");
        assert_eq!(sanitize_filename("...test..."), "test");
    }

    #[test]
    fn test_sanitize_filename_long_name() {
        let long_name = "a".repeat(300);
        let result = sanitize_filename(&long_name);
        assert_eq!(result.len(), 200);
    }

    #[test]
    fn test_download_status_equality() {
        assert_eq!(DownloadStatus::Starting, DownloadStatus::Starting);
        assert_eq!(
            DownloadStatus::Failed("error".to_string()),
            DownloadStatus::Failed("error".to_string())
        );
        assert_ne!(DownloadStatus::Starting, DownloadStatus::Downloading);
    }

    #[test]
    fn test_default_downloader_creation() {
        let downloader = DefaultYouTubeDownloader::new();
        let result = downloader.parse_playlist_url(
            "https://www.youtube.com/playlist?list=PLtest123",
        );
        assert!(result.is_ok());
    }
}
