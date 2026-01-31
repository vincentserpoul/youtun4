//! `YouTube` playlist downloading module.
//!
//! Handles downloading audio from `YouTube` playlists.
//! Uses pure Rust libraries - no external dependencies like yt-dlp required.
//!
//! # Pure Rust Implementation
//!
//! This module uses `rusty_ytdl` for `YouTube` video downloading, which is a
//! pure Rust implementation that doesn't require any external tools.
//!
//! ## Quality Settings
//!
//! Downloads are configured to:
//! - Extract the best audio stream available
//! - Save as the original format (usually m4a/webm)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use mp3youtube_core::youtube::{RustyYtdlDownloader, YouTubeDownloader};
//! use std::path::Path;
//!
//! let downloader = RustyYtdlDownloader::new();
//! let playlist = downloader.parse_playlist_url("https://www.youtube.com/playlist?list=PLtest").unwrap();
//! let results = downloader.download_playlist(&playlist, Path::new("/tmp/music"), None).unwrap();
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use regex::Regex;
use rusty_ytdl::{Video, VideoOptions, VideoQuality, VideoSearchOptions};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::error::{DownloadError, Error, Result};

/// Information about a `YouTube` video.
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
    /// Thumbnail URL for the video.
    pub thumbnail_url: Option<String>,
}

/// Information about a `YouTube` playlist.
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
    /// Thumbnail URL for the playlist (or first video's thumbnail).
    pub thumbnail_url: Option<String>,
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
    /// Bytes downloaded for the current file.
    pub current_bytes: u64,
    /// Total bytes for the current file (if known).
    pub current_total_bytes: Option<u64>,
    /// Total bytes downloaded across all files.
    pub total_bytes_downloaded: u64,
    /// Download speed in bytes per second.
    pub download_speed_bps: f64,
    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,
    /// Elapsed time in seconds since download started.
    pub elapsed_secs: f64,
    /// Number of videos completed successfully.
    pub videos_completed: usize,
    /// Number of videos skipped (already exist).
    pub videos_skipped: usize,
    /// Number of videos that failed.
    pub videos_failed: usize,
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self {
            current_index: 0,
            total_videos: 0,
            current_title: String::new(),
            current_progress: 0.0,
            overall_progress: 0.0,
            status: DownloadStatus::Starting,
            current_bytes: 0,
            current_total_bytes: None,
            total_bytes_downloaded: 0,
            download_speed_bps: 0.0,
            estimated_remaining_secs: None,
            elapsed_secs: 0.0,
            videos_completed: 0,
            videos_skipped: 0,
            videos_failed: 0,
        }
    }
}

impl DownloadProgress {
    /// Create a new download progress instance.
    #[must_use]
    pub fn new(total_videos: usize) -> Self {
        Self {
            total_videos,
            ..Default::default()
        }
    }

    /// Calculate overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn overall_progress_percent(&self) -> f64 {
        self.overall_progress * 100.0
    }

    /// Calculate current video progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn current_progress_percent(&self) -> f64 {
        self.current_progress * 100.0
    }

    /// Format the download speed as a human-readable string.
    #[must_use]
    pub fn formatted_speed(&self) -> String {
        format_bytes_per_second(self.download_speed_bps)
    }

    /// Format the estimated time remaining as a human-readable string.
    #[must_use]
    pub fn formatted_eta(&self) -> Option<String> {
        self.estimated_remaining_secs.map(format_duration)
    }

    /// Format the elapsed time as a human-readable string.
    #[must_use]
    pub fn formatted_elapsed(&self) -> String {
        format_duration(self.elapsed_secs)
    }
}

/// Format bytes per second as a human-readable string.
fn format_bytes_per_second(bps: f64) -> String {
    if bps < 1024.0 {
        format!("{bps:.0} B/s")
    } else if bps < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bps / 1024.0)
    } else {
        format!("{:.1} MB/s", bps / (1024.0 * 1024.0))
    }
}

/// Format duration in seconds as a human-readable string.
fn format_duration(secs: f64) -> String {
    let total_secs = secs as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
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

/// `YouTube` downloader trait for testability.
#[cfg_attr(test, mockall::automock)]
pub trait YouTubeDownloader: Send + Sync {
    /// Parse a `YouTube` URL and extract playlist information.
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

impl YouTubeUrlValidation {
    /// Create a successful validation result.
    #[must_use]
    pub const fn valid(
        playlist_id: String,
        url_type: YouTubeUrlType,
        normalized_url: String,
    ) -> Self {
        Self {
            is_valid: true,
            playlist_id: Some(playlist_id),
            normalized_url: Some(normalized_url),
            error_message: None,
            url_type,
        }
    }

    /// Create a failed validation result.
    #[must_use]
    pub const fn invalid(error_message: String, url_type: YouTubeUrlType) -> Self {
        Self {
            is_valid: false,
            playlist_id: None,
            normalized_url: None,
            error_message: Some(error_message),
            url_type,
        }
    }
}

/// Validate a `YouTube` URL and extract playlist information.
///
/// This function performs comprehensive validation of `YouTube` URLs and extracts
/// playlist IDs when present. It supports multiple URL formats and provides
/// detailed error messages.
///
/// # Supported URL Formats
///
/// - `https://www.youtube.com/playlist?list=PLxxxxxxxx` - Standard playlist URL
/// - `https://youtube.com/playlist?list=PLxxxxxxxx` - Without www
/// - `https://www.youtube.com/watch?v=xxxxx&list=PLxxxxxxxx` - Watch with playlist
/// - `https://youtu.be/xxxxx?list=PLxxxxxxxx` - Short URL with playlist
/// - `http://` variants are also accepted
///
/// # Examples
///
/// ```rust
/// use mp3youtube_core::youtube::validate_youtube_url;
///
/// // Valid playlist URL
/// let result = validate_youtube_url("https://www.youtube.com/playlist?list=PLrAXtmErZgOei");
/// assert!(result.is_valid);
/// assert_eq!(result.playlist_id, Some("PLrAXtmErZgOei".to_string()));
///
/// // Invalid URL
/// let result = validate_youtube_url("https://example.com");
/// assert!(!result.is_valid);
/// assert!(result.error_message.is_some());
/// ```
#[must_use]
pub fn validate_youtube_url(url: &str) -> YouTubeUrlValidation {
    let url = url.trim();

    // Check for empty URL
    if url.is_empty() {
        return YouTubeUrlValidation::invalid(
            "URL cannot be empty".to_string(),
            YouTubeUrlType::Invalid,
        );
    }

    // Check for basic URL format (must start with http:// or https://)
    let url_lower = url.to_lowercase();
    if !url_lower.starts_with("http://") && !url_lower.starts_with("https://") {
        return YouTubeUrlValidation::invalid(
            "URL must start with http:// or https://".to_string(),
            YouTubeUrlType::Invalid,
        );
    }

    // Check if it's a YouTube URL
    let is_youtube_domain = url_lower.contains("youtube.com") || url_lower.contains("youtu.be");
    if !is_youtube_domain {
        return YouTubeUrlValidation::invalid(
            "URL must be a YouTube URL (youtube.com or youtu.be)".to_string(),
            YouTubeUrlType::Invalid,
        );
    }

    // Determine URL type and extract playlist ID
    let url_type = detect_url_type(url);

    // Extract playlist ID based on URL type
    if let Some(playlist_id) = extract_playlist_id_internal(url) {
        // Validate playlist ID format
        if let Err(validation_error) = validate_playlist_id_format(&playlist_id) {
            return YouTubeUrlValidation::invalid(validation_error, url_type);
        }

        // Generate normalized URL
        let normalized = format!("https://www.youtube.com/playlist?list={playlist_id}");

        YouTubeUrlValidation::valid(playlist_id, url_type, normalized)
    } else {
        let error_msg = match url_type {
            YouTubeUrlType::SingleVideo => {
                "URL is a single video, not a playlist. Add a playlist to the URL or use a playlist URL.".to_string()
            }
            YouTubeUrlType::ShortUrl => {
                "Short URL does not contain a playlist. Use a playlist URL instead.".to_string()
            }
            _ => "URL does not contain a valid playlist ID".to_string(),
        };
        YouTubeUrlValidation::invalid(error_msg, url_type)
    }
}

/// Detect the type of `YouTube` URL.
fn detect_url_type(url: &str) -> YouTubeUrlType {
    let url_lower = url.to_lowercase();

    if url_lower.contains("youtu.be/") {
        if url_lower.contains("list=") {
            YouTubeUrlType::WatchWithPlaylist
        } else {
            YouTubeUrlType::ShortUrl
        }
    } else if url_lower.contains("/playlist") {
        YouTubeUrlType::Playlist
    } else if url_lower.contains("/watch") {
        if url_lower.contains("list=") {
            YouTubeUrlType::WatchWithPlaylist
        } else {
            YouTubeUrlType::SingleVideo
        }
    } else {
        YouTubeUrlType::Invalid
    }
}

/// Extract playlist ID from URL (internal implementation).
fn extract_playlist_id_internal(url: &str) -> Option<String> {
    let url_lower = url.to_lowercase();

    // Find list= parameter (case-insensitive search)
    if let Some(list_pos) = url_lower.find("list=") {
        let start = list_pos + 5;
        let rest = &url[start..];

        // Extract until next & or # or end of string
        let end = rest.find(['&', '#']).unwrap_or(rest.len());
        let playlist_id = rest[..end].trim();

        if !playlist_id.is_empty() {
            return Some(playlist_id.to_string());
        }
    }

    None
}

/// Validate playlist ID format.
///
/// `YouTube` playlist IDs have specific formats:
/// - User-created playlists: Start with "PL" followed by alphanumeric characters
/// - Watch Later: "WL"
/// - Liked Videos: "LL"
/// - Mix playlists: Start with "RD"
/// - Album playlists: Start with "`OLAK5uy`_"
fn validate_playlist_id_format(playlist_id: &str) -> std::result::Result<(), String> {
    // Check minimum length
    if playlist_id.len() < 2 {
        return Err("Playlist ID is too short".to_string());
    }

    // Check maximum length (YouTube playlist IDs are typically under 50 chars)
    if playlist_id.len() > 64 {
        return Err("Playlist ID is too long".to_string());
    }

    // Check for valid characters (alphanumeric, underscore, hyphen)
    if !playlist_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Playlist ID contains invalid characters".to_string());
    }

    // Known valid playlist ID prefixes
    let valid_prefixes = ["PL", "UU", "LL", "WL", "RD", "OLAK5uy_", "FL"];

    // Check if it matches a known prefix or is alphanumeric (for edge cases)
    let has_valid_prefix = valid_prefixes
        .iter()
        .any(|prefix| playlist_id.starts_with(prefix));

    // Allow any valid-looking alphanumeric ID (YouTube may have other formats)
    if !has_valid_prefix
        && !playlist_id
            .chars()
            .next()
            .is_some_and(char::is_alphanumeric)
    {
        return Err("Playlist ID has an invalid format".to_string());
    }

    Ok(())
}

/// Parse a `YouTube` playlist URL and extract the playlist ID.
///
/// Supports the following URL formats:
/// - `https://www.youtube.com/playlist?list=PLxxxxxxxx`
/// - `https://youtube.com/playlist?list=PLxxxxxxxx`
/// - `https://www.youtube.com/watch?v=xxxxx&list=PLxxxxxxxx`
/// - `https://youtu.be/xxxxx?list=PLxxxxxxxx`
///
/// # Errors
///
/// Returns an error if the URL is not a valid `YouTube` playlist URL.
///
/// # Panics
///
/// Panics if the URL validation reports valid but has no playlist ID (should never happen).
#[allow(clippy::expect_used)]
pub fn extract_playlist_id(url: &str) -> Result<String> {
    let validation = validate_youtube_url(url);

    if validation.is_valid {
        Ok(validation
            .playlist_id
            .expect("Valid URL should have playlist ID"))
    } else {
        let error_message = validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string());

        // Determine error type based on URL type
        match validation.url_type {
            YouTubeUrlType::Invalid => Err(Error::Download(DownloadError::InvalidUrl {
                url: url.to_string(),
                reason: error_message,
            })),
            YouTubeUrlType::SingleVideo | YouTubeUrlType::ShortUrl => {
                Err(Error::Download(DownloadError::NotAPlaylist {
                    url: url.to_string(),
                }))
            }
            _ => Err(Error::Download(DownloadError::NotAPlaylist {
                url: url.to_string(),
            })),
        }
    }
}

