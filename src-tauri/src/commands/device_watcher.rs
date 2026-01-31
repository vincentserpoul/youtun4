//! Device watching commands for monitoring USB device connections.

use tauri::{AppHandle, Emitter, State};
use tracing::{debug, error, info};
use youtun4_core::device::{DeviceEvent, DeviceWatcher};

use super::state::AppState;

/// Event names for device events emitted to the frontend.
pub mod device_events {
    /// Event emitted when a device is connected.
    pub const DEVICE_CONNECTED: &str = "device-connected";
    /// Event emitted when a device is disconnected.
    pub const DEVICE_DISCONNECTED: &str = "device-disconnected";
    /// Event emitted when the device list is refreshed.
    pub const DEVICES_REFRESHED: &str = "devices-refreshed";
}

/// Start watching for USB device connections/disconnections.
#[tauri::command]
pub async fn start_device_watcher(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<bool, String> {
    info!("Starting device watcher");

    // Check if watcher is already running
    {
        let handle = state.device_watcher_handle_arc();
        let existing = handle.read().await;
        if existing.is_some() {
            debug!("Device watcher already running");
            return Ok(false);
        }
    }

    // Create and start the device watcher
    let device_manager = state.device_manager_arc();
    let watcher = DeviceWatcher::new(device_manager);
    let (mut event_rx, watcher_handle) = watcher.start();

    // Store the handle
    {
        let handle_arc = state.device_watcher_handle_arc();
        let mut handle = handle_arc.write().await;
        *handle = Some(watcher_handle);
    }

    // Spawn a task to forward events to the frontend
    let app_handle = app.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match &event {
                DeviceEvent::Connected(device) => {
                    info!("Emitting device-connected event: {}", device.name);
                    if let Err(e) = app_handle.emit(device_events::DEVICE_CONNECTED, device) {
                        error!("Failed to emit device-connected event: {}", e);
                    }
                }
                DeviceEvent::Disconnected(device) => {
                    info!("Emitting device-disconnected event: {}", device.name);
                    if let Err(e) = app_handle.emit(device_events::DEVICE_DISCONNECTED, device) {
                        error!("Failed to emit device-disconnected event: {}", e);
                    }
                }
                DeviceEvent::Refreshed(devices) => {
                    info!(
                        "Emitting devices-refreshed event: {} devices",
                        devices.len()
                    );
                    if let Err(e) = app_handle.emit(device_events::DEVICES_REFRESHED, devices) {
                        error!("Failed to emit devices-refreshed event: {}", e);
                    }
                }
            }
        }
        debug!("Device event forwarding task ended");
    });

    info!("Device watcher started successfully");
    Ok(true)
}

/// Stop watching for USB device connections/disconnections.
#[tauri::command]
pub async fn stop_device_watcher(state: State<'_, AppState>) -> std::result::Result<bool, String> {
    info!("Stopping device watcher");

    let handle_arc = state.device_watcher_handle_arc();
    let mut handle = handle_arc.write().await;

    if let Some(watcher_handle) = handle.take() {
        watcher_handle.stop().await;
        info!("Device watcher stopped successfully");
        Ok(true)
    } else {
        debug!("Device watcher was not running");
        Ok(false)
    }
}

/// Check if the device watcher is currently running.
#[tauri::command]
pub async fn is_device_watcher_running(
    state: State<'_, AppState>,
) -> std::result::Result<bool, String> {
    let handle_arc = state.device_watcher_handle_arc();
    let handle = handle_arc.read().await;
    Ok(handle.is_some())
}
