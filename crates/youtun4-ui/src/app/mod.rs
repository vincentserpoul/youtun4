//! Main application component.

mod listeners;

use leptos::prelude::*;
use leptos::task::spawn_local;

use listeners::{ListenerContext, register_event_listeners};

use crate::components::{
    ContentHeader, CreatePlaylistDialog, DeletePlaylistDialog, DeviceSelectorPanel,
    DeviceStatusIndicator, DownloadPanelState, DownloadProgressPanel, Layout, LayoutMain,
    LayoutSidebar, LoadingState, NotificationProvider, PlaylistDetailView, PlaylistListState,
    PlaylistSelectionList, PlaylistSelectionState, PlaylistSelectionSummary, PlaylistTable,
    SettingsPanel, TransferPanelState, TransferProgressPanel, use_notifications,
};
use crate::tauri_api;
use crate::theme::generate_css_variables;
use crate::types::{DeviceInfo, DownloadProgress, PlaylistMetadata, TaskId, TransferProgress};

/// Main application component.
#[component]

pub fn App() -> impl IntoView {
    // CSS variables
    let css_vars = generate_css_variables();

    view! {
        <style>{css_vars}</style>
        <style>{include_str!("../../styles/main.css")}</style>
        <NotificationProvider>
            <AppContent />
        </NotificationProvider>
    }
}

