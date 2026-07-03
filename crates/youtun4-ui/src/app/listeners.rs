//! Event-listener wiring for `AppContent`.
//!
//! Extracted verbatim from the mount-effect `spawn_local` block in `app/mod.rs`.
//! Registers all Tauri event listeners (device connect/disconnect/refresh, sync
//! progress/completed/failed/cancelled, download started/progress/completed/failed/cancelled)
//! and starts the device watcher.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::components::{
    DownloadErrorInfo, DownloadPanelState, LoadingState, NotificationContext, TransferPanelState,
};
use crate::tauri_api;
use crate::types::{DeviceInfo, DownloadProgress, TaskId, TransferProgress, TransferStatus};

/// Captured state needed to register the application's event listeners.
pub(super) struct ListenerContext {
    pub notifications: NotificationContext,
    pub set_devices: WriteSignal<Vec<DeviceInfo>>,
    pub selected_device: ReadSignal<Option<DeviceInfo>>,
    pub set_selected_device: WriteSignal<Option<DeviceInfo>>,
    pub set_device_list_state: WriteSignal<LoadingState>,
    pub set_transfer_progress: WriteSignal<Option<TransferProgress>>,
    pub set_transfer_panel_state: WriteSignal<TransferPanelState>,
    pub set_syncing: WriteSignal<bool>,
    pub set_current_sync_task_id: WriteSignal<Option<TaskId>>,
    pub set_download_panel_state: WriteSignal<DownloadPanelState>,
    pub set_current_download_task_id: WriteSignal<Option<TaskId>>,
    pub set_download_progress: WriteSignal<Option<DownloadProgress>>,
    pub set_downloading_playlist: WriteSignal<Option<String>>,
    pub set_detail_refresh_trigger: WriteSignal<u32>,
    pub load_playlists: Box<dyn Fn()>,
}

