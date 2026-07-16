//! File transfer commands.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::{AppHandle, State};
use tracing::{debug, info};
use youtun4_core::TransferEngine;
use youtun4_core::transfer::{TransferOptions, TransferProgress, TransferResult};

use super::error::{emit_or_log, map_err};
use super::state::{AppState, SyncTaskInfo};

/// Sync a playlist to a device with progress tracking.
#[tauri::command]
pub async fn sync_playlist_with_progress(
    app: AppHandle,
    state: State<'_, AppState>,
    playlist_name: String,
    device_mount_point: String,
    verify_integrity: bool,
    skip_existing: bool,
) -> std::result::Result<TransferResult, String> {
    info!(
        "Syncing playlist '{}' to device at '{}' with progress tracking (verify={}, skip_existing={})",
        playlist_name, device_mount_point, verify_integrity, skip_existing
    );

    let mount_point = PathBuf::from(&device_mount_point);

    let options = TransferOptions {
        verify_integrity,
        skip_existing,
        ..Default::default()
    };

    let task_id = state.runtime().generate_task_id();
    let sync_info = SyncTaskInfo {
        task_id,
        playlist_name: playlist_name.clone(),
        device_mount_point: device_mount_point.clone(),
        verify_integrity,
        skip_existing,
    };
    state
        .try_register_sync_task(task_id, sync_info, Arc::new(AtomicBool::new(false)))
        .await
        .map_err(map_err)?;

    let app_handle = app;

    let manager = state.playlist_manager_arc().read_owned().await;
    let joined = tokio::task::spawn_blocking(move || {
        let progress_callback = move |progress: &TransferProgress| {
            emit_or_log(&app_handle, "transfer-progress", progress);
        };
        manager.sync_to_device_with_progress(
            &playlist_name,
            &mount_point,
            &options,
            Some(progress_callback),
        )
    })
    .await;

    // Unregister before either `?` so both join-panic and transfer-error
    // paths still clean up the registration.
    state.unregister_sync_task(task_id).await;

    let result = joined.map_err(|e| format!("Sync task failed: {e}"))?;
    result.map_err(map_err)
}

/// Get default transfer options.
#[tauri::command]
pub fn get_default_transfer_options() -> TransferOptions {
    TransferOptions::default()
}

/// Get fast transfer options (no verification).
#[tauri::command]
pub fn get_fast_transfer_options() -> TransferOptions {
    TransferOptions::fast()
}

/// Get reliable transfer options (full verification).
#[tauri::command]
pub fn get_reliable_transfer_options() -> TransferOptions {
    TransferOptions::reliable()
}

/// Transfer specific files to a device.
#[tauri::command]
pub async fn transfer_files_to_device(
    app: AppHandle,
    _state: State<'_, AppState>,
    source_files: Vec<String>,
    device_mount_point: String,
    options: TransferOptions,
) -> std::result::Result<TransferResult, String> {
    info!(
        "Transferring {} files to device at '{}'",
        source_files.len(),
        device_mount_point
    );

    options.validate().map_err(map_err)?;

    let mount_point = PathBuf::from(&device_mount_point);
    let source_paths: Vec<PathBuf> = source_files.iter().map(PathBuf::from).collect();

    let app_handle = app;
    let result = tokio::task::spawn_blocking(move || {
        let progress_callback = move |progress: &TransferProgress| {
            emit_or_log(&app_handle, "transfer-progress", progress);
        };
        let mut engine = TransferEngine::new();
        engine.transfer_files(
            &source_paths,
            &mount_point,
            &options,
            Some(progress_callback),
        )
    })
    .await
    .map_err(|e| format!("Transfer task failed: {e}"))?;
    result.map_err(map_err)
}

/// Compute the checksum of a file.
#[tauri::command]
pub async fn compute_file_checksum(file_path: String) -> std::result::Result<String, String> {
    debug!("Computing checksum for: {}", file_path);

    let path = PathBuf::from(&file_path);

    let result = tokio::task::spawn_blocking(move || {
        let engine = TransferEngine::new();
        engine.compute_file_checksum(&path)
    })
    .await
    .map_err(|e| format!("Checksum task failed: {e}"))?;
    result.map_err(map_err)
}

/// Verify integrity of a transferred file.
#[tauri::command]
pub async fn verify_file_integrity(
    source_path: String,
    destination_path: String,
) -> std::result::Result<bool, String> {
    debug!(
        "Verifying integrity: {} vs {}",
        source_path, destination_path
    );

    let source = PathBuf::from(&source_path);
    let dest = PathBuf::from(&destination_path);

    let result = tokio::task::spawn_blocking(move || {
        let engine = TransferEngine::new();
        let source_checksum = engine.compute_file_checksum(&source)?;
        let dest_checksum = engine.compute_file_checksum(&dest)?;
        Ok((
            source_checksum == dest_checksum,
            source_checksum,
            dest_checksum,
        ))
    })
    .await
    .map_err(|e| format!("Integrity check task failed: {e}"))?;
    let (matches, source_checksum, dest_checksum): (bool, String, String) =
        result.map_err(map_err)?;

    info!(
        "Integrity check: {} (source={}, dest={})",
        if matches { "PASSED" } else { "FAILED" },
        &source_checksum[..8],
        &dest_checksum[..8]
    );

    Ok(matches)
}
