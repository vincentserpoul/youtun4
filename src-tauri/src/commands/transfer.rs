//! File transfer commands.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};
use tracing::{debug, error, info};
use youtun4_core::transfer::{TransferOptions, TransferProgress, TransferResult};

use super::error::map_err;
use super::state::AppState;

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

    let app_handle = app.clone();
    let progress_callback = move |progress: &TransferProgress| {
        if let Err(e) = app_handle.emit("transfer-progress", progress) {
            error!("Failed to emit transfer-progress event: {}", e);
        }
    };

    let manager = state.playlist_manager.read().await;
    manager
        .sync_to_device_with_progress(
            &playlist_name,
            &mount_point,
            &options,
            Some(progress_callback),
        )
        .map_err(map_err)
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
    let progress_callback = move |progress: &TransferProgress| {
        if let Err(e) = app_handle.emit("transfer-progress", progress) {
            error!("Failed to emit transfer-progress event: {}", e);
        }
    };

    let mut engine = youtun4_core::TransferEngine::new();
    engine
        .transfer_files(
            &source_paths,
            &mount_point,
            &options,
            Some(progress_callback),
        )
        .map_err(map_err)
}

/// Compute the checksum of a file.
#[tauri::command]
pub async fn compute_file_checksum(file_path: String) -> std::result::Result<String, String> {
    debug!("Computing checksum for: {}", file_path);

    let path = PathBuf::from(&file_path);
    let engine = youtun4_core::TransferEngine::new();

    engine.compute_file_checksum(&path).map_err(map_err)
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

    let engine = youtun4_core::TransferEngine::new();

    let source_checksum = engine.compute_file_checksum(&source).map_err(map_err)?;
    let dest_checksum = engine.compute_file_checksum(&dest).map_err(map_err)?;

    let matches = source_checksum == dest_checksum;
    info!(
        "Integrity check: {} (source={}, dest={})",
        if matches { "PASSED" } else { "FAILED" },
        &source_checksum[..8],
        &dest_checksum[..8]
    );

    Ok(matches)
}
