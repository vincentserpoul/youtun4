//! YouTube URL validation and download commands.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};
use tracing::{debug, error, info};
use youtun4_core::Error;
use youtun4_core::youtube::{
    DownloadProgress, DownloadStatus, PlaylistInfo, RustyYtdlConfig, RustyYtdlDownloader,
    YouTubeDownloader, YouTubeUrlValidation, validate_youtube_url,
};

use crate::runtime::{TaskCategory, TaskId};

use super::error::map_err;
use super::state::AppState;

/// Event names for YouTube download events emitted to the frontend.
pub mod youtube_events {
    pub const DOWNLOAD_STARTED: &str = "youtube-download-started";
    pub const DOWNLOAD_PROGRESS: &str = "youtube-download-progress";
    pub const DOWNLOAD_COMPLETED: &str = "youtube-download-completed";
    pub const DOWNLOAD_FAILED: &str = "youtube-download-failed";
    pub const DOWNLOAD_CANCELLED: &str = "youtube-download-cancelled";
}

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

/// Validate a YouTube URL and extract playlist information.
#[tauri::command]
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

/// Check if a URL is a valid YouTube playlist URL.
#[tauri::command]
pub fn is_valid_youtube_playlist_url(url: String) -> bool {
    let result = validate_youtube_url(&url);
    result.is_valid
}