/// Sanitize a string for use as a filename.
#[must_use]
pub fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];

    let sanitized: String = name
        .chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
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

/// Progress tracker for monitoring download operations.
///
/// This struct tracks download statistics and calculates metrics like
/// speed and estimated time remaining.
#[derive(Debug)]
pub struct DownloadProgressTracker {
    /// Start time of the download operation.
    start_time: std::time::Instant,
    /// Total number of videos to download.
    pub total_videos: usize,
    /// Number of videos completed.
    pub videos_completed: usize,
    /// Number of videos skipped.
    pub videos_skipped: usize,
    /// Number of videos failed.
    pub videos_failed: usize,
    /// Total bytes downloaded across all files.
    pub total_bytes_downloaded: u64,
    /// Recent download samples for speed calculation (timestamp, bytes).
    speed_samples: Vec<(std::time::Instant, u64)>,
    /// Maximum number of samples to keep for speed averaging.
    max_samples: usize,
}

impl Default for DownloadProgressTracker {
    fn default() -> Self {
        Self::new(0)
    }
}

impl DownloadProgressTracker {
    /// Create a new progress tracker.
    #[must_use]
    pub fn new(total_videos: usize) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            total_videos,
            videos_completed: 0,
            videos_skipped: 0,
            videos_failed: 0,
            total_bytes_downloaded: 0,
            speed_samples: Vec::with_capacity(10),
            max_samples: 10,
        }
    }

    /// Record a progress update with current bytes downloaded.
    pub fn record_progress(&mut self, bytes_downloaded: u64) {
        let now = std::time::Instant::now();
        self.total_bytes_downloaded = bytes_downloaded;
        self.speed_samples.push((now, bytes_downloaded));

        // Keep only the most recent samples
        if self.speed_samples.len() > self.max_samples {
            self.speed_samples.remove(0);
        }
    }

    /// Mark a video as completed.
    pub const fn video_completed(&mut self) {
        self.videos_completed += 1;
    }

    /// Mark a video as skipped.
    pub const fn video_skipped(&mut self) {
        self.videos_skipped += 1;
    }

    /// Mark a video as failed.
    pub const fn video_failed(&mut self) {
        self.videos_failed += 1;
    }

    /// Get elapsed time in seconds.
    #[must_use]
    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Calculate current download speed in bytes per second.
    ///
    /// Uses a sliding window average for smoother speed estimates.
    #[must_use]
    pub fn download_speed_bps(&self) -> f64 {
        if self.speed_samples.len() < 2 {
            // Not enough samples, calculate from total
            let elapsed = self.elapsed_secs();
            if elapsed > 0.0 {
                return self.total_bytes_downloaded as f64 / elapsed;
            }
            return 0.0;
        }

        // Calculate speed from the sliding window
        let first = &self.speed_samples[0];
        // SAFETY: We checked len() >= 2 above, so last() is guaranteed to return Some
        let Some(last) = self.speed_samples.last() else {
            return 0.0;
        };

        let time_diff = last.0.duration_since(first.0).as_secs_f64();
        let bytes_diff = last.1.saturating_sub(first.1);

        if time_diff > 0.0 {
            bytes_diff as f64 / time_diff
        } else {
            0.0
        }
    }

    /// Estimate remaining time in seconds based on current progress.
    ///
    /// Returns `None` if there's not enough data to estimate.
    #[must_use]
    pub fn estimated_remaining_secs(&self, current_progress: f64) -> Option<f64> {
        if current_progress <= 0.0 || current_progress >= 1.0 {
            return None;
        }

        let elapsed = self.elapsed_secs();
        if elapsed < 1.0 {
            return None; // Wait for at least 1 second of data
        }

        // Estimate total time based on current progress
        let total_estimated = elapsed / current_progress;
        let remaining = total_estimated - elapsed;

        if remaining > 0.0 {
            Some(remaining)
        } else {
            None
        }
    }

    /// Create a `DownloadProgress` snapshot with current statistics.
    #[must_use]
    pub fn create_progress(
        &self,
        current_index: usize,
        current_title: &str,
        current_progress: f64,
        status: DownloadStatus,
        current_bytes: u64,
        current_total_bytes: Option<u64>,
    ) -> DownloadProgress {
        let overall_progress = if self.total_videos > 0 {
            (current_index.saturating_sub(1) as f64 + current_progress) / self.total_videos as f64
        } else {
            0.0
        };

        DownloadProgress {
            current_index,
            total_videos: self.total_videos,
            current_title: current_title.to_string(),
            current_progress,
            overall_progress,
            status,
            current_bytes,
            current_total_bytes,
            total_bytes_downloaded: self.total_bytes_downloaded,
            download_speed_bps: self.download_speed_bps(),
            estimated_remaining_secs: self.estimated_remaining_secs(overall_progress),
            elapsed_secs: self.elapsed_secs(),
            videos_completed: self.videos_completed,
            videos_skipped: self.videos_skipped,
            videos_failed: self.videos_failed,
        }
    }
}

