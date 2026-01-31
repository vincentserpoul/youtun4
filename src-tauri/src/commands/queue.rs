//! Download queue management commands.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tracing::{error, info};
use youtun4_core::queue::{
    DownloadPriority, DownloadRequest, QueueConfig, QueueItem, QueueItemId, QueueStats,
};
use youtun4_core::youtube::{
    DownloadProgress, RustyYtdlConfig, RustyYtdlDownloader, YouTubeDownloader, validate_youtube_url,
};

use crate::runtime::TaskCategory;

use super::error::map_err;
use super::state::AppState;

/// Event names for download queue events emitted to the frontend.
pub mod queue_events {
    pub const QUEUE_ITEM_ADDED: &str = "queue-item-added";
    pub const QUEUE_ITEM_STARTED: &str = "queue-item-started";
    pub const QUEUE_ITEM_PROGRESS: &str = "queue-item-progress";
    pub const QUEUE_ITEM_COMPLETED: &str = "queue-item-completed";
    pub const QUEUE_ITEM_FAILED: &str = "queue-item-failed";
    pub const QUEUE_ITEM_CANCELLED: &str = "queue-item-cancelled";
    pub const QUEUE_ITEM_REMOVED: &str = "queue-item-removed";
    pub const QUEUE_PAUSED: &str = "queue-paused";
    pub const QUEUE_RESUMED: &str = "queue-resumed";
    pub const QUEUE_CONFIG_UPDATED: &str = "queue-config-updated";
}

/// Serializable request for adding a download to the queue.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AddToQueueRequest {
    pub url: String,
    pub output_dir: String,
    pub playlist_name: Option<String>,
    pub audio_quality: Option<String>,
    pub embed_thumbnail: Option<bool>,
    pub priority: Option<String>,
}

impl AddToQueueRequest {
    fn into_download_request(self) -> DownloadRequest {
        let mut request = DownloadRequest::new(self.url, PathBuf::from(self.output_dir));

        if let Some(name) = self.playlist_name {
            request = request.with_playlist_name(name);
        }
        if let Some(quality) = self.audio_quality {
            request = request.with_audio_quality(quality);
        }
        if let Some(embed) = self.embed_thumbnail {
            request = request.with_embed_thumbnail(embed);
        }
        if let Some(priority) = self.priority {
            let priority = match priority.to_lowercase().as_str() {
                "high" => DownloadPriority::High,
                "low" => DownloadPriority::Low,
                _ => DownloadPriority::Normal,
            };
            request = request.with_priority(priority);
        }

        request
    }
}

/// Add a download request to the queue.
#[tauri::command]
pub async fn queue_add_download(
    app: AppHandle,
    state: State<'_, AppState>,
    request: AddToQueueRequest,
) -> std::result::Result<QueueItemId, String> {
    info!("Adding download to queue: {}", request.url);

    let validation = validate_youtube_url(&request.url);
    if !validation.is_valid {
        return Err(validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string()));
    }

    let download_request = request.into_download_request();
    let queue = state.download_queue_arc();
    let item_id = queue.add(download_request).await;

    if let Some(item) = queue.get_item(item_id).await
        && let Err(e) = app.emit(queue_events::QUEUE_ITEM_ADDED, &item)
    {
        error!("Failed to emit queue-item-added event: {}", e);
    }

    process_queue(app.clone(), state.clone()).await;

    Ok(item_id)
}

/// Add a download request to a specific local playlist.
#[tauri::command]
pub async fn queue_add_to_playlist(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    playlist_name: String,
    priority: Option<String>,
) -> std::result::Result<QueueItemId, String> {
    info!(
        "Adding download to queue for playlist '{}': {}",
        playlist_name, url
    );

    let validation = validate_youtube_url(&url);
    if !validation.is_valid {
        return Err(validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string()));
    }

    let playlist_manager = state.playlist_manager.read().await;
    let playlist_path = playlist_manager.base_path().join(&playlist_name);

    if !playlist_path.exists() {
        playlist_manager
            .create_playlist(&playlist_name, Some(url.clone()))
            .map_err(map_err)?;
    }

    drop(playlist_manager);

    let request = AddToQueueRequest {
        url,
        output_dir: playlist_path.display().to_string(),
        playlist_name: Some(playlist_name),
        audio_quality: None,
        embed_thumbnail: None,
        priority,
    };

    queue_add_download(app, state, request).await
}

