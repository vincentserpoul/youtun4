//! Playlist selection components for selecting a single playlist to sync.
//!
//! This module provides a radio-button style selection interface for choosing
//! a playlist to sync to a device.

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

/// Radio button indicator component.
#[component]
fn RadioIndicator(
    /// Whether this radio button is selected.
    selected: bool,
) -> impl IntoView {
    view! {
        <div class=move || if selected { "radio-indicator selected" } else { "radio-indicator" }>
            {if selected {
                Some(view! {
                    <div class="radio-indicator-inner"></div>
                })
            } else {
                None
            }}
        </div>
    }
}

/// A playlist selection card with radio button indicator.
///
/// Displays playlist info with a visual radio button to indicate selection state.
/// Clicking anywhere on the card selects it.
#[component]

pub fn PlaylistSelectionCard(
    /// The playlist metadata to display.
    playlist: PlaylistMetadata,
    /// Callback when playlist is selected.
    on_select: Callback<PlaylistMetadata>,
    /// Whether this playlist is currently selected.
    #[prop(default = false)]
    selected: bool,
) -> impl IntoView {
    let playlist_clone = playlist.clone();
    let playlist_name = playlist.name.clone();

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
            class=move || if selected { "playlist-selection-card selected" } else { "playlist-selection-card" }
            on:click=move |_| {
                on_select.run(playlist_clone.clone());
            }
            role="radio"
            aria-checked=move || selected.to_string()
            tabindex="0"
            on:keydown=move |e| {
                let key = e.key();
                if key == "Enter" || key == " " {
                    e.prevent_default();
                    on_select.run(playlist.clone());
                }
            }
        >
            <RadioIndicator selected=selected />
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
        </div>
    }
}

/// Loading skeleton for a playlist selection card.
#[component]
fn PlaylistSelectionCardSkeleton() -> impl IntoView {
    view! {
        <div class="playlist-selection-card playlist-selection-card-skeleton">
            <div class="radio-indicator skeleton-pulse"></div>
            <div class="playlist-icon skeleton-pulse"></div>
            <div class="playlist-info">
                <div class="skeleton-text skeleton-title"></div>
                <div class="skeleton-text skeleton-meta"></div>
            </div>
        </div>
    }
}

/// Loading state with skeleton placeholders for selection list.
#[component]
fn SelectionLoadingState(
    /// Number of skeleton items to show.
    #[prop(default = 4)]
    count: usize,
) -> impl IntoView {
    view! {
        <div class="playlist-selection-loading">
            {(0..count).map(|_| {
                view! { <PlaylistSelectionCardSkeleton /> }
            }).collect_view()}
        </div>
    }
}

/// Empty state when no playlists are available for selection.
#[component]
fn SelectionEmptyState() -> impl IntoView {
    view! {
        <div class="empty-state playlist-selection-empty">
            <svg viewBox="0 0 24 24" width="48" height="48" fill="var(--text-disabled)">
                <path d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/>
            </svg>
            <h3>"No playlists available"</h3>
            <p>"Create a playlist first to select it for syncing"</p>
        </div>
    }
}

/// State of the playlist selection list.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PlaylistSelectionState {
    /// Loading playlists.
    Loading,
    /// Playlists loaded successfully.
    #[default]
    Loaded,
    /// Error loading playlists.
    Error,
}

/// A list of playlists for single selection with radio button interface.
///
/// Features:
/// - Radio button selection (single playlist only)
/// - Clear visual feedback on selected item
/// - Loading state with skeleton placeholders
/// - Empty state for when no playlists exist
/// - Keyboard navigation support
#[component]

pub fn PlaylistSelectionList(
    /// List of playlists to display.
    playlists: ReadSignal<Vec<PlaylistMetadata>>,
    /// Currently selected playlist.
    selected_playlist: ReadSignal<Option<PlaylistMetadata>>,
    /// Callback when a playlist is selected.
    on_select: Callback<PlaylistMetadata>,
    /// Loading state of the list.
    #[prop(default = PlaylistSelectionState::Loaded)]
    state: PlaylistSelectionState,
    /// Optional title for the selection list.
    #[prop(optional, into)]
    title: Option<String>,
    /// Optional description text.
    #[prop(optional, into)]
    description: Option<String>,
) -> impl IntoView {
    view! {
        <div class="playlist-selection-list" role="radiogroup" aria-label="Select a playlist">
            {title.map(|t| {
                view! {
                    <div class="playlist-selection-header">
                        <h3 class="playlist-selection-title">{t}</h3>
                        {description.clone().map(|d| {
                            view! {
                                <p class="playlist-selection-description">{d}</p>
                            }
                        })}
                    </div>
                }
            })}
            <div class="playlist-selection-items">
                {move || match state {
                    PlaylistSelectionState::Loading => {
                        view! { <SelectionLoadingState count=4 /> }.into_any()
                    }
                    PlaylistSelectionState::Error => {
                        view! {
                            <div class="empty-state playlist-selection-error">
                                <svg viewBox="0 0 24 24" width="48" height="48" fill="var(--accent-error)">
                                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                                </svg>
                                <h3>"Failed to load playlists"</h3>
                                <p>"Please try again"</p>
                            </div>
                        }.into_any()
                    }
                    PlaylistSelectionState::Loaded => {
                        let playlist_list = playlists.get();

                        if playlist_list.is_empty() {
                            view! { <SelectionEmptyState /> }.into_any()
                        } else {
                            view! {
                                <div class="playlist-selection-grid">
                                    {playlist_list.into_iter().map(|playlist| {
                                        let is_selected = selected_playlist.get()
                                            .as_ref()
                                            .is_some_and(|s| s.name == playlist.name);
                                        view! {
                                            <PlaylistSelectionCard
                                                playlist=playlist
                                                on_select=on_select
                                                selected=is_selected
                                            />
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}

/// Summary bar showing the currently selected playlist.
#[component]

pub fn PlaylistSelectionSummary(
    /// Currently selected playlist.
    selected: ReadSignal<Option<PlaylistMetadata>>,
) -> impl IntoView {
    view! {
        <div class="playlist-selection-summary">
            {move || {
                match selected.get() {
                    Some(playlist) => {
                        view! {
                            <div class="playlist-selection-summary-content">
                                <span class="label">"Selected:"</span>
                                <span class="playlist-name">{playlist.name}</span>
                                <span class="separator">"•"</span>
                                <span class="track-count">{playlist.track_count} " tracks"</span>
                                <span class="separator">"•"</span>
                                <span class="size">{format_bytes(playlist.total_bytes)}</span>
                            </div>
                        }.into_any()
                    }
                    None => {
                        view! {
                            <div class="playlist-selection-summary-content empty">
                                <span class="hint">"Select a playlist to sync"</span>
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