/// Extract the playlist ID from a YouTube URL.
#[tauri::command]
pub fn extract_youtube_playlist_id(url: String) -> std::result::Result<String, String> {
    debug!("Extracting playlist ID from URL: {}", url);
    let result = validate_youtube_url(&url);

    if result.is_valid {
        #[allow(clippy::unwrap_used)]
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
#[allow(clippy::unnecessary_wraps)]
pub fn check_yt_dlp_available() -> std::result::Result<String, String> {
    info!("Checking downloader availability (pure Rust - always available)");
    Ok("rusty_ytdl (pure Rust)".to_string())
}

/// Fetch playlist information from a YouTube URL.
#[tauri::command]
pub async fn fetch_youtube_playlist_info(url: String) -> std::result::Result<PlaylistInfo, String> {
    info!("Fetching playlist info for URL: {}", url);

    let result = tokio::task::spawn_blocking(move || {
        let downloader = RustyYtdlDownloader::new();
        downloader.parse_playlist_url(&url)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    result.map_err(map_err)
}

/// Download a YouTube playlist as MP3 files.
#[tauri::command]
pub async fn download_youtube_playlist(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    output_dir: String,
    audio_quality: Option<String>,
    embed_thumbnail: Option<bool>,
) -> std::result::Result<TaskId, String> {
    info!(
        "Starting YouTube playlist download: {} -> {}",
        url, output_dir
    );

    let validation = validate_youtube_url(&url);
    if !validation.is_valid {
        return Err(validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string()));
    }

    let output_path = PathBuf::from(&output_dir);

    if !output_path.exists() {
        std::fs::create_dir_all(&output_path)
            .map_err(|e| format!("Failed to create output directory: {e}"))?;
    }

    let config = RustyYtdlConfig::default();
    let _ = audio_quality;
    let _ = embed_thumbnail;

    let task_id = state.runtime().spawn(
        TaskCategory::Download,
        Some(format!("Download playlist: {url}")),
        async {},
    );

    let url_clone = url;
    let app_handle = app;

    std::thread::spawn(move || {
        let downloader = RustyYtdlDownloader::with_config(config);

        if let Err(e) = app_handle.emit(youtube_events::DOWNLOAD_STARTED, &task_id) {
            error!("Failed to emit download-started event: {}", e);
        }

        let playlist_info = match downloader.parse_playlist_url(&url_clone) {
            Ok(info) => info,
            Err(e) => {
                error!("Failed to parse playlist: {}", e);
                let category = classify_error(&e);
                let payload = DownloadResultPayload {
                    task_id,
                    success: false,
                    successful_count: 0,
                    failed_count: 0,
                    skipped_count: 0,
                    total_count: 0,
                    results: vec![],
                    error_message: Some(e.to_string()),
                    error_category: Some(category),
                    error_title: Some(category.title().to_string()),
                    error_description: Some(category.description().to_string()),
                };
                if let Err(emit_err) = app_handle.emit(youtube_events::DOWNLOAD_FAILED, &payload) {
                    error!("Failed to emit download-failed event: {}", emit_err);
                }
                return;
            }
        };

        info!(
            "Playlist '{}' has {} videos",
            playlist_info.title, playlist_info.video_count
        );

        let app_handle_for_progress = app_handle.clone();
        let progress_callback = move |progress: DownloadProgress| {
            let payload = DownloadProgressPayload::from_progress(task_id, &progress);
            if let Err(e) =
                app_handle_for_progress.emit(youtube_events::DOWNLOAD_PROGRESS, &payload)
            {
                error!("Failed to emit download-progress event: {}", e);
            }
        };

        let results = match downloader.download_playlist(
            &playlist_info,
            &output_path,
            Some(Box::new(progress_callback)),
        ) {
            Ok(results) => results,
            Err(e) => {
                error!("Download failed: {}", e);
                let category = classify_error(&e);

                let event = if matches!(
                    e,
                    Error::Download(youtun4_core::error::DownloadError::Cancelled)
                ) {
                    youtube_events::DOWNLOAD_CANCELLED
                } else {
                    youtube_events::DOWNLOAD_FAILED
                };

                let payload = DownloadResultPayload {
                    task_id,
                    success: false,
                    successful_count: 0,
                    failed_count: 0,
                    skipped_count: 0,
                    total_count: playlist_info.video_count,
                    results: vec![],
                    error_message: Some(e.to_string()),
                    error_category: Some(category),
                    error_title: Some(category.title().to_string()),
                    error_description: Some(category.description().to_string()),
                };
                if let Err(emit_err) = app_handle.emit(event, &payload) {
                    error!("Failed to emit {} event: {}", event, emit_err);
                }
                return;
            }
        };

        let successful_count = results.iter().filter(|r| r.success).count();
        let failed_count = results
            .iter()
            .filter(|r| !r.success && r.error.is_some())
            .count();
        let skipped_count = results.len() - successful_count - failed_count;

        let video_results: Vec<VideoDownloadResult> = results
            .iter()
            .map(|r| VideoDownloadResult {
                video_id: r.video.id.clone(),
                title: r.video.title.clone(),
                success: r.success,
                output_path: r
                    .output_path
                    .as_ref()
                    .map(|p: &std::path::PathBuf| p.display().to_string()),
                error: r.error.clone(),
            })
            .collect();

        let payload = DownloadResultPayload {
            task_id,
            success: failed_count == 0,
            successful_count,
            failed_count,
            skipped_count,
            total_count: results.len(),
            results: video_results,
            error_message: None,
            error_category: None,
            error_title: None,
            error_description: None,
        };

        if failed_count == 0 {
            info!(
                "Download completed: {} successful, {} skipped",
                successful_count, skipped_count
            );
        } else {
            info!(
                "Download completed with errors: {} successful, {} failed, {} skipped",
                successful_count, failed_count, skipped_count
            );
        }

        if let Err(e) = app_handle.emit(youtube_events::DOWNLOAD_COMPLETED, &payload) {
            error!("Failed to emit download completed event: {}", e);
        }
    });

    info!("Download task {} spawned successfully", task_id);
    Ok(task_id)
}

// =============================================================================
// Helper functions for download_youtube_to_playlist
// =============================================================================

/// Audio file extensions recognized for track counting.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "mp4", "wav", "flac", "ogg", "aac", "webm", "opus",
];

/// Create a failure payload for download errors.
fn create_failure_payload(
    task_id: TaskId,
    error: &Error,
    total_count: usize,
) -> DownloadResultPayload {
    let category = classify_error(error);
    DownloadResultPayload {
        task_id,
        success: false,
        successful_count: 0,
        failed_count: 0,
        skipped_count: 0,
        total_count,
        results: vec![],
        error_message: Some(error.to_string()),
        error_category: Some(category),
        error_title: Some(category.title().to_string()),
        error_description: Some(category.description().to_string()),
    }
}

/// Emit a failure event for download errors.
fn emit_failure_event(app_handle: &AppHandle, error: &Error, payload: &DownloadResultPayload) {
    let event = if matches!(
        error,
        Error::Download(youtun4_core::error::DownloadError::Cancelled)
    ) {
        youtube_events::DOWNLOAD_CANCELLED
    } else {
        youtube_events::DOWNLOAD_FAILED
    };

    if let Err(emit_err) = app_handle.emit(event, payload) {
        error!("Failed to emit {} event: {}", event, emit_err);
    }
}

/// Update playlist.json with source URL and thumbnail before download.
fn update_playlist_metadata_before_download(
    playlist_json_path: &std::path::Path,
    source_url: &str,
    playlist_info: &PlaylistInfo,
) {
    if !playlist_json_path.exists() {
        return;
    }

    let Ok(content) = std::fs::read_to_string(playlist_json_path) else {
        return;
    };
    let Ok(mut metadata) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };
    let Some(obj) = metadata.as_object_mut() else {
        return;
    };

    obj.insert("source_url".to_string(), serde_json::json!(source_url));

    if let Some(thumb) = &playlist_info.thumbnail_url {
        obj.insert("thumbnail_url".to_string(), serde_json::json!(thumb));
    }

    if let Ok(updated) = serde_json::to_string_pretty(&metadata) {
        let _ = std::fs::write(playlist_json_path, updated);
    }
}

/// Count audio files and calculate total size in a directory.
fn count_audio_files(dir: &std::path::Path) -> (usize, u64) {
    let mut track_count = 0usize;
    let mut total_size = 0u64;

    let Ok(entries) = std::fs::read_dir(dir) else {
        return (track_count, total_size);
    };

    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        if AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            track_count += 1;
            if let Ok(meta) = std::fs::metadata(&path) {
                total_size += meta.len();
            }
        }
    }

    (track_count, total_size)
}

