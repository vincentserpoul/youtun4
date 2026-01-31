//! Playlist detail view component for displaying individual playlist information.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::{TrackList, TrackListState};
use crate::tauri_api;
use crate::types::{PlaylistMetadata, TrackInfo};

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

/// Format Unix timestamp to human-readable date string.
fn format_date(timestamp: u64) -> String {
    // Simple date formatting (YYYY-MM-DD)
    // Using JavaScript Date for proper formatting in WASM
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
    let year = date.get_full_year();
    let month = date.get_month() + 1; // getMonth returns 0-11
    let day = date.get_date();
    format!("{year}-{month:02}-{day:02}")
}

/// Format duration from total seconds.
fn format_total_duration(total_secs: u64) -> String {
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;

    if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

/// Playlist detail view header with metadata.
#[component]
fn PlaylistDetailHeader(
    /// The playlist metadata.
    playlist: PlaylistMetadata,
    /// Total duration in seconds (calculated from tracks).
    total_duration_secs: ReadSignal<u64>,
    /// Callback when back button is clicked.
    on_back: Callback<()>,
    /// Callback when sync button is clicked.
    on_sync: Callback<String>,
    /// Callback when delete button is clicked.
    on_delete: Callback<String>,
) -> impl IntoView {
    let playlist_name = playlist.name.clone();
    let playlist_name_for_sync = playlist.name.clone();
    let playlist_name_for_delete = playlist.name.clone();
    let has_source_url = playlist.source_url.is_some();
    let source_url = playlist.source_url.clone();
    let created_at = format_date(playlist.created_at);
    let modified_at = format_date(playlist.modified_at);
    let total_bytes = format_bytes(playlist.total_bytes);
    let track_count = playlist.track_count;

    view! {
        <div class="playlist-detail-header">
            <div class="playlist-detail-header-top">
                <button
                    class="btn btn-ghost playlist-back-btn"
                    on:click=move |_| on_back.run(())
                    aria-label="Go back to playlist list"
                >
                    <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                        <path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"/>
                    </svg>
                    "Back"
                </button>
                <div class="playlist-detail-actions">
                    <button
                        class="btn btn-secondary"
                        on:click=move |_| on_sync.run(playlist_name_for_sync.clone())
                        title="Sync to device"
                    >
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                            <path d="M19 8l-4 4h3c0 3.31-2.69 6-6 6-1.01 0-1.97-.25-2.8-.7l-1.46 1.46C8.97 19.54 10.43 20 12 20c4.42 0 8-3.58 8-8h3l-4-4zM6 12c0-3.31 2.69-6 6-6 1.01 0 1.97.25 2.8.7l1.46-1.46C15.03 4.46 13.57 4 12 4c-4.42 0-8 3.58-8 8H1l4 4 4-4H6z"/>
                        </svg>
                        "Sync"
                    </button>
                    <button
                        class="btn btn-danger"
                        on:click=move |_| on_delete.run(playlist_name_for_delete.clone())
                        title="Delete playlist"
                    >
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                            <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
                        </svg>
                        "Delete"
                    </button>
                </div>
            </div>

            <div class="playlist-detail-info">
                <div class="playlist-detail-icon">
                    <svg viewBox="0 0 24 24" width="64" height="64" fill="currentColor">
                        <path d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/>
                    </svg>
                </div>
                <div class="playlist-detail-text">
                    <h1 class="playlist-detail-name">{playlist_name}</h1>
                    <div class="playlist-detail-stats">
                        <span class="stat-item">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                                <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                            </svg>
                            {track_count} " track" {if track_count == 1 { "" } else { "s" }}
                        </span>
                        <span class="stat-separator">"•"</span>
                        <span class="stat-item">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                                <path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-7 14H5v-2h7v2zm5-4H5v-2h12v2zm0-4H5V7h12v2z"/>
                            </svg>
                            {total_bytes}
                        </span>
                        <span class="stat-separator">"•"</span>
                        <span class="stat-item">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                                <path d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z"/>
                            </svg>
                            {move || {
                                let secs = total_duration_secs.get();
                                if secs > 0 {
                                    format_total_duration(secs)
                                } else {
                                    "--".to_string()
                                }
                            }}
                        </span>
                    </div>
                </div>
            </div>

            <div class="playlist-detail-metadata">
                {if has_source_url {
                    let url_display = source_url.unwrap_or_default();
                    let url_short = if url_display.len() > 50 {
                        format!("{}...", &url_display[..47])
                    } else {
                        url_display.clone()
                    };
                    Some(view! {
                        <div class="metadata-row source-url">
                            <span class="metadata-label">
                                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor" class="youtube-icon">
                                    <path d="M21.543 6.498C22 8.28 22 12 22 12s0 3.72-.457 5.502c-.254.985-.997 1.76-1.938 2.022C17.896 20 12 20 12 20s-5.893 0-7.605-.476c-.945-.266-1.687-1.04-1.938-2.022C2 15.72 2 12 2 12s0-3.72.457-5.502c.254-.985.997-1.76 1.938-2.022C6.107 4 12 4 12 4s5.896 0 7.605.476c.945.266 1.687 1.04 1.938 2.022zM10 15.5l6-3.5-6-3.5v7z"/>
                                </svg>
                                "Source"
                            </span>
                            <a
                                href=url_display.clone()
                                target="_blank"
                                rel="noopener noreferrer"
                                class="metadata-value source-link"
                                title=url_display
                            >
                                {url_short}
                                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" class="external-link-icon">
                                    <path d="M19 19H5V5h7V3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2v-7h-2v7zM14 3v2h3.59l-9.83 9.83 1.41 1.41L19 6.41V10h2V3h-7z"/>
                                </svg>
                            </a>
                        </div>
                    })
                } else {
                    None
                }}
                <div class="metadata-row">
                    <span class="metadata-label">
                        <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                            <path d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8z"/>
                            <path d="M12.5 7H11v6l5.25 3.15.75-1.23-4.5-2.67z"/>
                        </svg>
                        "Created"
                    </span>
                    <span class="metadata-value">{created_at}</span>
                </div>
                <div class="metadata-row">
                    <span class="metadata-label">
                        <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                            <path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34a.9959.9959 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/>
                        </svg>
                        "Modified"
                    </span>
                    <span class="metadata-value">{modified_at}</span>
                </div>
            </div>
        </div>
    }
}

/// Loading skeleton for the detail header.
#[component]
fn PlaylistDetailHeaderSkeleton() -> impl IntoView {
    view! {
        <div class="playlist-detail-header playlist-detail-header-skeleton">
            <div class="playlist-detail-header-top">
                <div class="skeleton-pulse" style="width: 80px; height: 36px; border-radius: var(--radius-md);"></div>
                <div class="playlist-detail-actions">
                    <div class="skeleton-pulse" style="width: 80px; height: 36px; border-radius: var(--radius-md);"></div>
                    <div class="skeleton-pulse" style="width: 80px; height: 36px; border-radius: var(--radius-md);"></div>
                </div>
            </div>

            <div class="playlist-detail-info">
                <div class="playlist-detail-icon skeleton-pulse"></div>
                <div class="playlist-detail-text">
                    <div class="skeleton-text" style="width: 60%; height: 2rem; margin-bottom: var(--spacing-sm);"></div>
                    <div class="skeleton-text" style="width: 40%; height: 1rem;"></div>
                </div>
            </div>

            <div class="playlist-detail-metadata">
                <div class="metadata-row">
                    <div class="skeleton-text" style="width: 80px; height: 1rem;"></div>
                    <div class="skeleton-text" style="width: 120px; height: 1rem;"></div>
                </div>
                <div class="metadata-row">
                    <div class="skeleton-text" style="width: 80px; height: 1rem;"></div>
                    <div class="skeleton-text" style="width: 100px; height: 1rem;"></div>
                </div>
            </div>
        </div>
    }
}

/// Track preview modal for showing detailed track info.
#[component]
fn TrackPreviewModal(
    /// Whether the modal is open.
    is_open: ReadSignal<bool>,
    /// The track to preview.
    track: ReadSignal<Option<TrackInfo>>,
    /// Callback when the modal is closed.
    on_close: Callback<()>,
) -> impl IntoView {
    view! {
        <div
            class="track-preview-overlay"
            class:visible=move || is_open.get()
            on:click=move |_| on_close.run(())
        >
            <div
                class="track-preview-modal"
                on:click=move |e: web_sys::MouseEvent| e.stop_propagation()
            >
                {move || {
                    if let Some(t) = track.get() {
                        let has_metadata = t.metadata.as_ref().is_some_and(super::super::types::Mp3Metadata::has_content);
                        let title = t.metadata.as_ref()
                            .and_then(|m| m.title.clone())
                            .unwrap_or_else(|| {
                                t.file_name.rsplit_once('.').map_or_else(|| t.file_name.clone(), |(name, _)| name.to_string())
                            });
                        let artist = t.metadata.as_ref().and_then(|m| m.artist.clone());
                        let album = t.metadata.as_ref().and_then(|m| m.album.clone());
                        let duration = t.metadata.as_ref().and_then(|m| m.duration_secs).map(|secs| {
                            let mins = secs / 60;
                            let secs = secs % 60;
                            format!("{mins}:{secs:02}")
                        });
                        let year = t.metadata.as_ref().and_then(|m| m.year);
                        let genre = t.metadata.as_ref().and_then(|m| m.genre.clone());
                        let bitrate = t.metadata.as_ref().and_then(|m| m.bitrate_kbps);
                        let track_num = t.metadata.as_ref().and_then(|m| m.track_number);
                        let total_tracks = t.metadata.as_ref().and_then(|m| m.total_tracks);
                        let size = format_bytes(t.size_bytes);

                        view! {
                            <div class="track-preview-header">
                                <h2 class="track-preview-title">{title}</h2>
                                <button
                                    class="btn btn-ghost btn-icon"
                                    on:click=move |_| on_close.run(())
                                    aria-label="Close preview"
                                >
                                    <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                                        <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                                    </svg>
                                </button>
                            </div>
                            <div class="track-preview-body">
                                <div class="track-preview-icon">
                                    <svg viewBox="0 0 24 24" width="48" height="48" fill="currentColor">
                                        <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                                    </svg>
                                </div>
                                <div class="track-preview-details">
                                    {artist.map(|a| view! {
                                        <div class="detail-row">
                                            <span class="detail-label">"Artist"</span>
                                            <span class="detail-value">{a}</span>
                                        </div>
                                    })}
                                    {album.map(|a| view! {
                                        <div class="detail-row">
                                            <span class="detail-label">"Album"</span>
                                            <span class="detail-value">{a}</span>
                                        </div>
                                    })}
                                    {track_num.map(|tn| view! {
                                        <div class="detail-row">
                                            <span class="detail-label">"Track"</span>
                                            <span class="detail-value">
                                                {if let Some(total) = total_tracks {
                                                    format!("{tn} / {total}")
                                                } else {
                                                    tn.to_string()
                                                }}
                                            </span>
                                        </div>
                                    })}
                                    {duration.map(|d| view! {
                                        <div class="detail-row">
                                            <span class="detail-label">"Duration"</span>
                                            <span class="detail-value">{d}</span>
                                        </div>
                                    })}
                                    {year.map(|y| view! {
                                        <div class="detail-row">
                                            <span class="detail-label">"Year"</span>
                                            <span class="detail-value">{y}</span>
                                        </div>
                                    })}
                                    {genre.map(|g| view! {
                                        <div class="detail-row">
                                            <span class="detail-label">"Genre"</span>
                                            <span class="detail-value">{g}</span>
                                        </div>
                                    })}
                                    {bitrate.map(|b| view! {
                                        <div class="detail-row">
                                            <span class="detail-label">"Bitrate"</span>
                                            <span class="detail-value">{b} " kbps"</span>
                                        </div>
                                    })}
                                    <div class="detail-row">
                                        <span class="detail-label">"File Size"</span>
                                        <span class="detail-value">{size}</span>
                                    </div>
                                    <div class="detail-row">
                                        <span class="detail-label">"Filename"</span>
                                        <span class="detail-value filename">{t.file_name}</span>
                                    </div>
                                    {if has_metadata {
                                        None
                                    } else {
                                        Some(view! {
                                            <div class="no-metadata-hint">
                                                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                                                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                                                </svg>
                                                "No ID3 metadata found in this file"
                                            </div>
                                        })
                                    }}
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="track-preview-empty">
                                "No track selected"
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

/// Playlist detail view state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaylistDetailState {
    /// Loading playlist data.
    Loading,
    /// Playlist loaded successfully.
    Loaded,
    /// Error loading playlist.
    Error,
}

/// Playlist detail view component showing all tracks and metadata.
#[component]

pub fn PlaylistDetailView(
    /// The playlist name to display.
    playlist_name: String,
    /// Callback when back button is clicked.
    on_back: Callback<()>,
    /// Callback when sync button is clicked.
    on_sync: Callback<String>,
    /// Callback when delete button is clicked.
    on_delete: Callback<String>,
    /// Refresh trigger - increment to reload tracks.
    #[prop(optional, default = 0u32.into())]
    refresh_trigger: Signal<u32>,
) -> impl IntoView {
    let (state, set_state) = signal(PlaylistDetailState::Loading);
    let (playlist, set_playlist) = signal::<Option<PlaylistMetadata>>(None);
    let (tracks, set_tracks) = signal::<Vec<TrackInfo>>(vec![]);
    let (track_list_state, set_track_list_state) = signal(TrackListState::Loading);
    let (error_message, set_error_message) = signal::<Option<String>>(None);
    let (total_duration_secs, set_total_duration_secs) = signal(0u64);

    // Track preview modal state
    let (preview_open, set_preview_open) = signal(false);
    let (preview_track, set_preview_track) = signal::<Option<TrackInfo>>(None);

    let playlist_name_clone = playlist_name;

    // Load playlist data on mount and when refresh_trigger changes
    Effect::new(move || {
        // Subscribe to refresh_trigger to re-run when it changes
        let _trigger = refresh_trigger.get();
        let name = playlist_name_clone.clone();
        spawn_local(async move {
            leptos::logging::log!("Loading playlist details for: {}", name);

            // Load playlist metadata
            match tauri_api::get_playlist_details(&name).await {
                Ok(meta) => {
                    leptos::logging::log!("Playlist metadata loaded: {} tracks", meta.track_count);
                    set_playlist.set(Some(meta));
                    set_state.set(PlaylistDetailState::Loaded);
                }
                Err(e) => {
                    leptos::logging::error!("Failed to load playlist metadata: {}", e);
                    set_error_message.set(Some(e));
                    set_state.set(PlaylistDetailState::Error);
                    return;
                }
            }

            // Load tracks with metadata
            match tauri_api::get_playlist_tracks(&name).await {
                Ok(track_list) => {
                    leptos::logging::log!("Loaded {} tracks with metadata", track_list.len());

                    // Calculate total duration
                    let total: u64 = track_list
                        .iter()
                        .filter_map(|t| t.metadata.as_ref())
                        .filter_map(|m| m.duration_secs)
                        .sum();
                    set_total_duration_secs.set(total);

                    set_tracks.set(track_list);
                    set_track_list_state.set(TrackListState::Loaded);
                }
                Err(e) => {
                    leptos::logging::error!("Failed to load tracks: {}", e);
                    set_track_list_state.set(TrackListState::Error);
                }
            }
        });
    });

    // Handle track click to show preview
    let on_track_click = Callback::new(move |track: TrackInfo| {
        set_preview_track.set(Some(track));
        set_preview_open.set(true);
    });

    let on_preview_close = Callback::new(move |()| {
        set_preview_open.set(false);
    });

    view! {
        <div class="playlist-detail-view">
            {move || match state.get() {
                PlaylistDetailState::Loading => {
                    view! {
                        <PlaylistDetailHeaderSkeleton />
                        <div class="playlist-detail-content">
                            <h3 class="tracks-section-title">"Tracks"</h3>
                            <TrackList
                                tracks=tracks
                                state=TrackListState::Loading
                            />
                        </div>
                    }.into_any()
                }
                PlaylistDetailState::Error => {
                    let msg = error_message.get().unwrap_or_else(|| "Failed to load playlist".to_string());
                    view! {
                        <div class="playlist-detail-error">
                            <button
                                class="btn btn-ghost playlist-back-btn"
                                on:click=move |_| on_back.run(())
                            >
                                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                                    <path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"/>
                                </svg>
                                "Back"
                            </button>
                            <div class="error-content">
                                <svg viewBox="0 0 24 24" width="64" height="64" fill="var(--accent-error)">
                                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                                </svg>
                                <h3>"Failed to load playlist"</h3>
                                <p class="error-message">{msg}</p>
                            </div>
                        </div>
                    }.into_any()
                }
                PlaylistDetailState::Loaded => {
                    if let Some(pl) = playlist.get() {
                        view! {
                            <PlaylistDetailHeader
                                playlist=pl
                                total_duration_secs=total_duration_secs
                                on_back=on_back
                                on_sync=on_sync
                                on_delete=on_delete
                            />
                            <div class="playlist-detail-content">
                                <h3 class="tracks-section-title">"Tracks"</h3>
                                <p class="tracks-section-hint">"Click on a track to view detailed information"</p>
                                <TrackList
                                    tracks=tracks
                                    state=track_list_state.get()
                                    on_track_click=on_track_click
                                />
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="playlist-detail-error">
                                <button
                                    class="btn btn-ghost playlist-back-btn"
                                    on:click=move |_| on_back.run(())
                                >
                                    <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                                        <path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"/>
                                    </svg>
                                    "Back"
                                </button>
                                <div class="error-content">
                                    <h3>"Playlist not found"</h3>
                                </div>
                            </div>
                        }.into_any()
                    }
                }
            }}

            <TrackPreviewModal
                is_open=preview_open
                track=preview_track
                on_close=on_preview_close
            />
        </div>
    }
}