/// Default `YouTube` downloader implementation.
/// Note: This is a placeholder that will need a proper `YouTube` downloading library.
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
            thumbnail_url: None,
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
        let total_videos = playlist_info.videos.len();
        let mut tracker = DownloadProgressTracker::new(total_videos);

        for (index, video) in playlist_info.videos.iter().enumerate() {
            if let Some(ref callback) = progress {
                callback(tracker.create_progress(
                    index + 1,
                    &video.title,
                    0.0,
                    DownloadStatus::Starting,
                    0,
                    None,
                ));
            }

            // TODO: Implement actual download logic with rustube
            debug!("Would download: {} ({})", video.title, video.id);

            let filename = format!("{}.mp3", sanitize_filename(&video.title));
            let output_path = output_dir.join(&filename);

            tracker.video_failed();

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

// ============================================================================
// Pure Rust YouTube Downloader Implementation (using rusty_ytdl)
// ============================================================================

/// Configuration for the Rusty YTDL downloader.
#[derive(Debug, Clone)]
pub struct RustyYtdlConfig {
    /// Download timeout in seconds per video.
    pub timeout_secs: u64,
    /// Number of retries for failed downloads.
    pub retries: u32,
}

impl Default for RustyYtdlConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 300,
            retries: 3,
        }
    }
}

/// Pure Rust `YouTube` downloader using `rusty_ytdl`.
///
/// This implementation uses the `rusty_ytdl` library which is a pure Rust
/// implementation for downloading `YouTube` videos. No external tools required.
///
/// # Features
///
/// - Pure Rust - no yt-dlp or ffmpeg dependencies
/// - Downloads audio streams directly
/// - Playlist parsing via HTML scraping
/// - Progress tracking
///
/// # Example
///
/// ```rust,no_run
/// use mp3youtube_core::youtube::{RustyYtdlDownloader, YouTubeDownloader};
/// use std::path::Path;
///
/// let downloader = RustyYtdlDownloader::new();
/// let playlist = downloader.parse_playlist_url(
///     "https://www.youtube.com/playlist?list=PLtest123"
/// ).unwrap();
/// println!("Playlist: {} ({} videos)", playlist.title, playlist.video_count);
/// ```
pub struct RustyYtdlDownloader {
    config: RustyYtdlConfig,
    cancel_flag: Arc<AtomicBool>,
}

impl RustyYtdlDownloader {
    /// Create a new downloader with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RustyYtdlConfig::default(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new downloader with custom configuration.
    #[must_use]
    pub fn with_config(config: RustyYtdlConfig) -> Self {
        Self {
            config,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the cancel flag for external cancellation control.
    #[must_use]
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel_flag)
    }

    /// Cancel any ongoing download operation.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Reset the cancel flag.
    pub fn reset_cancel(&self) {
        self.cancel_flag.store(false, Ordering::SeqCst);
    }

    /// Fetch playlist info by scraping the `YouTube` playlist page.
    fn fetch_playlist_info(&self, playlist_id: &str) -> Result<(String, Vec<VideoInfo>)> {
        let url = format!("https://www.youtube.com/playlist?list={playlist_id}");

        info!("Fetching playlist page: {}", url);

        let client = reqwest::blocking::Client::new();
        let response = client
            .get(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .map_err(|e| {
                Error::Download(DownloadError::PlaylistParseFailed {
                    playlist_id: playlist_id.to_string(),
                    reason: format!("Failed to fetch playlist page: {e}"),
                })
            })?;

        let html = response.text().map_err(|e| {
            Error::Download(DownloadError::PlaylistParseFailed {
                playlist_id: playlist_id.to_string(),
                reason: format!("Failed to read response: {e}"),
            })
        })?;

        // Extract playlist title
        let title =
            Self::extract_playlist_title(&html).unwrap_or_else(|| "Unknown Playlist".to_string());

        // Extract video IDs and titles from the page
        let videos = Self::extract_videos_from_html(&html)?;

        Ok((title, videos))
    }

    /// Extract playlist title from HTML.
    fn extract_playlist_title(html: &str) -> Option<String> {
        // Try to find the title in the meta tag or page content
        // Pattern: <meta property="og:title" content="...">
        let og_title_re = Regex::new(r#"<meta\s+property="og:title"\s+content="([^"]+)""#).ok()?;
        if let Some(caps) = og_title_re.captures(html) {
            return Some(html_decode(caps.get(1)?.as_str()));
        }

        // Try: <title>... - YouTube</title>
        let title_re = Regex::new(r"<title>([^<]+?)\s*-\s*YouTube</title>").ok()?;
        if let Some(caps) = title_re.captures(html) {
            return Some(html_decode(caps.get(1)?.as_str()));
        }

        None
    }

    /// Extract video information from playlist HTML.
    fn extract_videos_from_html(html: &str) -> Result<Vec<VideoInfo>> {
        let mut videos = Vec::new();

        // YouTube embeds playlist data as JSON in the page
        // Look for: "playlistVideoListRenderer":{"contents":[...]
        // Or in ytInitialData

        // First try to find ytInitialData
        let json_data = Self::extract_yt_initial_data(html)?;

        // Parse the JSON to extract video info
        if let Some(contents) = Self::find_playlist_contents(&json_data) {
            for item in contents {
                if let Some(video) = Self::parse_playlist_item(item) {
                    videos.push(video);
                }
            }
        }

        if videos.is_empty() {
            warn!("No videos found in playlist HTML, trying alternative extraction");
            // Fallback: try regex-based extraction
            videos = Self::extract_videos_regex(html);
        }

        Ok(videos)
    }

    /// Extract ytInitialData JSON from HTML.
    fn extract_yt_initial_data(html: &str) -> Result<serde_json::Value> {
        // Find the start of ytInitialData
        let start_marker = "var ytInitialData = ";
        let start_pos = html
            .find(start_marker)
            .or_else(|| html.find("ytInitialData = "));

        let start_pos = match start_pos {
            Some(pos) => {
                // Skip past the marker to find the opening brace
                let marker_len = if html[pos..].starts_with("var ytInitialData = ") {
                    "var ytInitialData = ".len()
                } else {
                    "ytInitialData = ".len()
                };
                pos + marker_len
            }
            None => {
                return Err(Error::Download(DownloadError::PlaylistParseFailed {
                    playlist_id: String::new(),
                    reason: "Could not find ytInitialData in page".to_string(),
                }));
            }
        };

        // Find the JSON object by counting braces
        let json_bytes = &html.as_bytes()[start_pos..];
        if json_bytes.is_empty() || json_bytes[0] != b'{' {
            return Err(Error::Download(DownloadError::PlaylistParseFailed {
                playlist_id: String::new(),
                reason: "ytInitialData does not start with '{'".to_string(),
            }));
        }

        let mut brace_count = 0;
        let mut end_pos = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, &byte) in json_bytes.iter().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match byte {
                b'\\' if in_string => escape_next = true,
                b'"' => in_string = !in_string,
                b'{' if !in_string => brace_count += 1,
                b'}' if !in_string => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        end_pos = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        if end_pos == 0 {
            return Err(Error::Download(DownloadError::PlaylistParseFailed {
                playlist_id: String::new(),
                reason: "Could not find end of ytInitialData JSON".to_string(),
            }));
        }

        let json_str = &html[start_pos..start_pos + end_pos];
        debug!("Extracted ytInitialData JSON: {} bytes", json_str.len());

        let parsed: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
            Error::Download(DownloadError::PlaylistParseFailed {
                playlist_id: String::new(),
                reason: format!("Failed to parse ytInitialData: {e}"),
            })
        })?;

        Ok(parsed)
    }

