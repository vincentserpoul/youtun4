//! Track list component for displaying tracks with MP3 metadata.

use leptos::prelude::*;

use crate::components::empty_state::{EmptyStateSize, ErrorEmptyState, NoTracksEmptyState};
use crate::types::TrackInfo;

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

/// Format duration as MM:SS.
fn format_duration(secs: u64) -> String {
    let mins = secs / 60;
    let remaining_secs = secs % 60;
    format!("{mins}:{remaining_secs:02}")
}

/// Single track row component.
#[component]
fn TrackRow(
    /// Track number (1-based index).
    index: usize,
    /// The track information to display.
    track: TrackInfo,
    /// Callback when track is clicked.
    #[prop(optional)]
    on_click: Option<Callback<TrackInfo>>,
) -> impl IntoView {
    let track_clone = track.clone();
    let has_metadata = track
        .metadata
        .as_ref()
        .is_some_and(super::super::types::Mp3Metadata::has_content);

    // Display title: prefer metadata title, fall back to filename
    let display_title = track
        .metadata
        .as_ref()
        .and_then(|m| m.title.clone())
        .unwrap_or_else(|| {
            // Remove file extension from filename for display
            track
                .file_name
                .rsplit_once('.')
                .map_or_else(|| track.file_name.clone(), |(name, _)| name.to_string())
        });

    // Artist from metadata
    let artist = track.metadata.as_ref().and_then(|m| m.artist.clone());

    // Album from metadata
    let album = track.metadata.as_ref().and_then(|m| m.album.clone());

    // Duration from metadata
    let duration = track
        .metadata
        .as_ref()
        .and_then(|m| m.duration_secs)
        .map(format_duration);

    // Track number from metadata or index
    let track_num = track
        .metadata
        .as_ref()
        .and_then(|m| m.track_number)
        .map_or_else(|| index.to_string(), |n| n.to_string());

    view! {
        <div
            class="track-row"
            class:has-metadata=has_metadata
            on:click=move |_| {
                if let Some(callback) = on_click {
                    callback.run(track_clone.clone());
                }
            }
        >
            <div class="track-number">{track_num}</div>
            <div class="track-info">
                <div class="track-title">{display_title}</div>
                {artist.map(|a| view! {
                    <div class="track-artist">{a}</div>
                })}
            </div>
            {album.map(|a| view! {
                <div class="track-album">{a}</div>
            })}
            <div class="track-duration">{duration.unwrap_or_default()}</div>
            <div class="track-size">{format_bytes(track.size_bytes)}</div>
        </div>
    }
}

/// Track row skeleton for loading state.
#[component]
fn TrackRowSkeleton() -> impl IntoView {
    view! {
        <div class="track-row track-row-skeleton">
            <div class="track-number"><span class="skeleton-pulse"></span></div>
            <div class="track-info">
                <div class="track-title"><span class="skeleton-text skeleton-title"></span></div>
                <div class="track-artist"><span class="skeleton-text skeleton-artist"></span></div>
            </div>
            <div class="track-album"><span class="skeleton-text skeleton-album"></span></div>
            <div class="track-duration"><span class="skeleton-text skeleton-duration"></span></div>
            <div class="track-size"><span class="skeleton-text skeleton-size"></span></div>
        </div>
    }
}

/// Track list header row.
#[component]
fn TrackListHeader() -> impl IntoView {
    view! {
        <div class="track-list-header">
            <div class="track-number">"#"</div>
            <div class="track-info">"Title"</div>
            <div class="track-album">"Album"</div>
            <div class="track-duration">"Duration"</div>
            <div class="track-size">"Size"</div>
        </div>
    }
}

/// Empty state when no tracks exist.
#[component]
fn TrackListEmptyState() -> impl IntoView {
    view! {
        <div class="track-list-empty">
            <NoTracksEmptyState size=EmptyStateSize::Medium />
        </div>
    }
}

/// Loading state for track list.
#[component]
fn LoadingState(
    /// Number of skeleton rows to show.
    #[prop(default = 5)]
    count: usize,
) -> impl IntoView {
    view! {
        <div class="track-list-loading">
            <TrackListHeader />
            {(0..count).map(|_| view! { <TrackRowSkeleton /> }).collect_view()}
        </div>
    }
}

