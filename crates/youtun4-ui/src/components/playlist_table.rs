//! Playlist table component for displaying playlists in a dashboard table format.
//!
//! Displays playlists as rows in a table with columns:
//! Title, Track Count, Duration, Status, Actions.

use leptos::prelude::*;

use crate::components::empty_state::{EmptyStateSize, ErrorEmptyState, NoPlaylistsEmptyState};
use crate::components::playlist_list::PlaylistListState;
use crate::types::{DeviceInfo, PlaylistMetadata};

/// Format duration from track count (estimate ~3.5 min per track) as mm:ss.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "truncation/precision loss acceptable for display formatting"
)]
#[allow(
    clippy::float_arithmetic,
    reason = "float arithmetic needed for duration estimate"
)]
fn estimate_duration(track_count: usize) -> String {
    let total_mins = (track_count as f64 * 3.5) as u64;
    let secs = ((track_count as f64 * 3.5 - total_mins as f64) * 60.0) as u64;
    format!("{total_mins}:{secs:02}")
}

/// A single row in the playlist table.
#[component]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Leptos component prop requires owned value"
)]
fn PlaylistTableRow(
    /// The playlist metadata to display.
    playlist: PlaylistMetadata,
    /// Callback when playlist is selected.
    on_select: Callback<PlaylistMetadata>,
    /// Callback when delete is requested.
    on_delete: Callback<String>,
    /// Callback when sync is requested.
    on_sync: Callback<String>,
    /// Whether a device is connected (for enabling sync).
    has_device: Signal<bool>,
    /// Simulated sync percentage for visual display.
    #[prop(default = 100)]
    sync_percent: u8,
) -> impl IntoView {
    let playlist_for_click = playlist.clone();
    let playlist_for_view = playlist.clone();
    let playlist_name = playlist.name.clone();
    let name_for_sync = playlist.name.clone();
    let name_for_delete = playlist.name.clone();
    let track_count = playlist.track_count;
    let duration = estimate_duration(track_count);
    let is_synced = sync_percent >= 100;

    let status_text = if is_synced { "SYNCED" } else { "PENDING" };
    let status_class = if is_synced {
        "status-badge synced"
    } else {
        "status-badge pending"
    };

    view! {
        <tr class="playlist-table-row" on:click=move |_| on_select.run(playlist_for_click.clone())>
            <td class="cell-title">
                <span class="playlist-table-name">{playlist_name}</span>
            </td>
            <td class="cell-tracks">{track_count}</td>
            <td class="cell-duration">{duration}</td>
            <td class="cell-status">
                <span class=status_class>{status_text}</span>
                <div class="row-progress-bar">
                    <div
                        class="row-progress-fill"
                        style=format!("width: {}%", sync_percent)
                    ></div>
                    <span class="row-progress-text">{format!("{sync_percent}%")}</span>
                </div>
            </td>
            <td class="cell-actions">
                // Play/View button
                <button
                    class="btn btn-icon btn-table-action"
                    title="View details"
                    on:click=move |e| {
                        e.stop_propagation();
                        on_select.run(playlist_for_view.clone());
                    }
                >
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                        <path d="M8 5v14l11-7z"/>
                    </svg>
                </button>
                // Sync button
                <button
                    class="btn btn-icon btn-table-action"
                    title="Sync to device"
                    disabled=move || !has_device.get()
                    on:click=move |e| {
                        e.stop_propagation();
                        on_sync.run(name_for_sync.clone());
                    }
                >
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                        <path d="M19 8l-4 4h3c0 3.31-2.69 6-6 6-1.01 0-1.97-.25-2.8-.7l-1.46 1.46C8.97 19.54 10.43 20 12 20c4.42 0 8-3.58 8-8h3l-4-4zM6 12c0-3.31 2.69-6 6-6 1.01 0 1.97.25 2.8.7l1.46-1.46C15.03 4.46 13.57 4 12 4c-4.42 0-8 3.58-8 8H1l4 4 4-4H6z"/>
                    </svg>
                </button>
                // Delete button
                <button
                    class="btn btn-icon btn-table-action btn-table-danger"
                    title="Delete playlist"
                    on:click=move |e| {
                        e.stop_propagation();
                        on_delete.run(name_for_delete.clone());
                    }
                >
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                        <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
                    </svg>
                </button>
            </td>
        </tr>
    }
}

