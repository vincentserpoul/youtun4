//! Sync orchestrator commands for multi-playlist syncing.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::{AppHandle, Emitter, State};
use tracing::{error, info};
use youtun4_core::Error;
use youtun4_core::sync::{
    SyncOptions, SyncOrchestrator, SyncProgress, SyncRequest, SyncResult as CoreSyncResult,
};
use youtun4_core::transfer::TransferOptions;

use crate::runtime::{TaskCategory, TaskId};

use super::error::map_err;
use super::state::{AppState, SyncTaskInfo};
use super::sync::sync_events;

/// Event names for sync orchestrator events.
pub mod sync_orchestrator_events {
    pub const SYNC_ORCHESTRATOR_PROGRESS: &str = "sync-orchestrator-progress";
    pub const SYNC_ORCHESTRATOR_COMPLETED: &str = "sync-orchestrator-completed";
    pub const SYNC_ORCHESTRATOR_FAILED: &str = "sync-orchestrator-failed";
    pub const SYNC_ORCHESTRATOR_CANCELLED: &str = "sync-orchestrator-cancelled";
}

/// Start a multi-playlist sync operation using the sync orchestrator.
#[tauri::command]
pub async fn start_orchestrated_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    playlists: Vec<String>,
    device_mount_point: String,
    cleanup_enabled: bool,
    verify_integrity: bool,
    skip_existing: bool,
) -> std::result::Result<TaskId, String> {
    info!(
        "Starting orchestrated sync: {} playlist(s) -> device '{}' (cleanup={}, verify={}, skip_existing={})",
        playlists.len(),
        device_mount_point,
        cleanup_enabled,
        verify_integrity,
        skip_existing
    );

    if playlists.is_empty() {
        return Err(map_err(Error::Configuration(
            "No playlists specified for sync".to_string(),
        )));
    }

    let mount_point = PathBuf::from(&device_mount_point);

    if !mount_point.exists() || !mount_point.is_dir() {
        return Err(map_err(Error::device_not_found(&device_mount_point)));
    }

    let sync_options = SyncOptions {
        cleanup_enabled,
        transfer_options: TransferOptions {
            verify_integrity,
            skip_existing,
            ..Default::default()
        },
        ..Default::default()
    };

    let cancel_token = Arc::new(AtomicBool::new(false));
    let cancel_token_clone = Arc::clone(&cancel_token);

    let task_id = state.runtime().spawn(
        TaskCategory::FileTransfer,
        Some(format!(
            "Orchestrated sync: {} playlist(s) to '{}'",
            playlists.len(),
            device_mount_point
        )),
        async {},
    );

    let sync_info = SyncTaskInfo {
        task_id,
        playlist_name: playlists.first().cloned().unwrap_or_default(),
        device_mount_point: device_mount_point.clone(),
        verify_integrity,
        skip_existing,
    };
    state
        .register_sync_task(task_id, sync_info.clone(), Arc::clone(&cancel_token))
        .await;

    if let Err(e) = app.emit(sync_events::SYNC_STARTED, &sync_info) {
        error!("Failed to emit sync-started event: {}", e);
    }

    let playlists_clone = playlists.clone();
    let device_mount_point_clone = device_mount_point.clone();
    let app_handle = app.clone();
    let playlist_manager = state.playlist_manager_arc();
    let device_manager = state.device_manager_arc();
    let sync_tasks = Arc::clone(&state.sync_tasks);

    tokio::spawn(async move {
        let orchestrator = SyncOrchestrator::with_cancellation(cancel_token_clone);
        let request = SyncRequest::new(
            playlists_clone.clone(),
            PathBuf::from(&device_mount_point_clone),
        );

        let app_handle_for_progress = app_handle.clone();
        let progress_callback = move |progress: &SyncProgress| {
            if let Err(e) = app_handle_for_progress.emit(
                sync_orchestrator_events::SYNC_ORCHESTRATOR_PROGRESS,
                progress,
            ) {
                error!("Failed to emit sync-orchestrator-progress event: {}", e);
            }
        };

        let playlist_mgr = playlist_manager.read().await;
        let device_mgr = device_manager.read().await;

        let result = orchestrator.sync(
            &playlist_mgr,
            &*device_mgr,
            request,
            &sync_options,
            Some(progress_callback),
        );

        drop(playlist_mgr);
        drop(device_mgr);

        {
            let mut tasks = sync_tasks.write().await;
            tasks.remove(&task_id);
        }

        match result {
            Ok(sync_result) => {
                let event = if sync_result.was_cancelled {
                    info!("Orchestrated sync task {} was cancelled", task_id);
                    sync_orchestrator_events::SYNC_ORCHESTRATOR_CANCELLED
                } else if sync_result.success {
                    info!(
                        "Orchestrated sync task {} completed: {} files transferred, {} skipped, {} failed",
                        task_id,
                        sync_result.total_files_transferred,
                        sync_result.total_files_skipped,
                        sync_result.total_files_failed
                    );
                    sync_orchestrator_events::SYNC_ORCHESTRATOR_COMPLETED
                } else {
                    error!(
                        "Orchestrated sync task {} failed: {} files failed",
                        task_id, sync_result.total_files_failed
                    );
                    sync_orchestrator_events::SYNC_ORCHESTRATOR_FAILED
                };

                if let Err(e) = app_handle.emit(event, &sync_result) {
                    error!("Failed to emit {} event: {}", event, e);
                }
            }
            Err(e) => {
                error!(
                    "Orchestrated sync task {} failed with error: {}",
                    task_id, e
                );
                let error_result = CoreSyncResult {
                    success: false,
                    was_cancelled: false,
                    final_phase: youtun4_core::sync::SyncPhase::Failed,
                    cleanup_result: None,
                    transfer_results: vec![],
                    total_files_transferred: 0,
                    total_files_skipped: 0,
                    total_files_failed: 0,
                    total_bytes_transferred: 0,
                    duration_secs: 0.0,
                    average_speed_bps: 0.0,
                    error_message: Some(e.to_string()),
                };

                if let Err(emit_err) = app_handle.emit(
                    sync_orchestrator_events::SYNC_ORCHESTRATOR_FAILED,
                    &error_result,
                ) {
                    error!(
                        "Failed to emit sync-orchestrator-failed event: {}",
                        emit_err
                    );
                }
            }
        }
    });

    info!("Orchestrated sync task {} spawned successfully", task_id);
    Ok(task_id)
}

