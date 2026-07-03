use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, State};
use tokio::task::spawn_blocking;
use tracing::{error, info};
use youtun4_core::Error;
use youtun4_core::youtube::{
    DownloadProgress, RustyYtdlConfig, RustyYtdlDownloader, YouTubeDownloader, validate_youtube_url,
};

use crate::runtime::TaskId;

use super::super::error::emit_or_log;
use super::super::state::AppState;
use super::metadata::{
    update_playlist_metadata_after_download, update_playlist_metadata_before_download,
};
use super::{DownloadProgressPayload, DownloadResultPayload, VideoDownloadResult, classify_error};

/// Event names for `YouTube` download events emitted to the frontend.
pub mod youtube_events {
    pub const DOWNLOAD_STARTED: &str = "youtube-download-started";
    pub const DOWNLOAD_PROGRESS: &str = "youtube-download-progress";
    pub const DOWNLOAD_COMPLETED: &str = "youtube-download-completed";
    pub const DOWNLOAD_FAILED: &str = "youtube-download-failed";
    pub const DOWNLOAD_CANCELLED: &str = "youtube-download-cancelled";
}

/// Download a `YouTube` playlist as MP3 files.
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
        // One-shot metadata syscall; not worth spawn_blocking.
        fs::create_dir_all(&output_path)
            .map_err(|e| format!("Failed to create output directory: {e}"))?;
    }

    // TODO: audio_quality and embed_thumbnail are accepted for forward compatibility
    // but not yet wired through to the downloader.
    let _ = audio_quality;
    let _ = embed_thumbnail;

    let task_id = state.runtime().generate_task_id();

    // Create the downloader and register its cancel flag before spawning
    let config = RustyYtdlConfig::default();
    let downloader = RustyYtdlDownloader::with_config(config);
    let cancel_flag = downloader.cancel_flag();
    state.register_download_task(task_id, cancel_flag).await;

    let url_clone = url;
    let app_handle = app;
    let download_tasks = Arc::clone(&state.download_tasks);

    tokio::spawn(async move {
        let app_handle_outer = app_handle.clone();
        let _ = spawn_blocking(move || {
            emit_or_log(&app_handle, youtube_events::DOWNLOAD_STARTED, &task_id);

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
                    emit_or_log(&app_handle, youtube_events::DOWNLOAD_FAILED, &payload);
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
                emit_or_log(
                    &app_handle_for_progress,
                    youtube_events::DOWNLOAD_PROGRESS,
                    &payload,
                );
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
                    emit_or_log(&app_handle, event, &payload);
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
                        .map(|p: &PathBuf| p.display().to_string()),
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

            emit_or_log(&app_handle, youtube_events::DOWNLOAD_COMPLETED, &payload);
        })
        .await;

        // Unregister the download task in async context — no runtime recreation needed
        let mut tasks = download_tasks.write().await;
        tasks.remove(&task_id);
        drop(tasks);
        drop(app_handle_outer);
    });

    info!("Download task {} spawned successfully", task_id);
    Ok(task_id)
}

// =============================================================================
// Helper functions for download_youtube_to_playlist
// =============================================================================

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

    emit_or_log(app_handle, event, payload);
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
                .map(|p: &PathBuf| p.display().to_string()),
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

/// Download a `YouTube` playlist directly to a local playlist folder.
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
        // One-shot metadata syscall; not worth spawn_blocking.
        fs::create_dir_all(&playlist_path)
            .map_err(|e| format!("Failed to create playlist directory: {e}"))?;
    }

    let task_id = state.runtime().generate_task_id();

    // Create the downloader and register its cancel flag before spawning
    let config = RustyYtdlConfig::default();
    let downloader = RustyYtdlDownloader::with_config(config);
    let cancel_flag = downloader.cancel_flag();
    state.register_download_task(task_id, cancel_flag).await;

    let url_clone = url.clone();
    let playlist_name_clone = playlist_name.clone();
    let app_handle = app.clone();
    let output_path = playlist_path;
    let download_tasks = Arc::clone(&state.download_tasks);

    tokio::spawn(async move {
        let _ = spawn_blocking(move || {
            run_playlist_download(
                task_id,
                &app_handle,
                &url_clone,
                &playlist_name_clone,
                &output_path,
                &downloader,
            );
        })
        .await;

        // Unregister the download task in async context — no runtime recreation needed
        let mut tasks = download_tasks.write().await;
        tasks.remove(&task_id);
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
    output_path: &Path,
    downloader: &RustyYtdlDownloader,
) {
    emit_or_log(app_handle, youtube_events::DOWNLOAD_STARTED, &task_id);

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
        emit_or_log(
            &app_handle_for_progress,
            youtube_events::DOWNLOAD_PROGRESS,
            &payload,
        );
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

    emit_or_log(app_handle, youtube_events::DOWNLOAD_COMPLETED, &payload);
}