/// Build track metadata from download results.
fn build_tracks_metadata(
    results: &[youtun4_core::youtube::DownloadResult],
) -> Vec<serde_json::Value> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    results
        .iter()
        .filter(|r| r.success && r.output_path.is_some())
        .map(|r| {
            let file_name = r
                .output_path
                .as_ref()
                .and_then(|p: &std::path::PathBuf| p.file_name())
                .and_then(|n: &std::ffi::OsStr| n.to_str())
                .unwrap_or("")
                .to_string();

            serde_json::json!({
                "file_name": file_name,
                "video_id": r.video.id,
                "source_url": format!("https://www.youtube.com/watch?v={}", r.video.id),
                "title": r.video.title,
                "channel": r.video.channel,
                "duration_secs": r.video.duration_secs,
                "thumbnail_url": r.video.thumbnail_url,
                "downloaded_at": now
            })
        })
        .collect()
}

/// Update playlist.json with track count, size, and metadata after download.
fn update_playlist_metadata_after_download(
    playlist_json_path: &std::path::Path,
    output_path: &std::path::Path,
    results: &[youtun4_core::youtube::DownloadResult],
) {
    let Ok(content) = std::fs::read_to_string(playlist_json_path) else {
        return;
    };
    let Ok(mut metadata) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };
    let Some(obj) = metadata.as_object_mut() else {
        return;
    };

    let (track_count, total_size) = count_audio_files(output_path);

    obj.insert("track_count".to_string(), serde_json::json!(track_count));
    obj.insert(
        "total_size_bytes".to_string(),
        serde_json::json!(total_size),
    );
    obj.insert(
        "modified_at".to_string(),
        serde_json::json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        ),
    );

    let tracks_metadata = build_tracks_metadata(results);
    obj.insert("tracks".to_string(), serde_json::json!(tracks_metadata));

    if let Ok(updated) = serde_json::to_string_pretty(&metadata) {
        let _ = std::fs::write(playlist_json_path, updated);
    }

    info!(
        "Updated playlist.json: {} tracks, {} bytes, {} track metadata entries",
        track_count,
        total_size,
        tracks_metadata.len()
    );
}

/// Create a success payload from download results.
fn create_success_payload(
    task_id: TaskId,
    results: &[youtun4_core::youtube::DownloadResult],
) -> DownloadResultPayload {
    let successful_count = results.iter().filter(|r| r.success).count();
    let failed_count = results
        .iter()
        .filter(|r| !r.success && r.error.is_some())
        .count();
    let skipped_count = results.len() - successful_count - failed_count;

    let video_results: Vec<VideoDownloadResult> = results
        .iter()
        .map(|r| VideoDownloadResult {
            video_id: r.video.id.clone(),
            title: r.video.title.clone(),
            success: r.success,
            output_path: r
                .output_path
                .as_ref()
                .map(|p: &std::path::PathBuf| p.display().to_string()),
            error: r.error.clone(),
        })
        .collect();

    DownloadResultPayload {
        task_id,
        success: failed_count == 0,
        successful_count,
        failed_count,
        skipped_count,
        total_count: results.len(),
        results: video_results,
        error_message: None,
        error_category: None,
        error_title: None,
        error_description: None,
    }
}

