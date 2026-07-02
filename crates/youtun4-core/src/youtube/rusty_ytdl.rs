use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use regex::Regex;
use rusty_ytdl::{Video, VideoOptions, VideoQuality, VideoSearchOptions};
use tracing::{debug, error, info, warn};

use crate::error::{DownloadError, Error, Result};

use super::audio::extract_audio_to_m4a;
use super::downloader::YouTubeDownloader;
use super::model::{DownloadResult, DownloadStatus, PlaylistInfo, ProgressCallback, VideoInfo};
use super::progress::DownloadProgressTracker;
use super::url::{extract_playlist_id, sanitize_filename};

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
/// use youtun4_core::youtube::{RustyYtdlDownloader, YouTubeDownloader};
/// use std::path::Path;
///
/// let downloader = RustyYtdlDownloader::new();
/// let playlist = downloader.parse_playlist_url(
///     "https://www.youtube.com/playlist?list=PLtest123"
/// ).unwrap();
/// println!("Playlist: {} ({} videos)", playlist.title, playlist.video_count);
/// ```
#[derive(Debug)]
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
    #[allow(clippy::unused_self, reason = "consistent API")]
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
        let json_bytes = html.as_bytes().get(start_pos..).unwrap_or_default();
        if json_bytes.first() != Some(&b'{') {
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
    #[allow(clippy::unused_self, reason = "consistent API")]
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
        use std::io::Write;

        let video_url = format!("https://www.youtube.com/watch?v={video_id}");

        debug!(
            "Downloading video+audio for {} using rusty_ytdl (async), will extract audio",
            video_id
        );

        // Download combined video+audio stream (VideoAudio) - this works reliably
        // Audio-only streams often return 403 errors from YouTube
        // Use Highest quality to get best available audio (more likely to be AAC-LC than HE-AAC)
        let video_opts = VideoOptions {
            quality: VideoQuality::Highest,
            filter: VideoSearchOptions::VideoAudio,
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

        let sanitized_title = sanitize_filename(video_title);
        let temp_mp4_path = output_dir.join(format!("{sanitized_title}.temp.mp4"));
        let output_path = output_dir.join(format!("{sanitized_title}.aac"));

        // Download using stream API with chunks (async) to temporary MP4
        let stream = video.stream().await.map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: video_title.to_string(),
                reason: format!("Failed to create stream: {e}"),
            })
        })?;

        info!("Stream content length: {} bytes", stream.content_length());

        let mut file = std::fs::File::create(&temp_mp4_path).map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: video_title.to_string(),
                reason: format!("Failed to create temp file: {e}"),
            })
        })?;

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
        drop(file); // Close the file before reading

        info!(
            "Downloaded {} bytes to temp file, extracting audio...",
            total_bytes
        );

        // Extract audio from MP4 to AAC using spawn_blocking since it's CPU-bound
        let temp_path = temp_mp4_path.clone();
        let out_path = output_path.clone();
        let title = video_title.to_string();

        tokio::task::spawn_blocking(move || extract_audio_to_m4a(&temp_path, &out_path, &title))
            .await
            .map_err(|e| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: video_title.to_string(),
                    reason: format!("Audio extraction task failed: {e}"),
                })
            })??;

        // Delete the temporary MP4 file
        if let Err(e) = std::fs::remove_file(&temp_mp4_path) {
            warn!("Failed to delete temp MP4 file: {}", e);
        }

        info!(
            "Successfully extracted audio: {} -> {:?}",
            video_title, output_path
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

    #[allow(
        clippy::too_many_lines,
        reason = "playlist download requires sequential steps with progress tracking"
    )]
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
            let extensions = ["aac", "m4a", "webm", "audio", "mp3"];
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
                        let file_size = path.metadata().map_or(0, |m| m.len());
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
            let mins: u64 = parts.first()?.parse().ok()?;
            let secs: u64 = parts.get(1)?.parse().ok()?;
            Some(mins * 60 + secs)
        }
        3 => {
            // HH:MM:SS
            let hours: u64 = parts.first()?.parse().ok()?;
            let mins: u64 = parts.get(1)?.parse().ok()?;
            let secs: u64 = parts.get(2)?.parse().ok()?;
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