    /// Find playlist contents in the parsed JSON.
    fn find_playlist_contents(json: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
        // Navigate: contents.twoColumnBrowseResultsRenderer.tabs[0].tabRenderer.content
        //           .sectionListRenderer.contents[0].itemSectionRenderer.contents[0]
        //           .playlistVideoListRenderer.contents

        let contents = json.get("contents")?;
        let two_col = contents.get("twoColumnBrowseResultsRenderer")?;
        let tabs = two_col.get("tabs")?.as_array()?;

        for tab in tabs {
            if let Some(tab_renderer) = tab.get("tabRenderer")
                && let Some(content) = tab_renderer.get("content")
                && let Some(section_list) = content.get("sectionListRenderer")
                && let Some(section_contents) = section_list.get("contents")?.as_array()
            {
                for section in section_contents {
                    if let Some(item_section) = section.get("itemSectionRenderer")
                        && let Some(item_contents) = item_section.get("contents")?.as_array()
                    {
                        for item in item_contents {
                            if let Some(playlist_renderer) = item.get("playlistVideoListRenderer") {
                                return playlist_renderer.get("contents")?.as_array();
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Parse a single playlist item from JSON.
    fn parse_playlist_item(item: &serde_json::Value) -> Option<VideoInfo> {
        let renderer = item.get("playlistVideoRenderer")?;

        let id = renderer.get("videoId")?.as_str()?.to_string();

        let title = renderer
            .get("title")?
            .get("runs")?
            .as_array()?
            .first()?
            .get("text")?
            .as_str()?
            .to_string();

        // Duration in seconds - try lengthSeconds first, then parse lengthText
        let duration_secs = renderer
            .get("lengthSeconds")
            .and_then(|d| d.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| {
                renderer
                    .get("lengthText")
                    .and_then(|lt| lt.get("simpleText"))
                    .and_then(|st| st.as_str())
                    .and_then(parse_duration_text)
            });

        let channel = renderer
            .get("shortBylineText")
            .and_then(|sbt| sbt.get("runs"))
            .and_then(|runs| runs.as_array())
            .and_then(|arr| arr.first())
            .and_then(|run| run.get("text"))
            .and_then(|t| t.as_str())
            .map(String::from);

        // Get thumbnail - prefer highest quality
        let thumbnail_url = renderer
            .get("thumbnail")
            .and_then(|t| t.get("thumbnails"))
            .and_then(|thumbs| thumbs.as_array())
            .and_then(|arr| arr.last())
            .and_then(|thumb| thumb.get("url"))
            .and_then(|u| u.as_str())
            .map(String::from);

        Some(VideoInfo {
            id,
            title,
            duration_secs,
            channel,
            thumbnail_url,
        })
    }

    /// Fallback: extract videos using regex patterns.
    fn extract_videos_regex(html: &str) -> Vec<VideoInfo> {
        let mut videos = Vec::new();

        // Pattern to find video IDs in playlist context
        // Look for: "videoId":"XXXXXXXXXXX"
        let video_id_re = Regex::new(r#""videoId"\s*:\s*"([a-zA-Z0-9_-]{11})""#).ok();
        // Title regex is complex and may not be needed for basic extraction
        let _title_re =
            Regex::new(r#""title"\s*:\s*\{\s*"runs"\s*:\s*\[\s*\{\s*"text"\s*:\s*"([^"]+)""#).ok();

        if let Some(ref id_regex) = video_id_re {
            let mut seen_ids = std::collections::HashSet::new();

            for caps in id_regex.captures_iter(html) {
                if let Some(id_match) = caps.get(1) {
                    let id = id_match.as_str().to_string();

                    // Skip duplicates
                    if seen_ids.contains(&id) {
                        continue;
                    }
                    seen_ids.insert(id.clone());

                    // Try to find the title near this video ID
                    let title = format!("Video {id}");

                    videos.push(VideoInfo {
                        id,
                        title,
                        duration_secs: None,
                        channel: None,
                        thumbnail_url: None,
                    });
                }
            }
        }

        // Limit to reasonable number and deduplicate
        videos.truncate(200);
        videos
    }

    /// Download a single video's audio stream.
    fn download_single_video(
        &self,
        video_id: &str,
        video_title: &str,
        output_dir: &Path,
    ) -> Result<PathBuf> {
        // Use tokio runtime to run async rusty_ytdl code
        // The blocking feature of rusty_ytdl hangs, so we use async API

        let video_id_owned = video_id.to_string();
        let video_title_owned = video_title.to_string();
        let output_dir_owned = output_dir.to_path_buf();

        // Try to use existing runtime handle if we're inside a runtime context (e.g., spawn_blocking)
        // Otherwise create a new runtime
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // We're inside an existing runtime - use block_in_place to run async code
            tokio::task::block_in_place(|| {
                handle.block_on(async move {
                    Self::download_single_video_async(
                        &video_id_owned,
                        &video_title_owned,
                        &output_dir_owned,
                    )
                    .await
                })
            })
        } else {
            // No runtime exists - create a new one
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: video_title.to_string(),
                    reason: format!("Failed to create tokio runtime: {e}"),
                })
            })?;

            rt.block_on(async move {
                Self::download_single_video_async(
                    &video_id_owned,
                    &video_title_owned,
                    &output_dir_owned,
                )
                .await
            })
        }
    }

    /// Async implementation of video download
    async fn download_single_video_async(
        video_id: &str,
        video_title: &str,
        output_dir: &Path,
    ) -> Result<PathBuf> {
        let video_url = format!("https://www.youtube.com/watch?v={video_id}");

        debug!(
            "Downloading audio for {} using rusty_ytdl (async)",
            video_id
        );

        // Use VideoAudio (combined stream) with Lowest quality for smallest file size
        // Audio-only streams (Audio filter) often get 403 Forbidden errors from YouTube
        // VideoAudio combined streams are more reliable
        let video_opts = VideoOptions {
            quality: VideoQuality::Lowest,          // Smallest combined stream
            filter: VideoSearchOptions::VideoAudio, // Combined video+audio (more reliable)
            ..Default::default()
        };

        let video = Video::new_with_options(&video_url, video_opts).map_err(|e| {
            Error::Download(DownloadError::VideoUnavailable {
                video_id: video_id.to_string(),
                reason: format!("Failed to create video instance: {e}"),
            })
        })?;

        // Get video info for logging
        let video_info = video.get_info().await.map_err(|e| {
            Error::Download(DownloadError::VideoUnavailable {
                video_id: video_id.to_string(),
                reason: format!("Failed to get video info: {e}"),
            })
        })?;

        // Log available formats for debugging
        info!(
            "Available formats for {}: {}",
            video_id,
            video_info.formats.len()
        );

        // Use mp4 extension for the combined stream
        let sanitized_title = sanitize_filename(video_title);
        let output_path = output_dir.join(format!("{sanitized_title}.mp4"));

        // Download using stream API with chunks (async)
        let stream = video.stream().await.map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: video_title.to_string(),
                reason: format!("Failed to create stream: {e}"),
            })
        })?;

        info!("Stream content length: {} bytes", stream.content_length());

        let mut file = std::fs::File::create(&output_path).map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: video_title.to_string(),
                reason: format!("Failed to create file: {e}"),
            })
        })?;

        use std::io::Write;
        let mut total_bytes = 0u64;
        while let Some(chunk) = stream.chunk().await.map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: video_title.to_string(),
                reason: format!("Failed to download chunk: {e}"),
            })
        })? {
            total_bytes += chunk.len() as u64;
            file.write_all(&chunk).map_err(|e| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: video_title.to_string(),
                    reason: format!("Failed to write chunk: {e}"),
                })
            })?;
        }

        info!(
            "Successfully downloaded {} bytes: {} -> {:?}",
            total_bytes, video_title, output_path
        );
        Ok(output_path)
    }

    /// Get info for a single video.
    pub fn get_video_info(&self, video_id: &str) -> Result<VideoInfo> {
        let video_id_owned = video_id.to_string();

        // Try to use existing runtime handle if available
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(Self::get_video_info_async(&video_id_owned))
            })
        } else {
            // No runtime exists - create a new one
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                Error::Download(DownloadError::VideoUnavailable {
                    video_id: video_id.to_string(),
                    reason: format!("Failed to create tokio runtime: {e}"),
                })
            })?;

            rt.block_on(Self::get_video_info_async(&video_id_owned))
        }
    }

    /// Async implementation of `get_video_info`
    async fn get_video_info_async(video_id: &str) -> Result<VideoInfo> {
        let video_url = format!("https://www.youtube.com/watch?v={video_id}");

        let video = Video::new(&video_url).map_err(|e| {
            Error::Download(DownloadError::VideoUnavailable {
                video_id: video_id.to_string(),
                reason: format!("Failed to create video instance: {e}"),
            })
        })?;

        let info = video.get_info().await.map_err(|e| {
            Error::Download(DownloadError::VideoUnavailable {
                video_id: video_id.to_string(),
                reason: format!("Failed to get video info: {e}"),
            })
        })?;

        let details = &info.video_details;

        Ok(VideoInfo {
            id: details.video_id.clone(),
            title: details.title.clone(),
            duration_secs: details.length_seconds.parse().ok(),
            channel: details.author.as_ref().map(|a| a.name.clone()),
            thumbnail_url: details.thumbnails.last().map(|t| t.url.clone()),
        })
    }
}

