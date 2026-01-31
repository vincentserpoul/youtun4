//! Device mount/unmount commands.

use std::path::PathBuf;

use tauri::State;
use tracing::{debug, info};
use youtun4_core::device::{DeviceMountHandler, MountResult, MountStatus, UnmountResult};

use super::error::map_err;
use super::state::AppState;

/// Get the mount status of a device.
#[tauri::command]
pub async fn get_mount_status(
    state: State<'_, AppState>,
    device_path: String,
) -> std::result::Result<MountStatus, String> {
    debug!("Getting mount status for: {}", device_path);

    let path = PathBuf::from(&device_path);
    let status = state
        .mount_handler
        .get_mount_status(&path)
        .map_err(map_err)?;

    info!(
        "Mount status for {}: mounted={}, accessible={}, read_only={}",
        device_path, status.is_mounted, status.is_accessible, status.is_read_only
    );
    Ok(status)
}

/// Mount a device.
#[tauri::command]
pub async fn mount_device(
    state: State<'_, AppState>,
    device_path: String,
    mount_point: Option<String>,
) -> std::result::Result<MountResult, String> {
    info!("Mounting device: {}", device_path);

    let path = PathBuf::from(&device_path);

    let result = if let Some(mp) = mount_point {
        let mount_pt = PathBuf::from(&mp);
        state
            .mount_handler
            .mount_device_at(&path, &mount_pt)
            .map_err(map_err)?
    } else {
        state
            .mount_handler
            .mount_device_auto(&path)
            .map_err(map_err)?
    };

    info!(
        "Device {} mounted at {:?}",
        result.device_name, result.mount_point
    );
    Ok(result)
}

/// Unmount a device.
#[tauri::command]
pub async fn unmount_device(
    state: State<'_, AppState>,
    mount_point: String,
    force: bool,
) -> std::result::Result<UnmountResult, String> {
    info!("Unmounting device at: {} (force={})", mount_point, force);

    let path = PathBuf::from(&mount_point);
    let result = state
        .mount_handler
        .unmount_device(&path, force)
        .map_err(map_err)?;

    info!("Device unmounted from {:?}", result.mount_point);
    Ok(result)
}

/// Eject a device.
#[tauri::command]
pub async fn eject_device(
    state: State<'_, AppState>,
    mount_point: String,
) -> std::result::Result<UnmountResult, String> {
    info!("Ejecting device at: {}", mount_point);

    let path = PathBuf::from(&mount_point);
    let result = state.mount_handler.eject_device(&path).map_err(map_err)?;

    info!("Device ejected from {:?}", result.mount_point);
    Ok(result)
}

/// Check if a mount point is accessible.
#[tauri::command]
pub async fn is_mount_point_accessible(
    state: State<'_, AppState>,
    mount_point: String,
) -> std::result::Result<bool, String> {
    debug!("Checking accessibility of mount point: {}", mount_point);

    let path = PathBuf::from(&mount_point);
    let accessible = state.mount_handler.is_mount_point_accessible(&path);

    info!("Mount point {} accessible: {}", mount_point, accessible);
    Ok(accessible)
}

/// Get the current platform identifier.
#[tauri::command]
pub async fn get_mount_handler_platform(
    state: State<'_, AppState>,
) -> std::result::Result<String, String> {
    Ok(state.mount_handler.platform().to_string())
}