/// Add multiple download requests to the queue at once.
#[tauri::command]
pub async fn queue_add_batch(
    app: AppHandle,
    state: State<'_, AppState>,
    requests: Vec<AddToQueueRequest>,
) -> std::result::Result<Vec<QueueItemId>, String> {
    info!("Adding {} downloads to queue (batch)", requests.len());

    for request in &requests {
        let validation = validate_youtube_url(&request.url);
        if !validation.is_valid {
            return Err(format!(
                "Invalid URL '{}': {}",
                request.url,
                validation
                    .error_message
                    .unwrap_or_else(|| "Invalid".to_string())
            ));
        }
    }

    let download_requests: Vec<DownloadRequest> = requests
        .into_iter()
        .map(AddToQueueRequest::into_download_request)
        .collect();

    let queue = state.download_queue_arc();
    let item_ids = queue.add_batch(download_requests).await;

    for &item_id in &item_ids {
        if let Some(item) = queue.get_item(item_id).await
            && let Err(e) = app.emit(queue_events::QUEUE_ITEM_ADDED, &item)
        {
            error!("Failed to emit queue-item-added event: {}", e);
        }
    }

    process_queue(app.clone(), state.clone()).await;

    Ok(item_ids)
}

/// Remove an item from the queue.
#[tauri::command]
pub async fn queue_remove_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<bool, String> {
    info!("Removing item {} from queue", item_id);

    let queue = state.download_queue_arc();
    let removed = queue.remove(item_id).await;

    if removed && let Err(e) = app.emit(queue_events::QUEUE_ITEM_REMOVED, &item_id) {
        error!("Failed to emit queue-item-removed event: {}", e);
    }

    Ok(removed)
}

/// Cancel a downloading or pending item.
#[tauri::command]
pub async fn queue_cancel_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<bool, String> {
    info!("Cancelling queue item {}", item_id);

    let queue = state.download_queue_arc();
    let cancelled = queue.cancel(item_id).await;

    if cancelled {
        if let Err(e) = app.emit(queue_events::QUEUE_ITEM_CANCELLED, &item_id) {
            error!("Failed to emit queue-item-cancelled event: {}", e);
        }

        process_queue(app.clone(), state.clone()).await;
    }

    Ok(cancelled)
}

/// Update the priority of a queue item.
#[tauri::command]
pub async fn queue_set_priority(
    state: State<'_, AppState>,
    item_id: QueueItemId,
    priority: String,
) -> std::result::Result<bool, String> {
    info!("Setting priority of item {} to {}", item_id, priority);

    let priority = match priority.to_lowercase().as_str() {
        "high" => DownloadPriority::High,
        "low" => DownloadPriority::Low,
        _ => DownloadPriority::Normal,
    };

    let queue = state.download_queue_arc();
    Ok(queue.set_priority(item_id, priority).await)
}