impl Default for RustyYtdlDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTubeDownloader for RustyYtdlDownloader {
    fn parse_playlist_url(&self, url: &str) -> Result<PlaylistInfo> {
        // First validate the URL
        let playlist_id = extract_playlist_id(url)?;

        info!("Fetching playlist info for: {}", playlist_id);

        // Fetch playlist info by scraping the page
        let (title, videos) = self.fetch_playlist_info(&playlist_id)?;

        let video_count = videos.len();
        let thumbnail_url = videos.first().and_then(|v| v.thumbnail_url.clone());

        info!("Parsed playlist '{}' with {} videos", title, video_count);

        Ok(PlaylistInfo {
            id: playlist_id,
            title,
            video_count,
            videos,
            thumbnail_url,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn download_playlist(
        &self,
        playlist_info: &PlaylistInfo,
        output_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<Vec<DownloadResult>> {
        // Reset cancel flag at start
        self.reset_cancel();

        info!(
            "Starting download of playlist '{}' ({} videos) to {}",
            playlist_info.title,
            playlist_info.video_count,
            output_dir.display()
        );

        // Ensure output directory exists
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir).map_err(|e| {
                Error::FileSystem(crate::error::FileSystemError::CreateDirFailed {
                    path: output_dir.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;
        }

        let mut results = Vec::with_capacity(playlist_info.videos.len());
        let total_videos = playlist_info.videos.len();

        // Create progress tracker for this download operation
        let mut tracker = DownloadProgressTracker::new(total_videos);

        for (index, video) in playlist_info.videos.iter().enumerate() {
            // Check for cancellation
            if self.cancel_flag.load(Ordering::SeqCst) {
                info!("Download cancelled by user");
                return Err(Error::Download(DownloadError::Cancelled));
            }

            let current_index = index + 1;

            // Report progress: starting
            if let Some(ref callback) = progress {
                callback(tracker.create_progress(
                    current_index,
                    &video.title,
                    0.0,
                    DownloadStatus::Starting,
                    0,
                    None,
                ));
            }

            // Check if file already exists (check multiple extensions)
            let sanitized_title = sanitize_filename(&video.title);
            let extensions = ["m4a", "webm", "audio", "mp3"];
            let existing_file = extensions
                .iter()
                .map(|ext| output_dir.join(format!("{sanitized_title}.{ext}")))
                .find(|p| p.exists());

            if let Some(existing_path) = existing_file {
                info!("Skipping existing file: {}", video.title);
                tracker.video_skipped();
                if let Some(ref callback) = progress {
                    callback(tracker.create_progress(
                        current_index,
                        &video.title,
                        1.0,
                        DownloadStatus::Skipped,
                        0,
                        None,
                    ));
                }
                results.push(DownloadResult {
                    video: video.clone(),
                    success: true,
                    output_path: Some(existing_path),
                    error: None,
                });
                continue;
            }

            // Report progress: downloading
            if let Some(ref callback) = progress {
                callback(tracker.create_progress(
                    current_index,
                    &video.title,
                    0.1,
                    DownloadStatus::Downloading,
                    0,
                    None,
                ));
            }

            // Download the video with retries
            let mut last_error = None;
            let mut success = false;
            let mut output_path = None;

            for attempt in 1..=self.config.retries {
                match self.download_single_video(&video.id, &video.title, output_dir) {
                    Ok(path) => {
                        // Get file size for bytes tracking
                        let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
                        tracker.record_progress(tracker.total_bytes_downloaded + file_size);
                        tracker.video_completed();

                        if let Some(ref callback) = progress {
                            callback(tracker.create_progress(
                                current_index,
                                &video.title,
                                1.0,
                                DownloadStatus::Completed,
                                file_size,
                                Some(file_size),
                            ));
                        }
                        output_path = Some(path);
                        success = true;
                        break;
                    }
                    Err(e) => {
                        warn!(
                            "Download attempt {}/{} failed for '{}': {}",
                            attempt, self.config.retries, video.title, e
                        );
                        last_error = Some(e);

                        if attempt < self.config.retries {
                            // Wait before retry
                            std::thread::sleep(std::time::Duration::from_secs(2));
                        }
                    }
                }
            }

            if success {
                results.push(DownloadResult {
                    video: video.clone(),
                    success: true,
                    output_path,
                    error: None,
                });
            } else {
                let error_msg =
                    last_error.map_or_else(|| "Unknown error".to_string(), |e| e.to_string());
                error!("Failed to download '{}': {}", video.title, error_msg);
                tracker.video_failed();
                if let Some(ref callback) = progress {
                    callback(tracker.create_progress(
                        current_index,
                        &video.title,
                        0.0,
                        DownloadStatus::Failed(error_msg.clone()),
                        0,
                        None,
                    ));
                }
                results.push(DownloadResult {
                    video: video.clone(),
                    success: false,
                    output_path: None,
                    error: Some(error_msg),
                });
            }
        }

        // Log summary
        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.len() - successful;
        info!(
            "Download complete: {} successful, {} failed, elapsed: {:.1}s",
            successful,
            failed,
            tracker.elapsed_secs()
        );

        Ok(results)
    }
}

/// Parse duration text like "3:45" or "1:23:45" into seconds.
fn parse_duration_text(text: &str) -> Option<u64> {
    let parts: Vec<&str> = text.split(':').collect();
    match parts.len() {
        2 => {
            // MM:SS
            let mins: u64 = parts[0].parse().ok()?;
            let secs: u64 = parts[1].parse().ok()?;
            Some(mins * 60 + secs)
        }
        3 => {
            // HH:MM:SS
            let hours: u64 = parts[0].parse().ok()?;
            let mins: u64 = parts[1].parse().ok()?;
            let secs: u64 = parts[2].parse().ok()?;
            Some(hours * 3600 + mins * 60 + secs)
        }
        _ => None,
    }
}

/// Decode HTML entities in a string.
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

// ============================================================================
// Legacy YtDlpDownloader - kept for backwards compatibility but deprecated
// ============================================================================

/// Configuration for the yt-dlp downloader (deprecated - use `RustyYtdlConfig`).
#[deprecated(
    since = "0.2.0",
    note = "Use RustyYtdlConfig and RustyYtdlDownloader instead"
)]
#[derive(Debug, Clone)]
pub struct YtDlpConfig {
    /// Path to yt-dlp executable. If None, searches PATH.
    pub yt_dlp_path: Option<PathBuf>,
    /// Audio quality/bitrate for MP3 conversion (e.g., "192" for 192kbps).
    pub audio_quality: String,
    /// Whether to embed thumbnail in MP3.
    pub embed_thumbnail: bool,
    /// Whether to add metadata to MP3.
    pub add_metadata: bool,
    /// Download timeout in seconds per video.
    pub timeout_secs: u64,
    /// Number of retries for failed downloads.
    pub retries: u32,
}

#[allow(deprecated)]
impl Default for YtDlpConfig {
    fn default() -> Self {
        Self {
            yt_dlp_path: None,
            audio_quality: "192".to_string(),
            embed_thumbnail: true,
            add_metadata: true,
            timeout_secs: 300,
            retries: 3,
        }
    }
}

/// `YouTube` downloader using yt-dlp subprocess (deprecated - use `RustyYtdlDownloader`).
#[deprecated(
    since = "0.2.0",
    note = "Use RustyYtdlDownloader instead - it requires no external dependencies"
)]
pub struct YtDlpDownloader {
    inner: RustyYtdlDownloader,
}

#[allow(deprecated)]
impl YtDlpDownloader {
    /// Create a new downloader with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RustyYtdlDownloader::new(),
        }
    }

    /// Create a new downloader with custom configuration.
    #[must_use]
    pub fn with_config(config: YtDlpConfig) -> Self {
        let rusty_config = RustyYtdlConfig {
            timeout_secs: config.timeout_secs,
            retries: config.retries,
        };
        Self {
            inner: RustyYtdlDownloader::with_config(rusty_config),
        }
    }

