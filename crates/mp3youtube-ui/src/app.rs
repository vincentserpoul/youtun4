//! Main application component.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::{DeviceList, Header, PlaylistCard};
use crate::tauri_api;
use crate::theme::generate_css_variables;
use crate::types::{DeviceInfo, PlaylistMetadata};

/// Main application component.
#[component]
pub fn App() -> impl IntoView {
    // State signals
    let (devices, set_devices) = signal::<Vec<DeviceInfo>>(vec![]);
    let (playlists, set_playlists) = signal::<Vec<PlaylistMetadata>>(vec![]);
    let (selected_device, set_selected_device) = signal::<Option<DeviceInfo>>(None);
    let (selected_playlist, set_selected_playlist) = signal::<Option<PlaylistMetadata>>(None);
    let (error_message, set_error_message) = signal::<Option<String>>(None);

    // Function to load devices
    let load_devices = move || {
        spawn_local(async move {
            leptos::logging::log!("Loading devices...");
            match tauri_api::list_devices().await {
                Ok(device_list) => {
                    leptos::logging::log!("SUCCESS: Found {} devices", device_list.len());
                    for (i, dev) in device_list.iter().enumerate() {
                        leptos::logging::log!("  Device {}: {} at {}", i, dev.name, dev.mount_point);
                    }
                    set_devices.set(device_list);
                    set_error_message.set(None);
                }
                Err(e) => {
                    leptos::logging::error!("FAILED to load devices: {}", e);
                    set_error_message.set(Some(format!("Failed to load devices: {e}")));
                }
            }
        });
    };

    // Function to load playlists
    let load_playlists = move || {
        spawn_local(async move {
            leptos::logging::log!("Loading playlists...");
            match tauri_api::list_playlists().await {
                Ok(playlist_list) => {
                    leptos::logging::log!("Found {} playlists", playlist_list.len());
                    set_playlists.set(playlist_list);
                }
                Err(e) => {
                    leptos::logging::error!("Failed to load playlists: {}", e);
                    set_error_message.set(Some(format!("Failed to load playlists: {e}")));
                }
            }
        });
    };

    // Load data on mount
    Effect::new(move || {
        load_devices();
        load_playlists();
    });

    // Callbacks
    let on_device_select = Callback::new(move |device: DeviceInfo| {
        set_selected_device.set(Some(device));
    });

    let on_device_refresh = Callback::new(move |_| {
        load_devices();
    });

    let on_playlist_select = Callback::new(move |playlist: PlaylistMetadata| {
        set_selected_playlist.set(Some(playlist));
    });

    let on_playlist_delete = Callback::new(move |name: String| {
        spawn_local(async move {
            leptos::logging::log!("Deleting playlist: {}", name);
            match tauri_api::delete_playlist(&name).await {
                Ok(()) => {
                    leptos::logging::log!("Playlist deleted successfully");
                    // Reload playlists
                    if let Ok(playlist_list) = tauri_api::list_playlists().await {
                        set_playlists.set(playlist_list);
                    }
                }
                Err(e) => {
                    leptos::logging::error!("Failed to delete playlist: {}", e);
                    set_error_message.set(Some(format!("Failed to delete playlist: {e}")));
                }
            }
        });
    });

    let on_playlist_sync = Callback::new(move |name: String| {
        let selected = selected_device.get();
        spawn_local(async move {
            if let Some(device) = selected {
                leptos::logging::log!("Syncing playlist {} to {}", name, device.mount_point);
                match tauri_api::sync_playlist(&name, &device.mount_point).await {
                    Ok(()) => {
                        leptos::logging::log!("Playlist synced successfully");
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to sync playlist: {}", e);
                        set_error_message.set(Some(format!("Failed to sync playlist: {e}")));
                    }
                }
            } else {
                set_error_message.set(Some("Please select a device first".to_string()));
            }
        });
    });

    // CSS variables
    let css_vars = generate_css_variables();

    view! {
        <style>{css_vars}</style>
        <style>{include_str!("../styles/main.css")}</style>
        <div class="app">
            <Header />
            // Error banner
            {move || {
                error_message.get().map(|msg| {
                    view! {
                        <div class="error-banner">
                            <span>{msg}</span>
                            <button
                                class="btn btn-ghost btn-icon"
                                on:click=move |_| set_error_message.set(None)
                            >
                                "×"
                            </button>
                        </div>
                    }
                })
            }}
            <main class="app-main">
                <aside class="sidebar">
                    <DeviceList
                        devices=devices
                        selected_device=selected_device
                        on_select=on_device_select
                        on_refresh=on_device_refresh
                    />
                </aside>
                <section class="content">
                    <div class="content-header">
                        <h2>"Playlists"</h2>
                        <button class="btn btn-primary">
                            <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
                            </svg>
                            "New Playlist"
                        </button>
                    </div>
                    <div class="playlist-grid">
                        {move || {
                            let playlist_list = playlists.get();
                            if playlist_list.is_empty() {
                                view! {
                                    <div class="empty-state">
                                        <svg viewBox="0 0 24 24" width="64" height="64" fill="var(--text-disabled)">
                                            <path d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/>
                                        </svg>
                                        <h3>"No playlists yet"</h3>
                                        <p>"Create a playlist from a YouTube URL to get started"</p>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="playlists">
                                        {playlist_list.into_iter().map(|playlist| {
                                            let is_selected = selected_playlist.get()
                                                .as_ref()
                                                .map(|s| s.name == playlist.name)
                                                .unwrap_or(false);
                                            view! {
                                                <PlaylistCard
                                                    playlist=playlist
                                                    on_select=on_playlist_select
                                                    on_delete=on_playlist_delete
                                                    on_sync=on_playlist_sync
                                                    selected=is_selected
                                                />
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }
                        }}
                    </div>
                </section>
            </main>
        </div>
    }
}