/// Loading skeleton for the playlist table.
#[component]
fn PlaylistTableSkeleton() -> impl IntoView {
    view! {
        <div class="playlist-table-loading">
            {(0..4).map(|_| {
                view! {
                    <div class="skeleton-table-row">
                        <div class="skeleton-cell" style="width: 35%"><div class="skeleton-pulse"></div></div>
                        <div class="skeleton-cell" style="width: 12%"><div class="skeleton-pulse"></div></div>
                        <div class="skeleton-cell" style="width: 18%"><div class="skeleton-pulse"></div></div>
                        <div class="skeleton-cell" style="width: 20%"><div class="skeleton-pulse"></div></div>
                        <div class="skeleton-cell" style="width: 15%"><div class="skeleton-pulse"></div></div>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

/// Playlist table component for the dashboard view.
///
/// Displays playlists in a table format with sortable columns:
/// Title, Track Count, Duration, Size, Status, Actions.
#[component]

pub fn PlaylistTable(
    /// List of playlists to display.
    playlists: ReadSignal<Vec<PlaylistMetadata>>,
    /// Currently selected device (for sync button state).
    selected_device: ReadSignal<Option<DeviceInfo>>,
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
) -> impl IntoView {
    let has_device = Signal::derive(move || selected_device.get().is_some());

    view! {
        <div class="playlist-table-container">
            <div class="playlist-table-header">
                <h3 class="playlist-table-title">"MY PLAYLISTS"</h3>
                <div class="playlist-table-actions">
                    {on_create.map(|cb| view! {
                        <button
                            class="btn btn-secondary btn-add-playlist"
                            on:click=move |_| cb.run(())
                            title="Add new playlist"
                        >
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                                <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
                            </svg>
                            "NEW"
                        </button>
                    })}
                    {move || {
                        let count = playlists.get().len();
                        view! {
                            <span class="playlist-count-badge">
                                "PLAYLISTS (" {count} ")"
                            </span>
                        }
                    }}
                </div>
            </div>

            {match state {
                PlaylistListState::Loading => {
                    view! { <PlaylistTableSkeleton /> }.into_any()
                }
                PlaylistListState::Error => {
                    let msg = error_message.unwrap_or_else(|| "An error occurred".to_string());
                    let retry_cb = on_retry.unwrap_or_else(|| Callback::new(|()| {}));
                    view! {
                        <div class="playlist-table-error">
                            <ErrorEmptyState
                                message=msg
                                on_retry=retry_cb
                                size=EmptyStateSize::Medium
                            />
                        </div>
                    }.into_any()
                }
                PlaylistListState::Loaded => {
                    view! {
                        {move || {
                            let playlist_list = playlists.get();
                            if playlist_list.is_empty() {
                                let create_callback = on_create;
                                view! {
                                    <div class="playlist-table-empty">
                                        {if let Some(callback) = create_callback {
                                            view! {
                                                <NoPlaylistsEmptyState
                                                    on_create=callback
                                                    size=EmptyStateSize::Medium
                                                />
                                            }.into_any()
                                        } else {
                                            view! {
                                                <NoPlaylistsEmptyState
                                                    size=EmptyStateSize::Medium
                                                />
                                            }.into_any()
                                        }}
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="playlist-table-scroll">
                                        <table class="playlist-table">
                                            <thead>
                                                <tr>
                                                    <th class="th-title">"TITLE"</th>
                                                    <th class="th-tracks">"TRACK COUNT"</th>
                                                    <th class="th-duration">"DURATION"</th>
                                                    <th class="th-status">"STATUS"</th>
                                                    <th class="th-actions">"ACTIONS"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {playlist_list.into_iter().map(|playlist| {
                                                    view! {
                                                        <PlaylistTableRow
                                                            playlist=playlist
                                                            on_select=on_select
                                                            on_delete=on_delete
                                                            on_sync=on_sync
                                                            has_device=has_device
                                                        />
                                                    }
                                                }).collect_view()}
                                            </tbody>
                                        </table>
                                    </div>
                                }.into_any()
                            }
                        }}
                    }.into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_duration() {
        assert_eq!(estimate_duration(0), "0:00");
        assert_eq!(estimate_duration(12), "42:00");
        assert_eq!(estimate_duration(20), "70:00");
    }
}