    /// Get the cancel flag for external cancellation control.
    #[must_use]
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.inner.cancel_flag()
    }

    /// Cancel any ongoing download operation.
    pub fn cancel(&self) {
        self.inner.cancel();
    }

    /// Reset the cancel flag.
    pub fn reset_cancel(&self) {
        self.inner.reset_cancel();
    }
}

#[allow(deprecated)]
impl Default for YtDlpDownloader {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(deprecated)]
impl YouTubeDownloader for YtDlpDownloader {
    fn parse_playlist_url(&self, url: &str) -> Result<PlaylistInfo> {
        self.inner.parse_playlist_url(url)
    }

    fn download_playlist(
        &self,
        playlist_info: &PlaylistInfo,
        output_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<Vec<DownloadResult>> {
        self.inner
            .download_playlist(playlist_info, output_dir, progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // URL Validation Tests
    // =========================================================================

    mod validate_youtube_url_tests {
        use super::*;

        #[test]
        fn test_valid_standard_playlist_url() {
            let url = "https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(
                result.playlist_id,
                Some("PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf".to_string())
            );
            assert_eq!(result.url_type, YouTubeUrlType::Playlist);
            assert!(result.error_message.is_none());
            assert!(result.normalized_url.is_some());
        }

        #[test]
        fn test_valid_playlist_url_without_www() {
            let url = "https://youtube.com/playlist?list=PLtest123abc";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123abc".to_string()));
        }

        #[test]
        fn test_valid_watch_url_with_playlist() {
            let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOtest";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLrAXtmErZgOtest".to_string()));
            assert_eq!(result.url_type, YouTubeUrlType::WatchWithPlaylist);
        }

        #[test]
        fn test_valid_watch_url_list_first() {
            let url = "https://www.youtube.com/watch?list=PLrAXtmErZgOtest&v=dQw4w9WgXcQ";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLrAXtmErZgOtest".to_string()));
        }

        #[test]
        fn test_valid_short_url_with_playlist() {
            let url = "https://youtu.be/dQw4w9WgXcQ?list=PLrAXtmErZgOtest";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLrAXtmErZgOtest".to_string()));
        }

        #[test]
        fn test_valid_http_url() {
            let url = "http://www.youtube.com/playlist?list=PLtest123";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123".to_string()));
        }

        #[test]
        fn test_valid_mixed_case_list_parameter() {
            let url = "https://www.youtube.com/playlist?LIST=PLtest123";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123".to_string()));
        }

        #[test]
        fn test_valid_user_uploads_playlist() {
            let url = "https://www.youtube.com/playlist?list=UUxxxxxxxxxxxxxxxxxxxxxxxx";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(
                result.playlist_id,
                Some("UUxxxxxxxxxxxxxxxxxxxxxxxx".to_string())
            );
        }

        #[test]
        fn test_valid_mix_playlist() {
            let url = "https://www.youtube.com/watch?v=abc&list=RDxxxxxxxx";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("RDxxxxxxxx".to_string()));
        }

        #[test]
        fn test_invalid_empty_url() {
            let result = validate_youtube_url("");

            assert!(!result.is_valid);
            assert_eq!(result.url_type, YouTubeUrlType::Invalid);
            assert!(result.error_message.unwrap().contains("empty"));
        }

        #[test]
        fn test_invalid_whitespace_only_url() {
            let result = validate_youtube_url("   ");

            assert!(!result.is_valid);
            assert!(result.error_message.unwrap().contains("empty"));
        }

        #[test]
        fn test_invalid_not_youtube() {
            let url = "https://www.vimeo.com/video/123";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert_eq!(result.url_type, YouTubeUrlType::Invalid);
            assert!(result.error_message.unwrap().contains("YouTube"));
        }

        #[test]
        fn test_invalid_no_protocol() {
            let url = "www.youtube.com/playlist?list=PLtest123";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert!(result.error_message.unwrap().contains("http"));
        }

        #[test]
        fn test_invalid_single_video_no_playlist() {
            let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert_eq!(result.url_type, YouTubeUrlType::SingleVideo);
            assert!(result.error_message.unwrap().contains("single video"));
        }

        #[test]
        fn test_invalid_short_url_no_playlist() {
            let url = "https://youtu.be/dQw4w9WgXcQ";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert_eq!(result.url_type, YouTubeUrlType::ShortUrl);
        }

        #[test]
        fn test_invalid_empty_list_parameter() {
            let url = "https://www.youtube.com/playlist?list=";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
        }

        #[test]
        fn test_invalid_playlist_id_too_short() {
            let url = "https://www.youtube.com/playlist?list=X";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert!(result.error_message.unwrap().contains("too short"));
        }

        #[test]
        fn test_invalid_playlist_id_special_chars() {
            let url = "https://www.youtube.com/playlist?list=PL<script>alert(1)</script>";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert!(result.error_message.unwrap().contains("invalid characters"));
        }

        #[test]
        fn test_url_with_trailing_whitespace() {
            let url = "  https://www.youtube.com/playlist?list=PLtest123  ";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123".to_string()));
        }

        #[test]
        fn test_url_with_hash_fragment() {
            let url = "https://www.youtube.com/playlist?list=PLtest123#section";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123".to_string()));
        }