/// Perform a synchronous sync operation (blocking).
#[tauri::command]
pub async fn sync_playlists_to_device(
    app: AppHandle,
    state: State<'_, AppState>,
    playlists: Vec<String>,
    device_mount_point: String,
    options: SyncOptions,
) -> std::result::Result<CoreSyncResult, String> {
    info!(
        "Syncing {} playlist(s) to device '{}' (synchronous)",
        playlists.len(),
        device_mount_point
    );

    if playlists.is_empty() {
        return Err(map_err(Error::Configuration(
            "No playlists specified for sync".to_string(),
        )));
    }

    let mount_point = PathBuf::from(&device_mount_point);

    if !mount_point.exists() || !mount_point.is_dir() {
        return Err(map_err(Error::device_not_found(&device_mount_point)));
    }

    let orchestrator = SyncOrchestrator::new();
    let request = SyncRequest::new(playlists, mount_point);

    let app_handle = app.clone();
    let progress_callback = move |progress: &SyncProgress| {
        if let Err(e) = app_handle.emit(
            sync_orchestrator_events::SYNC_ORCHESTRATOR_PROGRESS,
            progress,
        ) {
            error!("Failed to emit sync-orchestrator-progress event: {}", e);
        }
    };

    let playlist_mgr = state.playlist_manager.read().await;
    let device_mgr = state.device_manager.read().await;

    orchestrator
        .sync(
            &playlist_mgr,
            &*device_mgr,
            request,
            &options,
            Some(progress_callback),
        )
        .map_err(map_err)
}

/// Get default sync options for the orchestrator.
#[tauri::command]
pub fn get_default_sync_options() -> SyncOptions {
    SyncOptions::default()
}

/// Get fast sync options for the orchestrator.
#[tauri::command]
pub fn get_fast_sync_options() -> SyncOptions {
    SyncOptions::fast()
}

/// Get reliable sync options for the orchestrator.
#[tauri::command]
pub fn get_reliable_sync_options() -> SyncOptions {
    SyncOptions::reliable()
}
