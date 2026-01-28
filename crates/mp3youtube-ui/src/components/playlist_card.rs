//! Playlist card component.

use leptos::prelude::*;

use crate::types::PlaylistMetadata;

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
    /// Callback when playlist is selected.
    on_select: Callback<PlaylistMetadata>,
    /// Callback when delete is requested.
    on_delete: Callback<String>,
    /// Callback when sync is requested.
    on_sync: Callback<String>,
    /// Whether this playlist is selected.
    #[prop(default = false)]
    selected: bool,
) -> impl IntoView {
    let playlist_clone = playlist.clone();
    let playlist_name = playlist.name.clone();
    let playlist_name_delete = playlist.name.clone();
    let playlist_name_sync = playlist.name.clone();

    // Pre-render the action buttons
    let sync_button = {
        let name = playlist_name_sync;
        view! {
            <button
                class="btn btn-icon btn-ghost"
                title="Sync to device"
                on:click=move |e| {
                    e.stop_propagation();
                    on_sync.run(name.clone());
                }
            >
                <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                    <path d="M19 8l-4 4h3c0 3.31-2.69 6-6 6-1.01 0-1.97-.25-2.8-.7l-1.46 1.46C8.97 19.54 10.43 20 12 20c4.42 0 8-3.58 8-8h3l-4-4zM6 12c0-3.31 2.69-6 6-6 1.01 0 1.97.25 2.8.7l1.46-1.46C15.03 4.46 13.57 4 12 4c-4.42 0-8 3.58-8 8H1l4 4 4-4H6z"/>
                </svg>
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
                <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
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

    view! {
        <div
            class=move || if selected { "playlist-card selected" } else { "playlist-card" }
            on:click=move |_| {
                on_select.run(playlist_clone.clone());
            }
        >
            <div class="playlist-icon">
                <svg viewBox="0 0 24 24" width="32" height="32" fill="currentColor">
                    <path d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/>
                </svg>
            </div>
            <div class="playlist-info">
                <h4 class="playlist-name">{playlist_name}</h4>
                <div class="playlist-meta">
                    <span class="track-count">{playlist.track_count} " tracks"</span>
                    <span class="separator">"•"</span>
                    <span class="size">{format_bytes(playlist.total_bytes)}</span>
                </div>
                {source_info}
            </div>
            <div class="playlist-actions">
                {sync_button}
                {delete_button}
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