        #[test]
        fn test_normalized_url_format() {
            let url = "https://youtube.com/watch?v=abc&list=PLtest123";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(
                result.normalized_url,
                Some("https://www.youtube.com/playlist?list=PLtest123".to_string())
            );
        }
    }

    // =========================================================================
    // Playlist ID Validation Tests
    // =========================================================================

    mod playlist_id_format_tests {
        use super::*;

        #[test]
        fn test_valid_pl_prefix() {
            assert!(validate_playlist_id_format("PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf").is_ok());
        }

        #[test]
        fn test_valid_uu_prefix() {
            assert!(validate_playlist_id_format("UUxxxxxxxxxxxxxxxxxxxxxxxx").is_ok());
        }

        #[test]
        fn test_valid_rd_prefix() {
            assert!(validate_playlist_id_format("RDxxxxxxxx").is_ok());
        }

        #[test]
        fn test_valid_olak_prefix() {
            assert!(validate_playlist_id_format("OLAK5uy_xxxxxxxxxxxxxxxxx").is_ok());
        }

        #[test]
        fn test_invalid_too_short() {
            assert!(validate_playlist_id_format("X").is_err());
        }

        #[test]
        fn test_invalid_too_long() {
            let long_id = "PL".to_string() + &"x".repeat(100);
            assert!(validate_playlist_id_format(&long_id).is_err());
        }

        #[test]
        fn test_invalid_special_characters() {
            assert!(validate_playlist_id_format("PL<>test").is_err());
            assert!(validate_playlist_id_format("PL test").is_err());
            assert!(validate_playlist_id_format("PL@test").is_err());
        }

        #[test]
        fn test_valid_with_underscore_and_hyphen() {
            assert!(validate_playlist_id_format("PLtest_123-abc").is_ok());
        }
    }

    // =========================================================================
    // URL Type Detection Tests
    // =========================================================================

    mod url_type_detection_tests {
        use super::*;

        #[test]
        fn test_detect_playlist_type() {
            assert_eq!(
                detect_url_type("https://www.youtube.com/playlist?list=PLtest"),
                YouTubeUrlType::Playlist
            );
        }

        #[test]
        fn test_detect_watch_with_playlist_type() {
            assert_eq!(
                detect_url_type("https://www.youtube.com/watch?v=abc&list=PLtest"),
                YouTubeUrlType::WatchWithPlaylist
            );
        }

        #[test]
        fn test_detect_single_video_type() {
            assert_eq!(
                detect_url_type("https://www.youtube.com/watch?v=abc"),
                YouTubeUrlType::SingleVideo
            );
        }

        #[test]
        fn test_detect_short_url_type() {
            assert_eq!(
                detect_url_type("https://youtu.be/abc"),
                YouTubeUrlType::ShortUrl
            );
        }

        #[test]
        fn test_detect_short_url_with_playlist() {
            assert_eq!(
                detect_url_type("https://youtu.be/abc?list=PLtest"),
                YouTubeUrlType::WatchWithPlaylist
            );
        }

        #[test]
        fn test_detect_invalid_type() {
            assert_eq!(
                detect_url_type("https://youtube.com/channel/abc"),
                YouTubeUrlType::Invalid
            );
        }
    }

    // =========================================================================
    // Extract Playlist ID Tests (backward compatibility)
    // =========================================================================

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
        assert!(matches!(
            result,
            Err(Error::Download(DownloadError::NotAPlaylist { .. }))
        ));
    }

    #[test]
    fn test_extract_playlist_id_not_youtube() {
        let url = "https://www.vimeo.com/video/123";
        let result = extract_playlist_id(url);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Download(DownloadError::InvalidUrl { .. }))
        ));
    }

    // =========================================================================
    // Sanitize Filename Tests
    // =========================================================================

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

    // =========================================================================
    // Download Status Tests
    // =========================================================================

    #[test]
    fn test_download_status_equality() {
        assert_eq!(DownloadStatus::Starting, DownloadStatus::Starting);
        assert_eq!(
            DownloadStatus::Failed("error".to_string()),
            DownloadStatus::Failed("error".to_string())
        );
        assert_ne!(DownloadStatus::Starting, DownloadStatus::Downloading);
    }

    // =========================================================================
    // Default Downloader Tests
    // =========================================================================

    #[test]
    fn test_default_downloader_creation() {
        let downloader = DefaultYouTubeDownloader::new();
        let result =
            downloader.parse_playlist_url("https://www.youtube.com/playlist?list=PLtest123");
        assert!(result.is_ok());
    }

    // =========================================================================
    // YouTubeUrlValidation Struct Tests
    // =========================================================================

    #[test]
    fn test_youtube_url_validation_valid() {
        let validation = YouTubeUrlValidation::valid(
            "PLtest123".to_string(),
            YouTubeUrlType::Playlist,
            "https://www.youtube.com/playlist?list=PLtest123".to_string(),
        );

        assert!(validation.is_valid);
        assert_eq!(validation.playlist_id, Some("PLtest123".to_string()));
        assert!(validation.error_message.is_none());
    }

    #[test]
    fn test_youtube_url_validation_invalid() {
        let validation =
            YouTubeUrlValidation::invalid("Test error".to_string(), YouTubeUrlType::Invalid);

        assert!(!validation.is_valid);
        assert!(validation.playlist_id.is_none());
        assert_eq!(validation.error_message, Some("Test error".to_string()));
    }

    // =========================================================================
    // Default Trait Tests
    // =========================================================================

    #[test]
    fn test_youtube_url_type_default() {
        let url_type = YouTubeUrlType::default();
        assert_eq!(url_type, YouTubeUrlType::Invalid);
    }

    // =========================================================================
    // YtDlpDownloader Tests (deprecated wrapper)
    // =========================================================================

    #[allow(deprecated)]
    mod yt_dlp_downloader_tests {
        use super::*;

        #[test]
        fn test_yt_dlp_downloader_creation() {
            // YtDlpDownloader is now a thin wrapper - just test it creates
            let _downloader = YtDlpDownloader::new();
        }

        #[test]
        fn test_yt_dlp_downloader_with_config() {
            let config = YtDlpConfig {
                yt_dlp_path: Some(PathBuf::from("/custom/yt-dlp")),
                audio_quality: "320".to_string(),
                embed_thumbnail: false,
                add_metadata: true,
                timeout_secs: 600,
                retries: 5,
            };
            let _downloader = YtDlpDownloader::with_config(config);
        }

        #[test]
        fn test_yt_dlp_downloader_cancel_flag() {
            let downloader = YtDlpDownloader::new();
            let flag = downloader.cancel_flag();

            // Initially not cancelled
            assert!(!flag.load(Ordering::SeqCst));

            // Set cancel
            downloader.cancel();
            assert!(flag.load(Ordering::SeqCst));

            // Reset cancel
            downloader.reset_cancel();
            assert!(!flag.load(Ordering::SeqCst));
        }

        #[test]
        fn test_yt_dlp_downloader_cancel_flag_shared() {
            let downloader = YtDlpDownloader::new();
            let flag = downloader.cancel_flag();

            // Cancel via the shared flag
            flag.store(true, Ordering::SeqCst);

            // Verify the cancellation is visible via a new flag request
            let flag2 = downloader.cancel_flag();
            assert!(flag2.load(Ordering::SeqCst));
        }

        #[test]
        fn test_yt_dlp_config_default() {
            let config = YtDlpConfig::default();

            assert!(config.yt_dlp_path.is_none());
            assert_eq!(config.audio_quality, "192");
            assert!(config.embed_thumbnail);
            assert!(config.add_metadata);
            assert_eq!(config.timeout_secs, 300);
            assert_eq!(config.retries, 3);
        }

        #[test]
        fn test_yt_dlp_downloader_default() {
            let _downloader = YtDlpDownloader::default();
        }

        #[test]
        fn test_yt_dlp_downloader_implements_youtube_downloader_trait() {
            // Verify that YtDlpDownloader implements YouTubeDownloader
            fn assert_youtube_downloader<T: YouTubeDownloader>() {}
            assert_youtube_downloader::<YtDlpDownloader>();
        }

        #[test]
        fn test_yt_dlp_downloader_parse_invalid_url() {
            let downloader = YtDlpDownloader::new();

            // Invalid URL should fail
            let result = downloader.parse_playlist_url("https://example.com");
            assert!(result.is_err());
        }

        #[test]
        fn test_yt_dlp_downloader_parse_single_video_url() {
            let downloader = YtDlpDownloader::new();

            // Single video URL (no playlist) should fail
            let result =
                downloader.parse_playlist_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            assert!(result.is_err());
        }
    }

    // =========================================================================
    // Download Progress and Result Tests
    // =========================================================================

    mod download_progress_tests {
        use super::*;

        #[test]
        fn test_download_progress_struct() {
            let progress = DownloadProgress {
                current_index: 5,
                total_videos: 10,
                current_title: "Test Song".to_string(),
                current_progress: 0.75,
                overall_progress: 0.45,
                status: DownloadStatus::Downloading,
                current_bytes: 1024,
                current_total_bytes: Some(2048),
                total_bytes_downloaded: 5000,
                download_speed_bps: 102_400.0,
                estimated_remaining_secs: Some(30.0),
                elapsed_secs: 10.5,
                videos_completed: 3,
                videos_skipped: 1,
                videos_failed: 0,
            };

            assert_eq!(progress.current_index, 5);
            assert_eq!(progress.total_videos, 10);
            assert_eq!(progress.current_title, "Test Song");
            assert!((progress.current_progress - 0.75).abs() < f64::EPSILON);
            assert!((progress.overall_progress - 0.45).abs() < f64::EPSILON);
            assert_eq!(progress.status, DownloadStatus::Downloading);
            assert_eq!(progress.current_bytes, 1024);
            assert_eq!(progress.current_total_bytes, Some(2048));
            assert_eq!(progress.total_bytes_downloaded, 5000);
            assert!((progress.download_speed_bps - 102_400.0).abs() < f64::EPSILON);
            assert_eq!(progress.estimated_remaining_secs, Some(30.0));
            assert!((progress.elapsed_secs - 10.5).abs() < f64::EPSILON);
            assert_eq!(progress.videos_completed, 3);
            assert_eq!(progress.videos_skipped, 1);
            assert_eq!(progress.videos_failed, 0);
        }

        #[test]
        fn test_download_progress_formatting() {
            let progress = DownloadProgress {
                current_index: 1,
                total_videos: 5,
                current_title: "Test".to_string(),
                current_progress: 0.5,
                overall_progress: 0.1,
                status: DownloadStatus::Downloading,
                current_bytes: 0,
                current_total_bytes: None,
                total_bytes_downloaded: 0,
                download_speed_bps: 1_536_000.0,       // ~1.5 MB/s
                estimated_remaining_secs: Some(150.0), // 2:30
                elapsed_secs: 75.0,                    // 1:15
                videos_completed: 0,
                videos_skipped: 0,
                videos_failed: 0,
            };

            assert_eq!(progress.formatted_speed(), "1.5 MB/s");
            assert_eq!(progress.formatted_eta(), Some("2:30".to_string()));
            assert_eq!(progress.formatted_elapsed(), "1:15");
            assert!((progress.overall_progress_percent() - 10.0).abs() < f64::EPSILON);
            assert!((progress.current_progress_percent() - 50.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_download_progress_default() {
            let progress = DownloadProgress::default();

            assert_eq!(progress.current_index, 0);
            assert_eq!(progress.total_videos, 0);
            assert!(progress.current_title.is_empty());
            assert!((progress.current_progress - 0.0).abs() < f64::EPSILON);
            assert!((progress.overall_progress - 0.0).abs() < f64::EPSILON);
            assert_eq!(progress.status, DownloadStatus::Starting);
            assert_eq!(progress.current_bytes, 0);
            assert_eq!(progress.current_total_bytes, None);
            assert_eq!(progress.download_speed_bps, 0.0);
        }

        #[test]
        fn test_download_progress_new() {
            let progress = DownloadProgress::new(10);

            assert_eq!(progress.total_videos, 10);
            assert_eq!(progress.current_index, 0);
        }

        #[test]
        fn test_download_result_success() {
            let video = VideoInfo {
                id: "test123".to_string(),
                title: "Test Video".to_string(),
                duration_secs: Some(180),
                channel: Some("Test Channel".to_string()),
                thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            };

            let result = DownloadResult {
                video: video,
                success: true,
                output_path: Some(PathBuf::from("/output/test.mp3")),
                error: None,
            };

            assert!(result.success);
            assert!(result.output_path.is_some());
            assert!(result.error.is_none());
        }

        #[test]
        fn test_download_result_failure() {
            let video = VideoInfo {
                id: "test123".to_string(),
                title: "Test Video".to_string(),
                duration_secs: None,
                channel: None,
                thumbnail_url: None,
            };

            let result = DownloadResult {
                video: video,
                success: false,
                output_path: None,
                error: Some("Download failed".to_string()),
            };

            assert!(!result.success);
            assert!(result.output_path.is_none());
            assert!(result.error.is_some());
        }

        #[test]
        fn test_video_info_clone() {
            let video = VideoInfo {
                id: "test123".to_string(),
                title: "Test Video".to_string(),
                duration_secs: Some(180),
                channel: Some("Test Channel".to_string()),
                thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            };

            let cloned = video.clone();
            assert_eq!(video.id, cloned.id);
            assert_eq!(video.title, cloned.title);
            assert_eq!(video.duration_secs, cloned.duration_secs);
            assert_eq!(video.channel, cloned.channel);
            assert_eq!(video.thumbnail_url, cloned.thumbnail_url);
        }

        #[test]
        fn test_playlist_info_struct() {
            let videos = vec![
                VideoInfo {
                    id: "vid1".to_string(),
                    title: "Video 1".to_string(),
                    duration_secs: Some(120),
                    channel: None,
                    thumbnail_url: None,
                },
                VideoInfo {
                    id: "vid2".to_string(),
                    title: "Video 2".to_string(),
                    duration_secs: Some(240),
                    channel: None,
                    thumbnail_url: None,
                },
            ];

            let playlist = PlaylistInfo {
                id: "PLtest123".to_string(),
                title: "Test Playlist".to_string(),
                video_count: 2,
                videos,
                thumbnail_url: Some("https://example.com/playlist-thumb.jpg".to_string()),
            };

            assert_eq!(playlist.id, "PLtest123");
            assert_eq!(playlist.title, "Test Playlist");
            assert_eq!(playlist.video_count, 2);
            assert_eq!(playlist.videos.len(), 2);
            assert!(playlist.thumbnail_url.is_some());
        }
    }

    // =========================================================================
    // Download Progress Tracker Tests
    // =========================================================================

    mod progress_tracker_tests {
        use super::*;

        #[test]
        fn test_tracker_creation() {
            let tracker = DownloadProgressTracker::new(10);

            assert_eq!(tracker.total_videos, 10);
            assert_eq!(tracker.videos_completed, 0);
            assert_eq!(tracker.videos_skipped, 0);
            assert_eq!(tracker.videos_failed, 0);
            assert_eq!(tracker.total_bytes_downloaded, 0);
        }

        #[test]
        fn test_tracker_default() {
            let tracker = DownloadProgressTracker::default();

            assert_eq!(tracker.total_videos, 0);
        }

        #[test]
        fn test_tracker_video_counts() {
            let mut tracker = DownloadProgressTracker::new(5);

            tracker.video_completed();
            tracker.video_completed();
            tracker.video_skipped();
            tracker.video_failed();

            assert_eq!(tracker.videos_completed, 2);
            assert_eq!(tracker.videos_skipped, 1);
            assert_eq!(tracker.videos_failed, 1);
        }

        #[test]
        fn test_tracker_elapsed_time() {
            let tracker = DownloadProgressTracker::new(5);

            // Elapsed time should be very small (close to 0)
            let elapsed = tracker.elapsed_secs();
            assert!(elapsed >= 0.0);
            assert!(elapsed < 1.0); // Should be less than 1 second
        }

        #[test]
        fn test_tracker_record_progress() {
            let mut tracker = DownloadProgressTracker::new(5);

            tracker.record_progress(1000);
            assert_eq!(tracker.total_bytes_downloaded, 1000);

            tracker.record_progress(2500);
            assert_eq!(tracker.total_bytes_downloaded, 2500);
        }

        #[test]
        fn test_tracker_create_progress() {
            let tracker = DownloadProgressTracker::new(4);

            let progress = tracker.create_progress(
                2,
                "Test Video",
                0.5,
                DownloadStatus::Downloading,
                512,
                Some(1024),
            );

            assert_eq!(progress.current_index, 2);
            assert_eq!(progress.total_videos, 4);
            assert_eq!(progress.current_title, "Test Video");
            assert!((progress.current_progress - 0.5).abs() < f64::EPSILON);
            assert_eq!(progress.status, DownloadStatus::Downloading);
            assert_eq!(progress.current_bytes, 512);
            assert_eq!(progress.current_total_bytes, Some(1024));
        }

        #[test]
        fn test_tracker_overall_progress_calculation() {
            let tracker = DownloadProgressTracker::new(4);

            // Video 2 at 50% progress: (2-1 + 0.5) / 4 = 0.375
            let progress =
                tracker.create_progress(2, "Test", 0.5, DownloadStatus::Downloading, 0, None);

            assert!((progress.overall_progress - 0.375).abs() < f64::EPSILON);
        }

        #[test]
        fn test_tracker_speed_with_insufficient_samples() {
            let tracker = DownloadProgressTracker::new(1);

            // With no samples, speed should be based on total elapsed
            let speed = tracker.download_speed_bps();
            // Speed could be 0 or very high depending on timing
            assert!(speed >= 0.0);
        }

        #[test]
        fn test_tracker_eta_with_no_progress() {
            let tracker = DownloadProgressTracker::new(5);

            // ETA should be None with 0% progress
            let eta = tracker.estimated_remaining_secs(0.0);
            assert!(eta.is_none());

            // ETA should be None with 100% progress
            let eta = tracker.estimated_remaining_secs(1.0);
            assert!(eta.is_none());
        }
    }

    // =========================================================================
    // Format Helper Tests
    // =========================================================================

    mod format_helper_tests {
        use super::*;

        #[test]
        fn test_format_bytes_per_second_bytes() {
            assert_eq!(format_bytes_per_second(0.0), "0 B/s");
            assert_eq!(format_bytes_per_second(512.0), "512 B/s");
            assert_eq!(format_bytes_per_second(1023.0), "1023 B/s");
        }

        #[test]
        fn test_format_bytes_per_second_kilobytes() {
            assert_eq!(format_bytes_per_second(1024.0), "1.0 KB/s");
            assert_eq!(format_bytes_per_second(1536.0), "1.5 KB/s");
            assert_eq!(format_bytes_per_second(102_400.0), "100.0 KB/s");
        }

        #[test]
        fn test_format_bytes_per_second_megabytes() {
            assert_eq!(format_bytes_per_second(1_048_576.0), "1.0 MB/s");
            assert_eq!(format_bytes_per_second(1_572_864.0), "1.5 MB/s");
            assert_eq!(format_bytes_per_second(10_485_760.0), "10.0 MB/s");
        }

        #[test]
        fn test_format_duration_seconds() {
            assert_eq!(format_duration(0.0), "0:00");
            assert_eq!(format_duration(30.0), "0:30");
            assert_eq!(format_duration(59.0), "0:59");
        }

        #[test]
        fn test_format_duration_minutes() {
            assert_eq!(format_duration(60.0), "1:00");
            assert_eq!(format_duration(90.0), "1:30");
            assert_eq!(format_duration(150.0), "2:30");
            assert_eq!(format_duration(3599.0), "59:59");
        }

        #[test]
        fn test_format_duration_hours() {
            assert_eq!(format_duration(3600.0), "1:00:00");
            assert_eq!(format_duration(3660.0), "1:01:00");
            assert_eq!(format_duration(3661.0), "1:01:01");
            assert_eq!(format_duration(7200.0), "2:00:00");
        }
    }
}
