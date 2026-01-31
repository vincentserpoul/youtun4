//! Playlist list component for displaying playlists in a grid or list format.

use leptos::prelude::*;

use crate::components::PlaylistCard;
use crate::components::empty_state::{EmptyStateSize, ErrorEmptyState, NoPlaylistsEmptyState};
use crate::types::PlaylistMetadata;

/// Loading skeleton for a single playlist card.
#[component]
fn PlaylistCardSkeleton() -> impl IntoView {
    view! {
        <div class="playlist-card playlist-card-skeleton">
            <div class="playlist-icon skeleton-pulse"></div>
            <div class="playlist-info">
                <div class="skeleton-text skeleton-title"></div>
                <div class="skeleton-text skeleton-meta"></div>
            </div>
        </div>
    }
}

/// Loading state with skeleton placeholders.
#[component]
fn LoadingState(
    /// Number of skeleton items to show.
    #[prop(default = 6)]
    count: usize,
) -> impl IntoView {
    view! {
        <div class="playlist-list-loading">
            <div class="responsive-grid" style="--grid-min-width: 300px">
                {(0..count).map(|_| {
                    view! { <PlaylistCardSkeleton /> }
                }).collect_view()}
            </div>
        </div>
    }
}

/// Error state when loading fails.
#[component]
fn PlaylistListErrorState(
    /// The error message to display.
    message: String,
    /// Callback to retry loading.
    on_retry: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="playlist-list-error">
            <ErrorEmptyState
                message=message
                on_retry=on_retry
                size=EmptyStateSize::Medium
            />
        </div>
    }
}

/// Playlist count summary.
#[component]
fn PlaylistSummary(
    /// Total number of playlists.
    count: usize,
    /// Total size of all playlists in bytes.
    total_bytes: u64,
) -> impl IntoView {
    let size_str = format_bytes(total_bytes);

    view! {
        <div class="playlist-list-summary">
            <span class="playlist-count">{count} " playlist" {if count == 1 { "" } else { "s" }}</span>
            <span class="separator">"•"</span>
            <span class="total-size">{size_str} " total"</span>
        </div>
    }
}

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

/// Playlist list loading state enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaylistListState {
    /// Initial loading state.
    Loading,
    /// Data loaded successfully.
    Loaded,
    /// Error occurred while loading.
    Error,
}

/// Playlist list component that displays playlists in a grid or list format.
///
/// Features:
/// - Responsive grid layout
/// - Loading state with skeleton placeholders
/// - Empty state for new users with action button
/// - Error state with retry capability
/// - Smooth scrolling
/// - Playlist count summary
#[component]

pub fn PlaylistList(
    /// List of playlists to display.
    playlists: ReadSignal<Vec<PlaylistMetadata>>,
    /// Currently selected playlist.
    selected_playlist: ReadSignal<Option<PlaylistMetadata>>,
    /// Loading state of the list.
    #[prop(default = PlaylistListState::Loaded)]
    state: PlaylistListState,
    /// Optional error message when state is Error.
    #[prop(optional)]
    error_message: Option<String>,
    /// Callback when a playlist is selected.
    on_select: Callback<PlaylistMetadata>,
    /// Callback when delete is requested.
    on_delete: Callback<String>,
    /// Callback when sync is requested.
    on_sync: Callback<String>,
    /// Callback to retry loading (used when state is Error).
    #[prop(optional)]
    on_retry: Option<Callback<()>>,
    /// Callback when create playlist is requested (used in empty state).
    #[prop(optional)]
    on_create: Option<Callback<()>>,
    /// Minimum width for grid items (CSS value).
    #[prop(default = "300px".to_string())]
    min_item_width: String,
    /// Whether to show the playlist summary.
    #[prop(default = true)]
    show_summary: bool,
) -> impl IntoView {
    let min_width = min_item_width;
    let min_width_style = format!("--grid-min-width: {min_width}");

    view! {
        <div class="playlist-list">
            {move || match state {
                PlaylistListState::Loading => {
                    view! { <LoadingState count=6 /> }.into_any()
                }
                PlaylistListState::Error => {
                    let msg = error_message.clone().unwrap_or_else(|| "An error occurred".to_string());
                    let retry_cb = on_retry.unwrap_or_else(|| Callback::new(|()| {}));
                    view! { <PlaylistListErrorState message=msg on_retry=retry_cb /> }.into_any()
                }
                PlaylistListState::Loaded => {
                    let playlist_list = playlists.get();

                    if playlist_list.is_empty() {
                        let create_callback = on_create;
                        view! {
                            <div class="playlist-list-empty">
                                {if let Some(callback) = create_callback {
                                    view! {
                                        <NoPlaylistsEmptyState
                                            on_create=callback
                                            size=EmptyStateSize::Large
                                        />
                                    }.into_any()
                                } else {
                                    view! {
                                        <NoPlaylistsEmptyState
                                            size=EmptyStateSize::Large
                                        />
                                    }.into_any()
                                }}
                            </div>
                        }.into_any()
                    } else {
                        let total_bytes: u64 = playlist_list.iter().map(|p| p.total_bytes).sum();
                        let count = playlist_list.len();
                        let style = min_width_style.clone();

                        view! {
                            <div class="playlist-list-content">
                                {if show_summary {
                                    Some(view! { <PlaylistSummary count=count total_bytes=total_bytes /> })
                                } else {
                                    None
                                }}
                                <div class="responsive-grid playlist-grid-container" style=style>
                                    {playlist_list.into_iter().map(|playlist| {
                                        let is_selected = selected_playlist.get()
                                            .as_ref()
                                            .is_some_and(|s| s.name == playlist.name);
                                        view! {
                                            <PlaylistCard
                                                playlist=playlist
                                                on_select=on_select
                                                on_delete=on_delete
                                                on_sync=on_sync
                                                selected=is_selected
                                            />
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        }.into_any()
                    }
                }
            }}
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
