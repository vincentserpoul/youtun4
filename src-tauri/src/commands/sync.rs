//! Sync API commands for transferring playlists to devices.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::{AppHandle, Emitter, State};
use tracing::{debug, error, info};
use youtun4_core::Error;
use youtun4_core::transfer::{TransferOptions, TransferProgress};

use crate::runtime::{TaskCategory, TaskId};

use super::error::map_err;
use super::state::{AppState, SyncTaskInfo};

/// Event names for sync events emitted to the frontend.
pub mod sync_events {
    /// Event emitted when a sync operation starts.
    pub const SYNC_STARTED: &str = "sync-started";
    /// Event emitted for sync progress updates.
    pub const SYNC_PROGRESS: &str = "sync-progress";
    /// Event emitted when a sync operation completes successfully.
    pub const SYNC_COMPLETED: &str = "sync-completed";
    /// Event emitted when a sync operation fails.
    pub const SYNC_FAILED: &str = "sync-failed";
    /// Event emitted when a sync operation is cancelled.
    pub const SYNC_CANCELLED: &str = "sync-cancelled";
}

/// Sync progress payload for events.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncProgressPayload {
    pub task_id: TaskId,
    pub status: String,
    pub playlist_name: String,
    pub device_mount_point: String,
    pub current_file_index: usize,
    pub total_files: usize,
    pub current_file_name: String,
    pub current_file_bytes: u64,
    pub current_file_total: u64,
    pub total_bytes_transferred: u64,
    pub total_bytes: u64,
    pub files_completed: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
    pub transfer_speed_bps: f64,
    pub estimated_remaining_secs: Option<f64>,
    pub elapsed_secs: f64,
    pub overall_progress_percent: f64,
}

impl SyncProgressPayload {
    pub fn from_transfer_progress(
        task_id: TaskId,
        playlist_name: &str,
        device_mount_point: &str,
        progress: &TransferProgress,
    ) -> Self {
        Self {
            task_id,
            status: progress.status.to_string(),
            playlist_name: playlist_name.to_string(),
            device_mount_point: device_mount_point.to_string(),
            current_file_index: progress.current_file_index,
            total_files: progress.total_files,
            current_file_name: progress.current_file_name.clone(),
            current_file_bytes: progress.current_file_bytes,
            current_file_total: progress.current_file_total,
            total_bytes_transferred: progress.total_bytes_transferred,
            total_bytes: progress.total_bytes,
            files_completed: progress.files_completed,
            files_skipped: progress.files_skipped,
            files_failed: progress.files_failed,
            transfer_speed_bps: progress.transfer_speed_bps,
            estimated_remaining_secs: progress.estimated_remaining_secs,
            elapsed_secs: progress.elapsed_secs,
            overall_progress_percent: progress.overall_progress_percent(),
        }
    }
}

/// Sync result payload for completion events.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncResultPayload {
    pub task_id: TaskId,
    pub success: bool,
    pub was_cancelled: bool,
    pub playlist_name: String,
    pub device_mount_point: String,
    pub total_files: usize,
    pub files_transferred: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
    pub bytes_transferred: u64,
    pub duration_secs: f64,
    pub error_message: Option<String>,
}