/// Register all Tauri event listeners used by `AppContent` and start the device watcher.
#[allow(
    clippy::cognitive_complexity,
    reason = "flat sequence of independent listener registrations moved verbatim from AppContent's mount effect"
)]
pub(super) async fn register_event_listeners(ctx: ListenerContext) {
    let notifications = ctx.notifications;
    let set_devices = ctx.set_devices;
    let selected_device = ctx.selected_device;
    let set_selected_device = ctx.set_selected_device;
    let set_device_list_state = ctx.set_device_list_state;
    let set_transfer_progress = ctx.set_transfer_progress;
    let set_transfer_panel_state = ctx.set_transfer_panel_state;
    let set_syncing = ctx.set_syncing;
    let set_current_sync_task_id = ctx.set_current_sync_task_id;
    let set_download_panel_state = ctx.set_download_panel_state;
    let set_current_download_task_id = ctx.set_current_download_task_id;
    let set_download_progress = ctx.set_download_progress;
    let set_downloading_playlist = ctx.set_downloading_playlist;
    let set_detail_refresh_trigger = ctx.set_detail_refresh_trigger;
    let load_playlists = ctx.load_playlists;

    leptos::logging::log!("Setting up device event listeners...");

    // Listen for device connected events
    let set_devices_connected = set_devices;
    let read_selected_device_connected = selected_device;
    let set_selected_device_connected = set_selected_device;
    if let Err(e) =
        tauri_api::listen_to_event(tauri_api::device_events::DEVICE_CONNECTED, move |event| {
            leptos::logging::log!("Device connected event received");
            // Parse the device from the event payload
            if let Ok(payload) = js_sys::Reflect::get(&event, &JsValue::from_str("payload"))
                && let Ok(device) = serde_wasm_bindgen::from_value::<DeviceInfo>(payload)
            {
                leptos::logging::log!(
                    "Device connected: {} at {}",
                    device.name,
                    device.mount_point
                );
                let device_name = device.name.clone();
                let device_for_select = device.clone();
                // Add the new device to the list
                set_devices_connected.update(|devices| {
                    // Only add if not already present
                    if !devices.iter().any(|d| d.mount_point == device.mount_point) {
                        devices.push(device);
                    }
                });
                // Auto-select if no device is currently selected
                if read_selected_device_connected.get_untracked().is_none() {
                    set_selected_device_connected.set(Some(device_for_select));
                }
                notifications.success(format!("Device \"{device_name}\" connected"));
            }
        })
        .await
    {
        leptos::logging::error!("Failed to listen for device-connected events: {}", e);
    }

    // Listen for device disconnected events
    let set_devices_disconnected = set_devices;
    let set_selected_device_disconnected = set_selected_device;
    if let Err(e) = tauri_api::listen_to_event(
        tauri_api::device_events::DEVICE_DISCONNECTED,
        move |event| {
            leptos::logging::log!("Device disconnected event received");
            // Parse the device from the event payload
            if let Ok(payload) = js_sys::Reflect::get(&event, &JsValue::from_str("payload"))
                && let Ok(device) = serde_wasm_bindgen::from_value::<DeviceInfo>(payload)
            {
                leptos::logging::log!(
                    "Device disconnected: {} at {}",
                    device.name,
                    device.mount_point
                );
                let device_name = device.name.clone();
                // Remove the device from the list
                set_devices_disconnected.update(|devices| {
                    devices.retain(|d| d.mount_point != device.mount_point);
                });
                // Clear selection if the disconnected device was selected
                set_selected_device_disconnected.update(|selected| {
                    if let Some(sel) = selected
                        && sel.mount_point == device.mount_point
                    {
                        *selected = None;
                    }
                });
                notifications.info(format!("Device \"{device_name}\" disconnected"));
            }
        },
    )
    .await
    {
        leptos::logging::error!("Failed to listen for device-disconnected events: {}", e);
    }

    // Listen for devices refreshed events (initial device list)
    let set_devices_refreshed = set_devices;
    let set_device_list_state_refreshed = set_device_list_state;
    let read_selected_device_refreshed = selected_device;
    let set_selected_device_refreshed = set_selected_device;
    if let Err(e) =
        tauri_api::listen_to_event(tauri_api::device_events::DEVICES_REFRESHED, move |event| {
            leptos::logging::log!("Devices refreshed event received");
            // Parse the device list from the event payload
            match js_sys::Reflect::get(&event, &JsValue::from_str("payload")) {
                Ok(payload) => {
                    match serde_wasm_bindgen::from_value::<Vec<DeviceInfo>>(payload.clone()) {
                        Ok(devices) => {
                            leptos::logging::log!("Devices refreshed: {} devices", devices.len());
                            // Auto-select first device if none selected
                            if read_selected_device_refreshed.get_untracked().is_none()
                                && let Some(first) = devices.first()
                            {
                                set_selected_device_refreshed.set(Some(first.clone()));
                            }
                            set_devices_refreshed.set(devices);
                            set_device_list_state_refreshed.set(LoadingState::Loaded);
                        }
                        Err(e) => {
                            leptos::logging::error!(
                                "Failed to parse devices from payload: {:?}",
                                e
                            );
                            // Log the payload for debugging
                            if let Some(json_str) = js_sys::JSON::stringify(&payload)
                                .ok()
                                .and_then(|s| s.as_string())
                            {
                                leptos::logging::error!("Payload was: {}", json_str);
                            }
                        }
                    }
                }
                Err(e) => {
                    leptos::logging::error!("Failed to get payload from event: {:?}", e);
                }
            }
        })
        .await
    {
        leptos::logging::error!("Failed to listen for devices-refreshed events: {}", e);
    }

    // Now start the device watcher AFTER event listeners are set up
    leptos::logging::log!("Starting device watcher...");
    match tauri_api::start_device_watcher().await {
        Ok(started) => {
            if started {
                leptos::logging::log!("Device watcher started successfully");
            } else {
                leptos::logging::log!("Device watcher was already running");
            }
        }
        Err(e) => {
            leptos::logging::error!("Failed to start device watcher: {}", e);
        }
    }

    // Listen for sync progress events
    let set_transfer_progress_listener = set_transfer_progress;
    let set_transfer_panel_state_listener = set_transfer_panel_state;
    if let Err(e) = tauri_api::listen_to_sync_progress(move |progress| {
        leptos::logging::log!(
            "Sync progress: {}% - {}",
            progress.overall_progress_percent,
            progress.current_file_name
        );
        // Convert SyncProgressPayload to TransferProgress
        let status = match progress.status.as_str() {
            "Preparing" => TransferStatus::Preparing,
            "Transferring" => TransferStatus::Transferring,
            "Verifying" => TransferStatus::Verifying,
            "Completed" => TransferStatus::Completed,
            "Failed" => TransferStatus::Failed,
            "Cancelled" => TransferStatus::Cancelled,
            _ => TransferStatus::Transferring,
        };
        let transfer_progress = TransferProgress {
            status,
            current_file_index: progress.current_file_index,
            total_files: progress.total_files,
            current_file_name: progress.current_file_name,
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
        };
        set_transfer_progress_listener.set(Some(transfer_progress));
        set_transfer_panel_state_listener.set(TransferPanelState::from_status(status));
    })
    .await
    {
        leptos::logging::error!("Failed to listen for sync-progress events: {}", e);
    }

    // Listen for sync completed events
    let set_transfer_panel_state_completed = set_transfer_panel_state;
    let set_syncing_completed = set_syncing;
    let set_current_sync_task_id_completed = set_current_sync_task_id;
    if let Err(e) = tauri_api::listen_to_sync_completed(move |result| {
        leptos::logging::log!(
            "Sync completed: {} files transferred",
            result.files_transferred
        );
        set_transfer_panel_state_completed.set(TransferPanelState::Completed);
        set_syncing_completed.set(false);
        set_current_sync_task_id_completed.set(None);
        notifications.success(format!(
            "Transfer complete: {} files transferred to {}",
            result.files_transferred, result.device_mount_point
        ));
    })
    .await
    {
        leptos::logging::error!("Failed to listen for sync-completed events: {}", e);
    }

    // Listen for sync failed events
    let set_transfer_panel_state_failed = set_transfer_panel_state;
    let set_syncing_failed = set_syncing;
    let set_current_sync_task_id_failed = set_current_sync_task_id;
    if let Err(e) = tauri_api::listen_to_sync_failed(move |result| {
        let error_msg = result
            .error_message
            .unwrap_or_else(|| "Unknown error".to_string());
        leptos::logging::error!("Sync failed: {}", error_msg);
        set_transfer_panel_state_failed.set(TransferPanelState::Failed(error_msg.clone()));
        set_syncing_failed.set(false);
        set_current_sync_task_id_failed.set(None);
        notifications.error(format!("Transfer failed: {error_msg}"));
    })
    .await
    {
        leptos::logging::error!("Failed to listen for sync-failed events: {}", e);
    }

    // Listen for sync cancelled events
    let set_transfer_panel_state_cancelled = set_transfer_panel_state;
    let set_syncing_cancelled = set_syncing;
    let set_current_sync_task_id_cancelled = set_current_sync_task_id;
    if let Err(e) = tauri_api::listen_to_sync_cancelled(move |_result| {
        leptos::logging::log!("Sync cancelled by user");
        set_transfer_panel_state_cancelled.set(TransferPanelState::Cancelled);
        set_syncing_cancelled.set(false);
        set_current_sync_task_id_cancelled.set(None);
        notifications.info("Transfer cancelled");
    })
    .await
    {
        leptos::logging::error!("Failed to listen for sync-cancelled events: {}", e);
    }

    // ===== YouTube Download Event Listeners =====

    // Listen for download started events
    let set_download_panel_state_started = set_download_panel_state;
    let set_current_download_task_id_started = set_current_download_task_id;
    if let Err(e) = tauri_api::listen_to_download_started(move |task_id| {
        leptos::logging::log!("Download started: task_id={:?}", task_id);
        set_download_panel_state_started.set(DownloadPanelState::Downloading);
        set_current_download_task_id_started.set(Some(task_id));
    })
    .await
    {
        leptos::logging::error!("Failed to listen for download-started events: {}", e);
    }

    // Listen for download progress events
    let set_download_progress_listener = set_download_progress;
    let set_download_panel_state_progress = set_download_panel_state;
    let prev_videos_failed = std::cell::Cell::new(0usize);
    if let Err(e) = tauri_api::listen_to_download_progress(move |progress| {
        leptos::logging::log!(
            "Download progress: {}/{} videos, current: {}",
            progress.current_index,
            progress.total_videos,
            progress.current_title
        );
        // Reset counter if a new download started
        if progress.videos_failed < prev_videos_failed.get() {
            prev_videos_failed.set(0);
        }
        // Show a toast when a video fails to download
        if progress.videos_failed > prev_videos_failed.get() {
            let reason = if progress.status.starts_with("failed: ") {
                progress.status.trim_start_matches("failed: ").to_string()
            } else {
                "Unknown error".to_string()
            };
            notifications.warning(format!(
                "Failed to download \"{}\": {}",
                progress.current_title, reason
            ));
        }
        prev_videos_failed.set(progress.videos_failed);
        set_download_progress_listener.set(Some(progress));
        set_download_panel_state_progress.set(DownloadPanelState::Downloading);
    })
    .await
    {
        leptos::logging::error!("Failed to listen for download-progress events: {}", e);
    }

    // Listen for download completed events
    let set_download_panel_state_completed = set_download_panel_state;
    let set_current_download_task_id_completed = set_current_download_task_id;
    let load_playlists_download = load_playlists;
    let set_detail_refresh_trigger_download = set_detail_refresh_trigger;
    if let Err(e) = tauri_api::listen_to_download_completed(move |result| {
        leptos::logging::log!(
            "Download completed: {} successful, {} failed, {} skipped",
            result.successful_count,
            result.failed_count,
            result.skipped_count
        );
        set_download_panel_state_completed.set(DownloadPanelState::Completed);
        set_current_download_task_id_completed.set(None);
        set_downloading_playlist.set(None);
        notifications.success(format!(
            "Downloaded {} track{}",
            result.successful_count,
            if result.successful_count == 1 {
                ""
            } else {
                "s"
            }
        ));
        // Refresh playlists to show downloaded tracks
        load_playlists_download();
        // Trigger detail view refresh
        set_detail_refresh_trigger_download.update(|v| *v += 1);
    })
    .await
    {
        leptos::logging::error!("Failed to listen for download-completed events: {}", e);
    }

    // Listen for download failed events
    let set_download_panel_state_failed = set_download_panel_state;
    let set_current_download_task_id_failed = set_current_download_task_id;
    if let Err(e) = tauri_api::listen_to_download_failed(move |result| {
        let error_info = DownloadErrorInfo::from_result(&result);
        leptos::logging::error!(
            "Download failed: {} - {}",
            error_info.title,
            error_info.description
        );
        set_download_panel_state_failed.set(DownloadPanelState::Failed(error_info.clone()));
        set_current_download_task_id_failed.set(None);
        set_downloading_playlist.set(None);
        notifications.error(format!("Download failed: {}", error_info.title));
    })
    .await
    {
        leptos::logging::error!("Failed to listen for download-failed events: {}", e);
    }

    // Listen for download cancelled events
    let set_download_panel_state_cancelled = set_download_panel_state;
    let set_current_download_task_id_cancelled = set_current_download_task_id;
    if let Err(e) = tauri_api::listen_to_download_cancelled(move |_result| {
        leptos::logging::log!("Download cancelled by user");
        set_download_panel_state_cancelled.set(DownloadPanelState::Cancelled);
        set_current_download_task_id_cancelled.set(None);
        set_downloading_playlist.set(None);
        notifications.info("Download cancelled");
    })
    .await
    {
        leptos::logging::error!("Failed to listen for download-cancelled events: {}", e);
    }
}
