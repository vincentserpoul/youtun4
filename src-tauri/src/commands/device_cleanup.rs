//! Device cleanup commands.

use std::path::PathBuf;

use tauri::State;
use tracing::info;
use youtun4_core::cleanup::{CleanupOptions, CleanupResult, DeviceCleanupHandler};
use youtun4_core::device::DeviceDetector;

use super::error::map_err;
use super::state::AppState;

/// Preview what would be deleted from a device.
#[tauri::command]
pub async fn preview_device_cleanup(
    mount_point: String,
    skip_hidden: bool,
    skip_system_files: bool,
    protected_patterns: Vec<String>,
) -> std::result::Result<CleanupResult, String> {
    info!("Previewing cleanup for device: {}", mount_point);

    let handler = DeviceCleanupHandler::new();
    let options = CleanupOptions {
        skip_hidden,
        skip_system_files,
        protected_patterns,
        dry_run: true,
        ..Default::default()
    };

    let path = PathBuf::from(&mount_point);
    let result = handler.preview_cleanup(&path, &options).map_err(map_err)?;

    info!(
        "Preview complete: {} files, {} directories would be deleted ({} bytes)",
        result.files_deleted, result.directories_deleted, result.bytes_freed
    );

    Ok(result)
}

/// Clean up (delete) all non-protected files from a device.
#[tauri::command]
pub async fn cleanup_device(
    mount_point: String,
    skip_hidden: bool,
    skip_system_files: bool,
    protected_patterns: Vec<String>,
    verify_deletions: bool,
) -> std::result::Result<CleanupResult, String> {
    info!("Starting cleanup for device: {}", mount_point);

    let handler = DeviceCleanupHandler::new();
    let options = CleanupOptions {
        skip_hidden,
        skip_system_files,
        protected_patterns,
        verify_deletions,
        dry_run: false,
        ..Default::default()
    };

    let path = PathBuf::from(&mount_point);
    let result = handler.cleanup_device(&path, &options).map_err(map_err)?;

    info!(
        "Cleanup complete: {} files, {} directories deleted ({} bytes freed, {} failed)",
        result.files_deleted, result.directories_deleted, result.bytes_freed, result.files_failed
    );

    Ok(result)
}

/// Clean up only audio files from a device.
#[tauri::command]
pub async fn cleanup_device_audio_only(
    mount_point: String,
    skip_hidden: bool,
    verify_deletions: bool,
) -> std::result::Result<CleanupResult, String> {
    info!("Starting audio-only cleanup for device: {}", mount_point);

    let handler = DeviceCleanupHandler::new();
    let options = CleanupOptions {
        skip_hidden,
        skip_system_files: true,
        verify_deletions,
        dry_run: false,
        ..Default::default()
    };

    let path = PathBuf::from(&mount_point);
    let result = handler
        .cleanup_audio_files_only(&path, &options)
        .map_err(map_err)?;

    info!(
        "Audio cleanup complete: {} files deleted ({} bytes freed)",
        result.files_deleted, result.bytes_freed
    );

    Ok(result)
}

/// Perform a verified cleanup with device connection check.
#[tauri::command]
pub async fn cleanup_device_verified(
    state: State<'_, AppState>,
    mount_point: String,
    skip_hidden: bool,
    skip_system_files: bool,
    protected_patterns: Vec<String>,
    verify_deletions: bool,
) -> std::result::Result<CleanupResult, String> {
    info!("Starting verified cleanup for device: {}", mount_point);

    let mut manager = state.device_manager.write().await;
    manager.refresh();

    let path = PathBuf::from(&mount_point);
    let device =
        youtun4_core::device::get_device_by_mount_point(&*manager, &path).map_err(map_err)?;

    youtun4_core::device::verify_device_accessible(&*manager, &device).map_err(map_err)?;

    let handler = DeviceCleanupHandler::new();
    let options = CleanupOptions {
        skip_hidden,
        skip_system_files,
        protected_patterns,
        verify_deletions,
        dry_run: false,
        ..Default::default()
    };

    let result = handler
        .cleanup_device_verified(&*manager, &device, &options)
        .map_err(map_err)?;

    info!(
        "Verified cleanup complete: {} files, {} directories deleted ({} bytes freed)",
        result.files_deleted, result.directories_deleted, result.bytes_freed
    );

    Ok(result)
}