/// Start a sync operation to transfer a playlist to a device.
#[tauri::command]
pub async fn start_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    playlist_name: String,
    device_mount_point: String,
    verify_integrity: bool,
    skip_existing: bool,
) -> std::result::Result<TaskId, String> {
    info!(
        "Starting sync: playlist '{}' -> device '{}' (verify={}, skip_existing={})",
        playlist_name, device_mount_point, verify_integrity, skip_existing
    );

    let mount_point = PathBuf::from(&device_mount_point);

    if !mount_point.exists() || !mount_point.is_dir() {
        return Err(map_err(Error::device_not_found(&device_mount_point)));
    }

    let options = TransferOptions {
        verify_integrity,
        skip_existing,
        ..Default::default()
    };

    let cancel_token = Arc::new(AtomicBool::new(false));
    let cancel_token_clone = Arc::clone(&cancel_token);

    let task_id = state.runtime().spawn(
        TaskCategory::FileTransfer,
        Some(format!("Sync '{playlist_name}' to '{device_mount_point}'")),
        async {},
    );

    let sync_info = SyncTaskInfo {
        task_id,
        playlist_name: playlist_name.clone(),
        device_mount_point: device_mount_point.clone(),
        verify_integrity,
        skip_existing,
    };
    state
        .register_sync_task(task_id, sync_info.clone(), cancel_token)
        .await;

    if let Err(e) = app.emit(sync_events::SYNC_STARTED, &sync_info) {
        error!("Failed to emit sync-started event: {}", e);
    }

    let playlist_name_clone = playlist_name.clone();
    let device_mount_point_clone = device_mount_point.clone();
    let app_handle = app.clone();
    let playlist_manager = state.playlist_manager_arc();
    let sync_tasks = Arc::clone(&state.sync_tasks);

    tokio::spawn(async move {
        let playlist_name_for_progress = playlist_name_clone.clone();
        let device_mount_point_for_progress = device_mount_point_clone.clone();
        let app_handle_for_progress = app_handle.clone();
        let task_id_for_progress = task_id;

        let progress_callback = move |progress: &TransferProgress| {
            let payload = SyncProgressPayload::from_transfer_progress(
                task_id_for_progress,
                &playlist_name_for_progress,
                &device_mount_point_for_progress,
                progress,
            );
            if let Err(e) = app_handle_for_progress.emit(sync_events::SYNC_PROGRESS, &payload) {
                error!("Failed to emit sync-progress event: {}", e);
            }
        };

        let manager = playlist_manager.read().await;
        let result = manager.sync_to_device_cancellable(
            &playlist_name_clone,
            &PathBuf::from(&device_mount_point_clone),
            &options,
            cancel_token_clone,
            Some(progress_callback),
        );

        {
            let mut tasks = sync_tasks.write().await;
            tasks.remove(&task_id);
        }

        match result {
            Ok(transfer_result) => {
                let payload = SyncResultPayload {
                    task_id,
                    success: transfer_result.success,
                    was_cancelled: transfer_result.was_cancelled,
                    playlist_name: playlist_name_clone.clone(),
                    device_mount_point: device_mount_point_clone.clone(),
                    total_files: transfer_result.total_files,
                    files_transferred: transfer_result.files_transferred,
                    files_skipped: transfer_result.files_skipped,
                    files_failed: transfer_result.files_failed,
                    bytes_transferred: transfer_result.bytes_transferred,
                    duration_secs: transfer_result.duration_secs,
                    error_message: None,
                };

                let event = if transfer_result.was_cancelled {
                    info!("Sync task {} was cancelled", task_id);
                    sync_events::SYNC_CANCELLED
                } else if transfer_result.success {
                    info!(
                        "Sync task {} completed: {} files transferred, {} skipped, {} failed",
                        task_id,
                        transfer_result.files_transferred,
                        transfer_result.files_skipped,
                        transfer_result.files_failed
                    );
                    sync_events::SYNC_COMPLETED
                } else {
                    error!(
                        "Sync task {} failed: {} files failed",
                        task_id, transfer_result.files_failed
                    );
                    sync_events::SYNC_FAILED
                };

                if let Err(e) = app_handle.emit(event, &payload) {
                    error!("Failed to emit {} event: {}", event, e);
                }
            }
            Err(e) => {
                error!("Sync task {} failed with error: {}", task_id, e);
                let payload = SyncResultPayload {
                    task_id,
                    success: false,
                    was_cancelled: false,
                    playlist_name: playlist_name_clone.clone(),
                    device_mount_point: device_mount_point_clone.clone(),
                    total_files: 0,
                    files_transferred: 0,
                    files_skipped: 0,
                    files_failed: 0,
                    bytes_transferred: 0,
                    duration_secs: 0.0,
                    error_message: Some(e.to_string()),
                };

                if let Err(e) = app_handle.emit(sync_events::SYNC_FAILED, &payload) {
                    error!("Failed to emit sync-failed event: {}", e);
                }
            }
        }
    });

    info!("Sync task {} spawned successfully", task_id);
    Ok(task_id)
}

/// Cancel a running sync operation.
#[tauri::command]
pub async fn cancel_sync(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> std::result::Result<bool, String> {
    info!("Cancelling sync task {}", task_id);
    let cancelled = state.cancel_sync_task(task_id).await;
    if cancelled {
        info!("Sync task {} cancellation requested", task_id);
    } else {
        debug!("Sync task {} not found or already completed", task_id);
    }
    Ok(cancelled)
}

/// Get the status of a sync operation.
#[tauri::command]
pub async fn get_sync_status(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> std::result::Result<Option<SyncTaskInfo>, String> {
    debug!("Getting sync status for task {}", task_id);
    let info = state.get_sync_task_info(task_id).await;
    Ok(info)
}

/// Get all currently active sync operations.
#[tauri::command]
pub async fn list_active_syncs(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<SyncTaskInfo>, String> {
    debug!("Listing active syncs");
    let syncs = state.list_sync_tasks().await;
    info!("Found {} active sync operations", syncs.len());
    Ok(syncs)
}