/// Track list loading state enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TrackListState {
    /// Initial loading state.
    #[default]
    Loading,
    /// Data loaded successfully.
    Loaded,
    /// Error occurred while loading.
    Error,
}

/// Track list component that displays tracks with metadata.
///
/// Features:
/// - Displays track title, artist, album, duration from ID3 metadata
/// - Falls back to filename when no metadata available
/// - Loading state with skeleton placeholders
/// - Empty state for playlists with no tracks
/// - Column headers for organization
#[component]

pub fn TrackList(
    /// List of tracks to display.
    tracks: ReadSignal<Vec<TrackInfo>>,
    /// Loading state of the list.
    #[prop(default = TrackListState::Loaded)]
    state: TrackListState,
    /// Optional error message when state is Error.
    #[prop(optional)]
    error_message: Option<String>,
    /// Callback when a track is clicked.
    #[prop(optional)]
    on_track_click: Option<Callback<TrackInfo>>,
    /// Whether to show column headers.
    #[prop(default = true)]
    show_header: bool,
) -> impl IntoView {
    view! {
        <div class="track-list">
            {move || match state {
                TrackListState::Loading => {
                    view! { <LoadingState count=5 /> }.into_any()
                }
                TrackListState::Error => {
                    let msg = error_message.clone().unwrap_or_else(|| "Failed to load tracks".to_string());
                    view! {
                        <div class="track-list-error">
                            <ErrorEmptyState
                                message=msg
                                size=EmptyStateSize::Medium
                            />
                        </div>
                    }.into_any()
                }
                TrackListState::Loaded => {
                    let track_list = tracks.get();

                    if track_list.is_empty() {
                        view! { <TrackListEmptyState /> }.into_any()
                    } else {
                        view! {
                            <div class="track-list-content">
                                {if show_header {
                                    Some(view! { <TrackListHeader /> })
                                } else {
                                    None
                                }}
                                <div class="track-list-rows">
                                    {track_list.into_iter().enumerate().map(|(i, track)| {
                                        let callback = on_track_click;
                                        if let Some(cb) = callback {
                                            view! {
                                                <TrackRow
                                                    index=i + 1
                                                    track=track
                                                    on_click=cb
                                                />
                                            }.into_any()
                                        } else {
                                            view! {
                                                <TrackRow
                                                    index=i + 1
                                                    track=track
                                                />
                                            }.into_any()
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

/// Compact track item for smaller displays.
#[component]
pub fn TrackItemCompact(
    /// The track information to display.
    track: TrackInfo,
) -> impl IntoView {
    let has_metadata = track
        .metadata
        .as_ref()
        .is_some_and(super::super::types::Mp3Metadata::has_content);

    let display_title = track
        .metadata
        .as_ref()
        .and_then(|m| m.title.clone())
        .unwrap_or_else(|| {
            track
                .file_name
                .rsplit_once('.')
                .map_or_else(|| track.file_name.clone(), |(name, _)| name.to_string())
        });

    let subtitle = track
        .metadata
        .as_ref()
        .and_then(|m| match (&m.artist, &m.album) {
            (Some(artist), Some(album)) => Some(format!("{artist} • {album}")),
            (Some(artist), None) => Some(artist.clone()),
            (None, Some(album)) => Some(album.clone()),
            (None, None) => None,
        });

    let duration = track
        .metadata
        .as_ref()
        .and_then(|m| m.duration_secs)
        .map(format_duration);

    view! {
        <div class="track-item-compact" class:has-metadata=has_metadata>
            <div class="track-icon">
                <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                    <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                </svg>
            </div>
            <div class="track-details">
                <div class="track-title">{display_title}</div>
                {subtitle.map(|s| view! {
                    <div class="track-subtitle">{s}</div>
                })}
            </div>
            <div class="track-meta">
                {duration.map(|d| view! {
                    <span class="track-duration">{d}</span>
                })}
                <span class="track-size">{format_bytes(track.size_bytes)}</span>
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

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(59), "0:59");
        assert_eq!(format_duration(60), "1:00");
        assert_eq!(format_duration(125), "2:05");
        assert_eq!(format_duration(3661), "61:01");
    }
}