/// Move an item to the front of the queue (high priority).
#[tauri::command]
pub async fn queue_move_to_front(
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<bool, String> {
    info!("Moving item {} to front of queue", item_id);

    let queue = state.download_queue_arc();
    Ok(queue.move_to_front(item_id).await)
}

/// Retry a failed queue item.
#[tauri::command]
pub async fn queue_retry_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<bool, String> {
    info!("Retrying queue item {}", item_id);

    let queue = state.download_queue_arc();
    let retried = queue.retry(item_id).await;

    if retried {
        process_queue(app.clone(), state.clone()).await;
    }

    Ok(retried)
}

/// Get a specific queue item.
#[tauri::command]
pub async fn queue_get_item(
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<Option<QueueItem>, String> {
    let queue = state.download_queue_arc();
    Ok(queue.get_item(item_id).await)
}

/// Get all items in the queue.
#[tauri::command]
pub async fn queue_get_all_items(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<QueueItem>, String> {
    let queue = state.download_queue_arc();
    Ok(queue.get_all_items().await)
}

/// Get all pending items in the queue, sorted by priority.
#[tauri::command]
pub async fn queue_get_pending_items(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<QueueItem>, String> {
    let queue = state.download_queue_arc();
    Ok(queue.get_pending_items().await)
}

/// Get all currently downloading items.
#[tauri::command]
pub async fn queue_get_downloading_items(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<QueueItem>, String> {
    let queue = state.download_queue_arc();
    Ok(queue.get_downloading_items().await)
}

/// Get queue statistics.
#[tauri::command]
pub async fn queue_get_stats(
    state: State<'_, AppState>,
) -> std::result::Result<QueueStats, String> {
    let queue = state.download_queue_arc();
    Ok(queue.stats().await)
}

/// Pause the queue (stop starting new downloads).
#[tauri::command]
pub async fn queue_pause(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    info!("Pausing download queue");

    let queue = state.download_queue_arc();
    queue.pause().await;

    if let Err(e) = app.emit(queue_events::QUEUE_PAUSED, &()) {
        error!("Failed to emit queue-paused event: {}", e);
    }

    Ok(())
}

/// Resume the queue (allow starting new downloads).
#[tauri::command]
pub async fn queue_resume(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    info!("Resuming download queue");

    let queue = state.download_queue_arc();
    queue.resume().await;

    if let Err(e) = app.emit(queue_events::QUEUE_RESUMED, &()) {
        error!("Failed to emit queue-resumed event: {}", e);
    }

    process_queue(app.clone(), state.clone()).await;

    Ok(())
}

/// Check if the queue is paused.
#[tauri::command]
pub async fn queue_is_paused(state: State<'_, AppState>) -> std::result::Result<bool, String> {
    let queue = state.download_queue_arc();
    Ok(queue.is_paused().await)
}

/// Clear all finished items from the queue.
#[tauri::command]
pub async fn queue_clear_finished(
    state: State<'_, AppState>,
) -> std::result::Result<usize, String> {
    info!("Clearing finished items from queue");

    let queue = state.download_queue_arc();
    Ok(queue.clear_finished().await)
}

/// Clear all non-downloading items from the queue.
#[tauri::command]
pub async fn queue_clear_all(state: State<'_, AppState>) -> std::result::Result<usize, String> {
    info!("Clearing all items from queue");

    let queue = state.download_queue_arc();
    Ok(queue.clear_all().await)
}

/// Get the queue configuration.
#[tauri::command]
pub async fn queue_get_config(
    state: State<'_, AppState>,
) -> std::result::Result<QueueConfig, String> {
    let queue = state.download_queue_arc();
    Ok(queue.config().await)
}

/// Update the queue configuration.
#[tauri::command]
pub async fn queue_set_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: QueueConfig,
) -> std::result::Result<(), String> {
    info!("Updating queue configuration");

    let queue = state.download_queue_arc();
    queue.set_config(config.clone()).await;

    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.queue = config.clone();
    config_manager.update(app_config).map_err(map_err)?;

    if let Err(e) = app.emit(queue_events::QUEUE_CONFIG_UPDATED, &config) {
        error!("Failed to emit queue-config-updated event: {}", e);
    }

    drop(config_manager);
    process_queue(app.clone(), state.clone()).await;

    Ok(())
}

/// Set the maximum number of concurrent downloads.
#[tauri::command]
pub async fn queue_set_max_concurrent(
    app: AppHandle,
    state: State<'_, AppState>,
    max_concurrent: usize,
) -> std::result::Result<(), String> {
    info!("Setting max concurrent downloads to {}", max_concurrent);

    let queue = state.download_queue_arc();
    queue.set_max_concurrent(max_concurrent).await;

    let new_config = queue.config().await;
    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.queue = new_config;
    config_manager.update(app_config).map_err(map_err)?;

    drop(config_manager);
    process_queue(app.clone(), state.clone()).await;

    Ok(())
}

/// Internal function to process the queue and start downloads.
pub async fn process_queue(app: AppHandle, state: State<'_, AppState>) {
    let queue = state.download_queue_arc();

    while queue.can_start_download().await {
        if let Some(item) = queue.start_next().await {
            let app_clone = app.clone();
            let queue_clone = Arc::clone(&queue);
            let item_id = item.id;

            let config_manager = state.config_manager.read().await;
            let download_quality = config_manager.config().download_quality;
            drop(config_manager);

            let audio_quality =
                item.request
                    .audio_quality
                    .clone()
                    .unwrap_or_else(|| match download_quality {
                        youtun4_core::config::DownloadQuality::Low => "128".to_string(),
                        youtun4_core::config::DownloadQuality::Medium => "192".to_string(),
                        youtun4_core::config::DownloadQuality::High => "320".to_string(),
                    });

            let embed_thumbnail = item.request.embed_thumbnail.unwrap_or(true);
            let url = item.request.url.clone();
            let output_dir = item.request.output_dir.clone();

            let task_id = state.runtime().spawn(
                TaskCategory::Download,
                Some(format!("Queue download: {}", item.display_name())),
                async move {
                    queue_clone.mark_started(item_id, 0).await;

                    if let Err(e) = app_clone.emit(queue_events::QUEUE_ITEM_STARTED, &serde_json::json!({
                        "item_id": item_id,
                        "task_id": 0
                    })) {
                        error!("Failed to emit queue-item-started event: {}", e);
                    }

                    let config = RustyYtdlConfig::default();
                    let _ = audio_quality;
                    let _ = embed_thumbnail;

                    let downloader = RustyYtdlDownloader::with_config(config);

                    let playlist_info = match downloader.parse_playlist_url(&url) {
                        Ok(info) => info,
                        Err(e) => {
                            error!("Failed to parse playlist for queue item {}: {}", item_id, e);
                            queue_clone.mark_failed(item_id, e.to_string()).await;
                            if let Err(emit_err) = app_clone.emit(queue_events::QUEUE_ITEM_FAILED, &serde_json::json!({
                                "item_id": item_id,
                                "error": e.to_string()
                            })) {
                                error!("Failed to emit queue-item-failed event: {}", emit_err);
                            }
                            return;
                        }
                    };

                    queue_clone.update_progress(
                        item_id,
                        0.0,
                        None,
                        Some(playlist_info.video_count),
                        Some(0),
                    ).await;

                    let app_for_progress = app_clone.clone();
                    let queue_for_progress = Arc::clone(&queue_clone);
                    let progress_callback = move |progress: DownloadProgress| {
                        let queue_inner = Arc::clone(&queue_for_progress);
                        let app_inner = app_for_progress.clone();

                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                queue_inner.update_progress(
                                    item_id,
                                    progress.overall_progress,
                                    Some(progress.current_title.clone()),
                                    Some(progress.total_videos),
                                    Some(progress.videos_completed + progress.videos_skipped),
                                ).await;
                            });
                        });

                        if let Err(e) = app_inner.emit(queue_events::QUEUE_ITEM_PROGRESS, &serde_json::json!({
                            "item_id": item_id,
                            "progress": progress.overall_progress,
                            "current_video": progress.current_title,
                            "total_videos": progress.total_videos,
                            "videos_completed": progress.videos_completed + progress.videos_skipped,
                        })) {
                            error!("Failed to emit queue-item-progress event: {}", e);
                        }
                    };

                    if let Err(e) = std::fs::create_dir_all(&output_dir) {
                        error!("Failed to create output directory for queue item {}: {}", item_id, e);
                        queue_clone.mark_failed(item_id, format!("Failed to create output directory: {e}")).await;
                        if let Err(emit_err) = app_clone.emit(queue_events::QUEUE_ITEM_FAILED, &serde_json::json!({
                            "item_id": item_id,
                            "error": format!("Failed to create output directory: {}", e)
                        })) {
                            error!("Failed to emit queue-item-failed event: {}", emit_err);
                        }
                        return;
                    }

                    match downloader.download_playlist(
                        &playlist_info,
                        &output_dir,
                        Some(Box::new(progress_callback)),
                    ) {
                        Ok(_results) => {
                            info!("Queue item {} completed successfully", item_id);
                            queue_clone.mark_completed(item_id).await;
                            if let Err(e) = app_clone.emit(queue_events::QUEUE_ITEM_COMPLETED, &item_id) {
                                error!("Failed to emit queue-item-completed event: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Queue item {} failed: {}", item_id, e);
                            queue_clone.mark_failed(item_id, e.to_string()).await;
                            if let Err(emit_err) = app_clone.emit(queue_events::QUEUE_ITEM_FAILED, &serde_json::json!({
                                "item_id": item_id,
                                "error": e.to_string()
                            })) {
                                error!("Failed to emit queue-item-failed event: {}", emit_err);
                            }
                        }
                    }
                },
            );

            queue.mark_started(item_id, task_id).await;
        }
    }
}
