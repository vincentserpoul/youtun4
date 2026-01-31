//! Playlist card component.

use leptos::prelude::*;

use crate::types::{DeviceInfo, PlaylistMetadata};

/// Format bytes to human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Playlist card component.
#[component]

pub fn PlaylistCard(
    /// The playlist metadata to display.
    playlist: PlaylistMetadata,
    /// Callback when playlist is selected (for viewing details).
    on_select: Callback<PlaylistMetadata>,
    /// Callback when delete is requested.
    on_delete: Callback<String>,
    /// Callback when sync is requested.
    on_sync: Callback<String>,
    /// The currently selected device (for enabling/disabling sync).
    #[prop(optional)]
    selected_device: Option<ReadSignal<Option<DeviceInfo>>>,
    /// Whether this playlist is selected.
    #[prop(default = false)]
    selected: bool,
) -> impl IntoView {
    let playlist_clone = playlist.clone();
    let playlist_name = playlist.name.clone();
    let playlist_name_delete = playlist.name.clone();
    let playlist_name_sync = playlist.name.clone();
    let playlist_name_view = playlist.name.clone();

    // Check if device is connected
    let has_device = move || selected_device.is_some_and(|sig| sig.get().is_some());

    let device_name = move || selected_device.and_then(|sig| sig.get().map(|d| d.name));

    // Sync button with device context
    let sync_button = {
        let name = playlist_name_sync;
        view! {
            <button
                class=move || {
                    if has_device() {
                        "btn btn-sync"
                    } else {
                        "btn btn-sync no-device"
                    }
                }
                title=move || {
                    if let Some(dev_name) = device_name() {
                        format!("Sync to {dev_name}")
                    } else {
                        "Connect a device to sync".to_string()
                    }
                }
                disabled=move || !has_device()
                on:click=move |e| {
                    e.stop_propagation();
                    if has_device() {
                        on_sync.run(name.clone());
                    }
                }
            >
                <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                    <path d="M19 8l-4 4h3c0 3.31-2.69 6-6 6-1.01 0-1.97-.25-2.8-.7l-1.46 1.46C8.97 19.54 10.43 20 12 20c4.42 0 8-3.58 8-8h3l-4-4zM6 12c0-3.31 2.69-6 6-6 1.01 0 1.97.25 2.8.7l1.46-1.46C15.03 4.46 13.57 4 12 4c-4.42 0-8 3.58-8 8H1l4 4 4-4H6z"/>
                </svg>
                {move || {
                    if let Some(dev_name) = device_name() {
                        format!("Sync to {dev_name}")
                    } else {
                        "Connect device to sync".to_string()
                    }
                }}
            </button>
        }
    };

    // View details button
    let view_button = {
        let playlist_for_view = playlist.clone();
        let name = playlist_name_view;
        view! {
            <button
                class="btn btn-secondary"
                title=format!("View {} details", name)
                on:click=move |e| {
                    e.stop_propagation();
                    on_select.run(playlist_for_view.clone());
                }
            >
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                    <path d="M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z"/>
                </svg>
                "Details"
            </button>
        }
    };

    let delete_button = {
        let name = playlist_name_delete;
        view! {
            <button
                class="btn btn-icon btn-danger"
                title="Delete playlist"
                on:click=move |e| {
                    e.stop_propagation();
                    on_delete.run(name.clone());
                }
            >
                <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                    <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
                </svg>
            </button>
        }
    };

    let source_info = playlist.source_url.as_ref().map(|url| {
        view! {
            <div class="playlist-source" title=url.clone()>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                    <path d="M10 15l5.19-3L10 9v6m11.56-7.83c.13.47.22 1.1.28 1.9.07.8.1 1.49.1 2.09L22 12c0 2.19-.16 3.8-.44 4.83-.25.9-.83 1.48-1.73 1.73-.47.13-1.33.22-2.65.28-1.3.07-2.49.1-3.59.1L12 19c-4.19 0-6.8-.16-7.83-.44-.9-.25-1.48-.83-1.73-1.73-.13-.47-.22-1.1-.28-1.9-.07-.8-.1-1.49-.1-2.09L2 12c0-2.19.16-3.8.44-4.83.25-.9.83-1.48 1.73-1.73.47-.13 1.33-.22 2.65-.28 1.3-.07 2.49-.1 3.59-.1L12 5c4.19 0 6.8.16 7.83.44.9.25 1.48.83 1.73 1.73z"/>
                </svg>
                <span>"YouTube"</span>
            </div>
        }
    });

    // Thumbnail view - show image if URL available, otherwise show icon
    let thumbnail_url = playlist.thumbnail_url.clone();
    let thumbnail_view = move || {
        if let Some(ref url) = thumbnail_url {
            view! {
                <div class="playlist-thumbnail">
                    <img
                        src=url.clone()
                        alt="Playlist thumbnail"
                        class="playlist-thumbnail-img"
                        loading="lazy"
                    />
                </div>
            }
            .into_any()
        } else {
            view! {
                <div class="playlist-icon">
                    <svg viewBox="0 0 512 512" width="48" height="48">
                        <defs>
                            <linearGradient id="brandGrad-card" x1="0%" y1="0%" x2="100%" y2="100%">
                                <stop offset="0%" style="stop-color:#8b5cf6"/>
                                <stop offset="100%" style="stop-color:#ec4899"/>
                            </linearGradient>
                        </defs>
                        <rect x="0" y="0" width="512" height="512" rx="110" ry="110" fill="url(#brandGrad-card)"/>
                        <g transform="translate(125.5, 50) scale(9, 9)">
                            <path d="M14.4848 20C14.4848 20 23.5695 20 25.8229 19.4C27.0917 19.06 28.0459 18.08 28.3808 16.87C29 14.65 29 9.98 29 9.98C29 9.98 29 5.34 28.3808 3.14C28.0459 1.9 27.0917 0.94 25.8229 0.61C23.5695 0 14.4848 0 14.4848 0C14.4848 0 5.42037 0 3.17711 0.61C1.9286 0.94 0.954148 1.9 0.59888 3.14C0 5.34 0 9.98 0 9.98C0 9.98 0 14.65 0.59888 16.87C0.954148 18.08 1.9286 19.06 3.17711 19.4C5.42037 20 14.4848 20 14.4848 20Z" fill="white" opacity="0.95"/>
                            <path d="M19 10L11.5 5.75V14.25L19 10Z" fill="url(#brandGrad-card)"/>
                        </g>
                        <g opacity="0.9">
                            <circle cx="256" cy="300" r="30" fill="none" stroke="white" stroke-width="9"/>
                            <polygon points="243,270 256,257 269,270" fill="white"/>
                            <polygon points="269,330 256,343 243,330" fill="white"/>
                        </g>
                        <rect x="70" y="390" width="372" height="70" rx="16" ry="16" fill="white" opacity="0.9"/>
                        <circle cx="160" cy="425" r="11" fill="#8b5cf6"/>
                        <circle cx="256" cy="425" r="11" fill="#ec4899"/>
                        <circle cx="352" cy="425" r="11" fill="#ef4444"/>
                    </svg>
                </div>
            }.into_any()
        }
    };

    view! {
        <div
            class=move || if selected { "playlist-card selected" } else { "playlist-card" }
        >
            <div class="playlist-card-header" on:click=move |_| {
                on_select.run(playlist_clone.clone());
            }>
                {thumbnail_view}
                <div class="playlist-info">
                    <h4 class="playlist-name">{playlist_name}</h4>
                    <div class="playlist-meta">
                        <span class="track-count">{playlist.track_count} " tracks"</span>
                        <span class="separator">"•"</span>
                        <span class="size">{format_bytes(playlist.total_bytes)}</span>
                    </div>
                    {source_info}
                </div>
            </div>
            <div class="playlist-card-actions">
                {sync_button}
                <div class="playlist-card-secondary-actions">
                    {view_button}
                    {delete_button}
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }
}