/// Log download completion status.
fn log_download_completion(playlist_name: &str, payload: &DownloadResultPayload) {
    if payload.failed_count == 0 {
        info!(
            "Download to playlist '{}' completed: {} successful, {} skipped",
            playlist_name, payload.successful_count, payload.skipped_count
        );
    } else {
        info!(
            "Download to playlist '{}' completed with errors: {} successful, {} failed, {} skipped",
            playlist_name, payload.successful_count, payload.failed_count, payload.skipped_count
        );
    }
}

// =============================================================================
// Download command
// =============================================================================

/// Download a YouTube playlist directly to a local playlist folder.
#[tauri::command]
pub async fn download_youtube_to_playlist(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    playlist_name: String,
) -> std::result::Result<TaskId, String> {
    info!(
        "Downloading YouTube playlist to local playlist: {} -> {}",
        url, playlist_name
    );

    let validation = validate_youtube_url(&url);
    if !validation.is_valid {
        return Err(validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string()));
    }

    let playlist_manager = state.playlist_manager.read().await;
    let playlist_path = playlist_manager.base_path().join(&playlist_name);
    drop(playlist_manager);

    if !playlist_path.exists() {
        std::fs::create_dir_all(&playlist_path)
            .map_err(|e| format!("Failed to create playlist directory: {e}"))?;
    }

    let task_id = state.runtime().generate_task_id();

    let url_clone = url.clone();
    let playlist_name_clone = playlist_name.clone();
    let app_handle = app.clone();
    let output_path = playlist_path;

    std::thread::spawn(move || {
        run_playlist_download(
            task_id,
            &app_handle,
            &url_clone,
            &playlist_name_clone,
            &output_path,
        );
    });

    info!(
        "Download task {} spawned for playlist '{}'",
        task_id, playlist_name
    );
    Ok(task_id)
}

/// Run the playlist download in a background thread.
fn run_playlist_download(
    task_id: TaskId,
    app_handle: &AppHandle,
    url: &str,
    playlist_name: &str,
    output_path: &std::path::Path,
) {
    let config = RustyYtdlConfig::default();
    let downloader = RustyYtdlDownloader::with_config(config);

    if let Err(e) = app_handle.emit(youtube_events::DOWNLOAD_STARTED, &task_id) {
        error!("Failed to emit download-started event: {}", e);
    }

    // Parse playlist
    let playlist_info = match downloader.parse_playlist_url(url) {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to parse playlist: {}", e);
            let payload = create_failure_payload(task_id, &e, 0);
            emit_failure_event(app_handle, &e, &payload);
            return;
        }
    };

    info!(
        "Playlist '{}' has {} videos",
        playlist_info.title, playlist_info.video_count
    );

    // Update playlist metadata before download
    let playlist_json_path = output_path.join("playlist.json");
    update_playlist_metadata_before_download(&playlist_json_path, url, &playlist_info);

    // Set up progress callback
    let app_handle_for_progress = app_handle.clone();
    let progress_callback = move |progress: DownloadProgress| {
        let payload = DownloadProgressPayload::from_progress(task_id, &progress);
        if let Err(e) = app_handle_for_progress.emit(youtube_events::DOWNLOAD_PROGRESS, &payload) {
            error!("Failed to emit download-progress event: {}", e);
        }
    };

    // Download playlist
    let results = match downloader.download_playlist(
        &playlist_info,
        output_path,
        Some(Box::new(progress_callback)),
    ) {
        Ok(results) => results,
        Err(e) => {
            error!("Download failed: {}", e);
            let payload = create_failure_payload(task_id, &e, playlist_info.video_count);
            emit_failure_event(app_handle, &e, &payload);
            return;
        }
    };

    // Create success payload and log completion
    let payload = create_success_payload(task_id, &results);
    log_download_completion(playlist_name, &payload);

    // Update playlist metadata after download
    update_playlist_metadata_after_download(&playlist_json_path, output_path, &results);

    if let Err(e) = app_handle.emit(youtube_events::DOWNLOAD_COMPLETED, &payload) {
        error!("Failed to emit download-completed event: {}", e);
    }
}