/// Inner application content with access to notification context.
#[component]
fn AppContent() -> impl IntoView {
    // Get notification context
    let notifications = use_notifications();

    // State signals
    let (devices, set_devices) = signal::<Vec<DeviceInfo>>(vec![]);
    let (playlists, set_playlists) = signal::<Vec<PlaylistMetadata>>(vec![]);
    let (selected_device, set_selected_device) = signal::<Option<DeviceInfo>>(None);
    let (selected_playlist, set_selected_playlist) = signal::<Option<PlaylistMetadata>>(None);
    let (settings_open, set_settings_open) = signal(false);
    let (playlist_list_state, set_playlist_list_state) = signal(PlaylistListState::Loading);
    let (playlist_error, set_playlist_error) = signal::<Option<String>>(None);

    // Device list loading state
    let (device_list_state, set_device_list_state) = signal(LoadingState::Loading);

    // Device selector panel visibility
    let (selector_open, set_selector_open) = signal(false);

    // Delete confirmation dialog state
    let (delete_dialog_open, set_delete_dialog_open) = signal(false);
    let (delete_playlist_name, set_delete_playlist_name) = signal::<Option<String>>(None);
    let (delete_playlist_track_count, set_delete_playlist_track_count) =
        signal::<Option<usize>>(None);
    let (delete_playlist_total_bytes, set_delete_playlist_total_bytes) =
        signal::<Option<u64>>(None);
    let (delete_playlist_source_url, set_delete_playlist_source_url) =
        signal::<Option<String>>(None);

    // Create playlist dialog state
    let (create_dialog_open, set_create_dialog_open) = signal(false);

    // View mode: true = selection mode (for syncing), false = management mode (list with actions)
    let (selection_mode, set_selection_mode) = signal(false);

    // Detail view state: Some(playlist_name) = viewing playlist detail, None = viewing list
    let (detail_view_playlist, set_detail_view_playlist) = signal::<Option<String>>(None);

    // Syncing state for the sync button
    let (_syncing, set_syncing) = signal(false);

    // Transfer progress state
    let (transfer_progress, set_transfer_progress) = signal::<Option<TransferProgress>>(None);
    let (transfer_panel_state, set_transfer_panel_state) = signal(TransferPanelState::Idle);
    let (current_sync_task_id, set_current_sync_task_id) = signal::<Option<TaskId>>(None);

    // Download progress state
    let (download_progress, set_download_progress) = signal::<Option<DownloadProgress>>(None);
    let (download_panel_state, set_download_panel_state) = signal(DownloadPanelState::Idle);
    let (_current_download_task_id, set_current_download_task_id) = signal::<Option<TaskId>>(None);

    // Refresh trigger for detail view (incremented when download completes)
    let (detail_refresh_trigger, set_detail_refresh_trigger) = signal(0u32);

    // Track which playlist is currently being downloaded
    let (downloading_playlist, set_downloading_playlist) = signal::<Option<String>>(None);

    // Function to load devices
    let load_devices = move || {
        set_device_list_state.set(LoadingState::Loading);
        spawn_local(async move {
            leptos::logging::log!("=== LOAD DEVICES START ===");
            match tauri_api::list_devices().await {
                Ok(device_list) => {
                    leptos::logging::log!(
                        "=== LOAD DEVICES SUCCESS: {} devices ===",
                        device_list.len()
                    );
                    for (i, dev) in device_list.iter().enumerate() {
                        leptos::logging::log!(
                            "  Device {}: name='{}' mount_point='{}'",
                            i,
                            dev.name,
                            dev.mount_point
                        );
                    }
                    set_devices.set(device_list);
                    set_device_list_state.set(LoadingState::Loaded);
                    leptos::logging::log!("=== LOAD DEVICES: Signals updated ===");
                }
                Err(e) => {
                    leptos::logging::error!("=== LOAD DEVICES FAILED: {} ===", e);
                    set_device_list_state.set(LoadingState::Error);
                    notifications.error(format!("Failed to load devices: {e}"));
                }
            }
        });
    };

    // Function to load playlists
    let load_playlists = move || {
        set_playlist_list_state.set(PlaylistListState::Loading);
        set_playlist_error.set(None);
        spawn_local(async move {
            leptos::logging::log!("Loading playlists...");
            match tauri_api::list_playlists().await {
                Ok(playlist_list) => {
                    leptos::logging::log!("Found {} playlists", playlist_list.len());
                    set_playlists.set(playlist_list);
                    set_playlist_list_state.set(PlaylistListState::Loaded);
                }
                Err(e) => {
                    leptos::logging::error!("Failed to load playlists: {}", e);
                    set_playlist_error.set(Some(e.clone()));
                    set_playlist_list_state.set(PlaylistListState::Error);
                    notifications.error(format!("Failed to load playlists: {e}"));
                }
            }
        });
    };

    // Load data on mount and start device watcher
    Effect::new(move || {
        load_devices();
        load_playlists();

        // Start the device watcher for automatic device detection
        // IMPORTANT: Set up event listeners BEFORE starting the watcher to avoid missing
        // the initial "devices-refreshed" event that is emitted immediately on watcher start.
        spawn_local(register_event_listeners(ListenerContext {
            notifications,
            set_devices,
            selected_device,
            set_selected_device,
            set_device_list_state,
            set_transfer_progress,
            set_transfer_panel_state,
            set_syncing,
            set_current_sync_task_id,
            set_download_panel_state,
            set_current_download_task_id,
            set_download_progress,
            set_downloading_playlist,
            set_detail_refresh_trigger,
            load_playlists: Box::new(load_playlists),
        }));
    });

    // Auto-open device selector when no device selected but devices are available
    Effect::new(move |_| {
        let device = selected_device.get();
        let device_list = devices.get();
        let state = device_list_state.get();

        if state == LoadingState::Loaded {
            if device.is_none() && !device_list.is_empty() {
                set_selector_open.set(true);
            } else if let Some(sel) = device {
                // Open if selected device is no longer in the list
                if !device_list.iter().any(|d| d.mount_point == sel.mount_point) {
                    set_selector_open.set(true);
                }
            }
        }
    });

    // Callbacks
    let on_device_refresh = Callback::new(move |()| {
        load_devices();
    });

    let on_device_select = Callback::new(move |device: DeviceInfo| {
        set_selected_device.set(Some(device));
        set_selector_open.set(false);
    });

    let on_select_device_click = Callback::new(move |()| {
        set_selector_open.update(|v| *v = !*v);
    });

    let on_device_eject = Callback::new(move |_mount_point: String| {
        load_devices();
    });

    let on_playlist_select = Callback::new(move |playlist: PlaylistMetadata| {
        set_selected_playlist.set(Some(playlist.clone()));
        // Navigate to detail view when clicking a playlist in management mode
        if !selection_mode.get() {
            set_detail_view_playlist.set(Some(playlist.name));
        }
    });

    // Callback to go back from detail view to list view
    let on_detail_back = Callback::new(move |()| {
        set_detail_view_playlist.set(None);
    });

    // Callback when sync is requested from detail view
    let on_detail_sync = {
        Callback::new(move |name: String| {
            let selected = selected_device.get();
            let name_for_notification = name.clone();
            if let Some(device) = selected {
                leptos::logging::log!("Syncing playlist {} to {}", name, device.mount_point);
                notifications.info(format!(
                    "Syncing \"{}\" to {}...",
                    name_for_notification, device.name
                ));
                let name_clone = name;
                let device_mount = device.mount_point;
                spawn_local(async move {
                    match tauri_api::sync_playlist(&name_clone, &device_mount).await {
                        Ok(()) => {
                            leptos::logging::log!("Playlist synced successfully");
                            notifications.success(format!(
                                "\"{name_for_notification}\" synced successfully"
                            ));
                        }
                        Err(e) => {
                            leptos::logging::error!("Failed to sync playlist: {}", e);
                            notifications.error(format!("Failed to sync playlist: {e}"));
                        }
                    }
                });
            } else {
                notifications.warning("Please select a device first");
            }
        })
    };

    // Callback when delete is requested from detail view
    let on_detail_delete = Callback::new(move |name: String| {
        // Find the playlist to get its details
        let playlist = playlists.get().iter().find(|p| p.name == name).cloned();

        if let Some(p) = playlist {
            set_delete_playlist_name.set(Some(p.name));
            set_delete_playlist_track_count.set(Some(p.track_count));
            set_delete_playlist_total_bytes.set(Some(p.total_bytes));
            set_delete_playlist_source_url.set(p.source_url);
        } else {
            set_delete_playlist_name.set(Some(name));
            set_delete_playlist_track_count.set(None);
            set_delete_playlist_total_bytes.set(None);
            set_delete_playlist_source_url.set(None);
        }
        set_delete_dialog_open.set(true);
    });

    // Handler when delete button is clicked on a playlist card - opens confirmation dialog
    let on_playlist_delete_request = Callback::new(move |name: String| {
        // Find the playlist to get its details
        let playlist = playlists.get().iter().find(|p| p.name == name).cloned();

        if let Some(p) = playlist {
            set_delete_playlist_name.set(Some(p.name));
            set_delete_playlist_track_count.set(Some(p.track_count));
            set_delete_playlist_total_bytes.set(Some(p.total_bytes));
            set_delete_playlist_source_url.set(p.source_url);
        } else {
            set_delete_playlist_name.set(Some(name));
            set_delete_playlist_track_count.set(None);
            set_delete_playlist_total_bytes.set(None);
            set_delete_playlist_source_url.set(None);
        }
        set_delete_dialog_open.set(true);
    });

    // Handler when delete is confirmed in the dialog
    let on_delete_confirm = Callback::new(move |()| {
        if let Some(name) = delete_playlist_name.get() {
            let name_clone = name.clone();
            let name_for_notification = name.clone();
            // If we're in detail view of this playlist, navigate back to list
            if detail_view_playlist.get().as_ref() == Some(&name) {
                set_detail_view_playlist.set(None);
            }
            spawn_local(async move {
                leptos::logging::log!("Deleting playlist: {}", name_clone);
                match tauri_api::delete_playlist(&name_clone).await {
                    Ok(()) => {
                        leptos::logging::log!("Playlist deleted successfully");
                        notifications
                            .success(format!("Playlist \"{name_for_notification}\" deleted"));
                        // Reload playlists
                        if let Ok(playlist_list) = tauri_api::list_playlists().await {
                            set_playlists.set(playlist_list);
                        }
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to delete playlist: {}", e);
                        notifications.error(format!("Failed to delete playlist: {e}"));
                    }
                }
            });
        }
        // Close the dialog
        set_delete_dialog_open.set(false);
        set_delete_playlist_name.set(None);
        set_delete_playlist_track_count.set(None);
        set_delete_playlist_total_bytes.set(None);
        set_delete_playlist_source_url.set(None);
    });

    // Handler when delete is cancelled
    let on_delete_cancel = Callback::new(move |()| {
        set_delete_dialog_open.set(false);
        set_delete_playlist_name.set(None);
        set_delete_playlist_track_count.set(None);
        set_delete_playlist_total_bytes.set(None);
        set_delete_playlist_source_url.set(None);
    });

    let on_playlist_sync = Callback::new(move |name: String| {
        let selected = selected_device.get();
        let name_for_notification = name.clone();
        spawn_local(async move {
            if let Some(device) = selected {
                leptos::logging::log!("Syncing playlist {} to {}", name, device.mount_point);
                notifications.info(format!(
                    "Syncing \"{}\" to {}...",
                    name_for_notification, device.name
                ));
                match tauri_api::sync_playlist(&name, &device.mount_point).await {
                    Ok(()) => {
                        leptos::logging::log!("Playlist synced successfully");
                        notifications
                            .success(format!("\"{name_for_notification}\" synced successfully"));
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to sync playlist: {}", e);
                        notifications.error(format!("Failed to sync playlist: {e}"));
                    }
                }
            } else {
                notifications.warning("Please select a device first");
            }
        });
    });

    // Download playlist callback
    let on_playlist_download = Callback::new(move |name: String| {
        if let Some(playlist) = playlists.get().iter().find(|p| p.name == name).cloned() {
            if let Some(url) = playlist.source_url {
                let name_for_notification = name.clone();
                set_downloading_playlist.set(Some(name.clone()));
                spawn_local(async move {
                    match tauri_api::download_youtube_to_playlist(&url, &name).await {
                        Ok(_task_id) => {
                            leptos::logging::log!("Download started for playlist: {}", name);
                            notifications
                                .info(format!("Downloading \"{name_for_notification}\"..."));
                        }
                        Err(e) => {
                            leptos::logging::error!("Failed to start download: {}", e);
                            notifications.error(format!("Failed to start download: {e}"));
                            set_downloading_playlist.set(None);
                        }
                    }
                });
            } else {
                notifications.warning("No source URL available for this playlist");
            }
        }
    });

    // Settings callbacks
    let on_settings_close = Callback::new(move |()| {
        set_settings_open.set(false);
    });

    // Playlist list retry callback
    let on_playlist_retry = Callback::new(move |()| {
        load_playlists();
    });

    // Create playlist dialog callbacks
    let on_create_playlist = Callback::new(move |name: String| {
        leptos::logging::log!("Playlist created: {}", name);
        notifications.success(format!("Playlist \"{name}\" created"));
        // Reload playlists to include the new one
        load_playlists();
    });

    let on_create_dialog_close = Callback::new(move |()| {
        set_create_dialog_open.set(false);
    });

    // Sync selected playlist callback (used in selection mode)
    let on_sync_selected = move |_: web_sys::MouseEvent| {
        let selected_pl = selected_playlist.get();
        let selected_dev = selected_device.get();

        match (selected_pl, selected_dev) {
            (Some(playlist), Some(device)) => {
                let name = playlist.name.clone();
                let name_for_notification = playlist.name;
                let device_name = device.name.clone();
                let mount_point = device.mount_point;
                notifications.info(format!(
                    "Starting transfer of \"{name_for_notification}\" to {device_name}..."
                ));
                set_syncing.set(true);
                set_transfer_panel_state.set(TransferPanelState::Preparing);
                set_transfer_progress.set(None);
                spawn_local(async move {
                    leptos::logging::log!("Starting sync for playlist {} to {}", name, mount_point);
                    match tauri_api::start_sync(&name, &mount_point, false, true).await {
                        Ok(task_id) => {
                            leptos::logging::log!("Sync started with task ID: {:?}", task_id);
                            set_current_sync_task_id.set(Some(task_id));
                            // Progress updates will be handled by event listeners
                        }
                        Err(e) => {
                            leptos::logging::error!("Failed to start sync: {}", e);
                            notifications.error(format!("Failed to start transfer: {e}"));
                            set_syncing.set(false);
                            set_transfer_panel_state.set(TransferPanelState::Failed(e));
                        }
                    }
                });
                // Exit selection mode after starting sync
                set_selection_mode.set(false);
            }
            (None, _) => {
                notifications.warning("Please select a playlist first");
            }
            (_, None) => {
                notifications.warning("Please select a device first");
            }
        }
    };

    // Cancel selection mode
    let cancel_selection = move |_: web_sys::MouseEvent| {
        set_selection_mode.set(false);
        set_selected_playlist.set(None);
    };

    // Transfer progress panel callbacks
    let on_transfer_cancel = Callback::new(move |task_id: TaskId| {
        leptos::logging::log!("Cancelling transfer task: {:?}", task_id);
        spawn_local(async move {
            match tauri_api::cancel_sync(task_id).await {
                Ok(cancelled) => {
                    if cancelled {
                        leptos::logging::log!("Transfer cancellation requested");
                    } else {
                        leptos::logging::log!(
                            "Transfer could not be cancelled (may have already completed)"
                        );
                    }
                }
                Err(e) => {
                    leptos::logging::error!("Failed to cancel transfer: {}", e);
                    notifications.error(format!("Failed to cancel transfer: {e}"));
                }
            }
        });
    });

    let on_transfer_dismiss = Callback::new(move |(): ()| {
        set_transfer_panel_state.set(TransferPanelState::Idle);
        set_transfer_progress.set(None);
        set_current_sync_task_id.set(None);
    });

    // Download progress panel callbacks
    let on_download_cancel = Callback::new(move |task_id: TaskId| {
        leptos::logging::log!("Cancelling download task: {:?}", task_id);
        spawn_local(async move {
            match tauri_api::cancel_download(task_id).await {
                Ok(cancelled) => {
                    if cancelled {
                        leptos::logging::log!("Download cancellation requested");
                    } else {
                        leptos::logging::log!(
                            "Download could not be cancelled (may have already completed)"
                        );
                    }
                }
                Err(e) => {
                    leptos::logging::error!("Failed to cancel download: {}", e);
                    notifications.error(format!("Failed to cancel download: {e}"));
                }
            }
        });
    });

    let on_download_dismiss = Callback::new(move |(): ()| {
        set_download_panel_state.set(DownloadPanelState::Idle);
        set_download_progress.set(None);
        set_current_download_task_id.set(None);
    });

    view! {
        <Layout on_settings_click=Callback::new(move |()| set_settings_open.set(true))>
            <LayoutSidebar>
                <DeviceStatusIndicator device=selected_device />
                <div class="device-selector-wrapper">
                    <div class="panel-header">
                        <h3 class="panel-title">"SELECT DEVICE"</h3>
                        <button
                            class="btn btn-ghost btn-icon header-action-icon"
                            class:active=move || selector_open.get()
                            on:click=move |_| on_select_device_click.run(())
                            title="Toggle device list"
                            data-testid="select-device-button"
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor" class="chevron-icon">
                                <path d="M7.41 8.59L12 13.17l4.59-4.58L18 10l-6 6-6-6z"/>
                            </svg>
                        </button>
                    </div>
                    <DeviceSelectorPanel
                        devices=devices
                        selected_device=selected_device
                        on_select=on_device_select
                        on_eject=on_device_eject
                        on_refresh=on_device_refresh
                        state=device_list_state
                        open=selector_open
                    />
                </div>
            </LayoutSidebar>
            <LayoutMain>
                // Download Progress Panel (inline in content area)
                <DownloadProgressPanel
                    progress=download_progress
                    state=download_panel_state
                    on_cancel=on_download_cancel
                    on_dismiss=on_download_dismiss
                />

                // Content switches between management mode, selection mode, and detail view
                {move || {
                    if let Some(playlist_name) = detail_view_playlist.get() {
                        let is_dl = downloading_playlist.get().as_ref().is_some_and(|n| *n == playlist_name);
                        view! {
                            <PlaylistDetailView
                                playlist_name=playlist_name
                                on_back=on_detail_back
                                on_sync=on_detail_sync
                                on_download=on_playlist_download
                                on_delete=on_detail_delete
                                is_downloading=is_dl
                                refresh_trigger=detail_refresh_trigger.into()
                            />
                        }.into_any()
                    } else if selection_mode.get() {
                        let selection_state = match playlist_list_state.get() {
                            PlaylistListState::Loading => PlaylistSelectionState::Loading,
                            PlaylistListState::Loaded => PlaylistSelectionState::Loaded,
                            PlaylistListState::Error => PlaylistSelectionState::Error,
                        };
                        view! {
                            <ContentHeader title="Select Playlist to Sync">
                                <button
                                    class="btn btn-secondary"
                                    on:click=cancel_selection
                                >
                                    "Cancel"
                                </button>
                                <button
                                    class="btn btn-primary"
                                    on:click=on_sync_selected
                                    disabled=move || selected_playlist.get().is_none() || selected_device.get().is_none()
                                >
                                    <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                        <path d="M19 8l-4 4h3c0 3.31-2.69 6-6 6-1.01 0-1.97-.25-2.8-.7l-1.46 1.46C8.97 19.54 10.43 20 12 20c4.42 0 8-3.58 8-8h3l-4-4zM6 12c0-3.31 2.69-6 6-6 1.01 0 1.97.25 2.8.7l1.46-1.46C15.03 4.46 13.57 4 12 4c-4.42 0-8 3.58-8 8H1l4 4 4-4H6z"/>
                                    </svg>
                                    "Sync to Device"
                                </button>
                            </ContentHeader>
                            <PlaylistSelectionSummary selected=selected_playlist />
                            <div style="margin-top: var(--spacing-md)">
                                <PlaylistSelectionList
                                    playlists=playlists
                                    selected_playlist=selected_playlist
                                    on_select=on_playlist_select
                                    state=selection_state
                                    title="Choose a playlist".to_string()
                                    description="Select one playlist to sync to your connected device".to_string()
                                />
                            </div>
                        }.into_any()
                    } else {
                        // Dashboard mode: playlist table
                        view! {
                            <PlaylistTable
                                playlists=playlists
                                selected_device=selected_device
                                state=playlist_list_state.get()
                                error_message=playlist_error.get().unwrap_or_default()
                                on_select=on_playlist_select
                                on_delete=on_playlist_delete_request
                                on_sync=on_playlist_sync
                                on_download=on_playlist_download
                                on_retry=on_playlist_retry
                                on_create=Callback::new(move |()| set_create_dialog_open.set(true))
                                downloading_playlist=downloading_playlist
                            />
                        }.into_any()
                    }
                }}
            </LayoutMain>
        </Layout>

        // Settings Panel
        <SettingsPanel
            is_open=settings_open
            on_close=on_settings_close
        />

        // Delete Playlist Confirmation Dialog
        <DeletePlaylistDialog
            is_open=delete_dialog_open
            playlist_name=delete_playlist_name
            track_count=delete_playlist_track_count
            total_bytes=delete_playlist_total_bytes
            source_url=delete_playlist_source_url
            on_confirm=on_delete_confirm
            on_cancel=on_delete_cancel
        />

        // Create Playlist Dialog
        <CreatePlaylistDialog
            is_open=create_dialog_open
            on_create=on_create_playlist
            on_close=on_create_dialog_close
        />

        // Transfer Progress Panel
        <TransferProgressPanel
            progress=transfer_progress
            state=transfer_panel_state
            on_cancel=on_transfer_cancel
            on_dismiss=on_transfer_dismiss
            task_id=current_sync_task_id
        />
    }
}
