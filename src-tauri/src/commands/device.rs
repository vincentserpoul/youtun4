//! Device API commands for detecting and managing USB devices.

#![allow(clippy::similar_names)]

use std::path::PathBuf;

use tauri::State;
use tracing::{debug, error, info};
use youtun4_core::device::{DeviceDetector, DeviceInfo};

use super::error::map_err;
use super::state::AppState;

/// Warning level for capacity checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum CapacityWarningLevel {
    /// Sufficient space available.
    #[default]
    Ok,
    /// Space is limited (usage will be high after sync).
    Warning,
    /// Insufficient space for sync.
    Critical,
}

/// Result of checking device capacity for a sync operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CapacityCheckResult {
    /// Whether the playlist(s) can fit on the device.
    pub can_fit: bool,
    /// Total bytes required for the sync operation.
    pub required_bytes: u64,
    /// Available bytes on the device.
    pub available_bytes: u64,
    /// Total device capacity in bytes.
    pub total_bytes: u64,
    /// Device usage percentage after sync (0.0 - 100.0).
    pub usage_after_sync_percent: f64,
    /// Warning level based on available space.
    pub warning_level: CapacityWarningLevel,
    /// Human-readable message about the capacity status.
    pub message: String,
}

/// List all detected devices.
#[tauri::command]
pub async fn list_devices(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<DeviceInfo>, String> {
    info!("=== LIST_DEVICES command called ===");

    let mut manager = state.device_manager.write().await;
    manager.refresh();
    let devices = manager.list_devices().map_err(map_err)?;

    info!("=== LIST_DEVICES returning {} devices ===", devices.len());
    for (i, d) in devices.iter().enumerate() {
        info!(
            "  Device {}: name='{}' mount='{}'",
            i,
            d.name,
            d.mount_point.display()
        );
    }

    Ok(devices)
}

/// Get information about a specific device by mount point.
#[tauri::command]
pub async fn get_device_info(
    state: State<'_, AppState>,
    mount_point: String,
) -> std::result::Result<DeviceInfo, String> {
    debug!("Getting device info for: {}", mount_point);

    let mut manager = state.device_manager.write().await;
    manager.refresh();

    let device =
        youtun4_core::device::get_device_by_mount_point(&*manager, &PathBuf::from(&mount_point))
            .map_err(map_err)?;

    info!(
        "Retrieved device info: {} at {} ({} bytes available)",
        device.name,
        device.mount_point.display(),
        device.available_bytes
    );
    Ok(device)
}

/// Check if a device is currently connected and available.
#[tauri::command]
pub async fn check_device_available(
    state: State<'_, AppState>,
    mount_point: String,
) -> std::result::Result<bool, String> {
    debug!("Checking device availability: {}", mount_point);

    let mut manager = state.device_manager.write().await;
    manager.refresh();

    let path = PathBuf::from(&mount_point);
    let is_connected = manager.is_device_connected(&path);
    let is_accessible = path.exists() && path.is_dir();

    let available = is_connected && is_accessible;
    info!(
        "Device availability for {}: connected={}, accessible={}, available={}",
        mount_point, is_connected, is_accessible, available
    );

    Ok(available)
}

/// Verify that a device has sufficient space for a transfer.
#[tauri::command]
pub async fn verify_device_space(
    state: State<'_, AppState>,
    mount_point: String,
    required_bytes: u64,
) -> std::result::Result<bool, String> {
    debug!(
        "Verifying space for device: {} (required: {} bytes)",
        mount_point, required_bytes
    );

    let mut manager = state.device_manager.write().await;
    manager.refresh();

    let device =
        youtun4_core::device::get_device_by_mount_point(&*manager, &PathBuf::from(&mount_point))
            .map_err(map_err)?;

    youtun4_core::device::check_device_space(&device, required_bytes).map_err(map_err)?;

    info!(
        "Device {} has sufficient space: {} bytes available, {} bytes required",
        device.name, device.available_bytes, required_bytes
    );
    Ok(true)
}

/// Check if playlists can fit on a device before syncing.
#[tauri::command]
pub async fn check_sync_capacity(
    state: State<'_, AppState>,
    playlist_names: Vec<String>,
    device_mount_point: String,
) -> std::result::Result<CapacityCheckResult, String> {
    debug!(
        "Checking sync capacity for {} playlists to device: {}",
        playlist_names.len(),
        device_mount_point
    );

    // Get device info
    let mut manager = state.device_manager.write().await;
    manager.refresh();
    let device = youtun4_core::device::get_device_by_mount_point(
        &*manager,
        &PathBuf::from(&device_mount_point),
    )
    .map_err(map_err)?;
    drop(manager);

    // Calculate total required bytes from all playlists
    let playlist_manager = state.playlist_manager.read().await;
    let mut total_required: u64 = 0;

    for playlist_name in &playlist_names {
        match playlist_manager.get_folder_statistics(playlist_name) {
            Ok(stats) => {
                total_required += stats.audio_size_bytes;
                debug!(
                    "Playlist '{}' size: {} bytes",
                    playlist_name, stats.audio_size_bytes
                );
            }
            Err(e) => {
                error!(
                    "Failed to get stats for playlist '{}': {}",
                    playlist_name, e
                );
                return Err(map_err(e));
            }
        }
    }
    drop(playlist_manager);

    // Calculate capacity metrics
    let can_fit = device.available_bytes >= total_required;
    let used_after_sync = device.used_bytes() + total_required;
    let usage_after_sync_percent = if device.total_bytes > 0 {
        (used_after_sync as f64 / device.total_bytes as f64) * 100.0
    } else {
        100.0
    };

    let warning_level = if !can_fit || usage_after_sync_percent > 95.0 {
        CapacityWarningLevel::Critical
    } else if usage_after_sync_percent > 85.0 {
        CapacityWarningLevel::Warning
    } else {
        CapacityWarningLevel::Ok
    };

    let message = if !can_fit {
        let deficit = total_required - device.available_bytes;
        format!(
            "Insufficient space: need {} more on {}",
            format_bytes(deficit),
            device.name
        )
    } else if warning_level == CapacityWarningLevel::Warning {
        format!(
            "Limited space: {} will be {:.0}% full after sync",
            device.name, usage_after_sync_percent
        )
    } else {
        format!(
            "Ready to sync: {} available on {}",
            format_bytes(device.available_bytes - total_required),
            device.name
        )
    };

    let result = CapacityCheckResult {
        can_fit,
        required_bytes: total_required,
        available_bytes: device.available_bytes,
        total_bytes: device.total_bytes,
        usage_after_sync_percent,
        warning_level,
        message,
    };

    info!(
        "Capacity check result: can_fit={}, required={} bytes, available={} bytes, usage_after={:.1}%, level={:?}",
        result.can_fit,
        result.required_bytes,
        result.available_bytes,
        result.usage_after_sync_percent,
        result.warning_level
    );

    Ok(result)
}

/// Format bytes as a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.2} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} bytes")
    }
}
