//! Empty state components for displaying helpful messages when no data is available.
//!
//! This module provides reusable empty state components with consistent styling
//! and helpful action buttons to guide users.

use leptos::prelude::*;

/// Icon types for empty states.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EmptyStateIcon {
    /// Music/playlist icon (default).
    #[default]
    Music,
    /// USB device icon.
    Device,
    /// Search icon.
    Search,
    /// Error/warning icon.
    Error,
    /// Folder icon.
    Folder,
    /// Download icon.
    Download,
}

/// Size variants for empty state displays.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EmptyStateSize {
    /// Small size for inline/compact displays.
    Small,
    /// Medium size (default).
    #[default]
    Medium,
    /// Large size for full-page displays.
    Large,
}

impl EmptyStateSize {
    const fn icon_size(&self) -> (&'static str, &'static str) {
        match self {
            Self::Small => ("40", "40"),
            Self::Medium => ("64", "64"),
            Self::Large => ("80", "80"),
        }
    }

    const fn class(&self) -> &'static str {
        match self {
            Self::Small => "empty-state-small",
            Self::Medium => "empty-state-medium",
            Self::Large => "empty-state-large",
        }
    }
}

/// Renders an SVG icon based on the icon type.
#[component]
fn EmptyStateIconSvg(
    /// The type of icon to display.
    icon: EmptyStateIcon,
    /// Width of the icon.
    width: &'static str,
    /// Height of the icon.
    height: &'static str,
    /// Fill color (CSS variable or color value).
    #[prop(default = "var(--text-disabled)")]
    fill: &'static str,
) -> impl IntoView {
    match icon {
        EmptyStateIcon::Music => view! {
            <svg viewBox="0 0 24 24" width=width height=height fill=fill>
                <path d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/>
            </svg>
        }.into_any(),
        EmptyStateIcon::Device => view! {
            <svg viewBox="0 0 24 24" width=width height=height fill=fill>
                <path d="M15 7v4h1v2h-3V5h2l-3-4-3 4h2v8H8v-2.07c.7-.37 1.2-1.08 1.2-1.93 0-1.21-.99-2.2-2.2-2.2-1.21 0-2.2.99-2.2 2.2 0 .85.5 1.56 1.2 1.93V13c0 1.1.9 2 2 2h3v3.05c-.71.37-1.2 1.1-1.2 1.95 0 1.22.99 2.2 2.2 2.2 1.21 0 2.2-.98 2.2-2.2 0-.85-.49-1.58-1.2-1.95V15h3c1.1 0 2-.9 2-2v-2h1V7h-4z"/>
            </svg>
        }.into_any(),
        EmptyStateIcon::Search => view! {
            <svg viewBox="0 0 24 24" width=width height=height fill=fill>
                <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/>
            </svg>
        }.into_any(),
        EmptyStateIcon::Error => view! {
            <svg viewBox="0 0 24 24" width=width height=height fill="var(--accent-error)">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
            </svg>
        }.into_any(),
        EmptyStateIcon::Folder => view! {
            <svg viewBox="0 0 24 24" width=width height=height fill=fill>
                <path d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
            </svg>
        }.into_any(),
        EmptyStateIcon::Download => view! {
            <svg viewBox="0 0 24 24" width=width height=height fill=fill>
                <path d="M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z"/>
            </svg>
        }.into_any(),
    }
}

/// Base empty state component with customizable content.
///
/// This is the foundational component that other empty states build upon.
#[component]

pub fn EmptyState(
    /// The icon to display.
    #[prop(default = EmptyStateIcon::Music)]
    icon: EmptyStateIcon,
    /// The main title/heading.
    title: &'static str,
    /// The descriptive message.
    message: &'static str,
    /// Optional hint text (displayed in italics).
    #[prop(optional)]
    hint: Option<&'static str>,
    /// Size variant.
    #[prop(default = EmptyStateSize::Medium)]
    size: EmptyStateSize,
    /// Optional additional CSS class.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Optional children (for action buttons, etc.).
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    let (width, height) = size.icon_size();
    let size_class = size.class();
    let custom_class = class.unwrap_or("");
    let full_class = format!("empty-state {size_class} {custom_class}");

    view! {
        <div class=full_class>
            <div class="empty-state-icon">
                <EmptyStateIconSvg icon=icon width=width height=height />
            </div>
            <h3 class="empty-state-title">{title}</h3>
            <p class="empty-state-message">{message}</p>
            {hint.map(|h| view! {
                <p class="empty-state-hint">{h}</p>
            })}
            {children.map(|c| view! {
                <div class="empty-state-actions">
                    {c()}
                </div>
            })}
        </div>
    }
}

/// Empty state for when no playlists exist.
///
/// Displays a friendly message encouraging users to create their first playlist
/// with an action button to open the create dialog.
#[component]

pub fn NoPlaylistsEmptyState(
    /// Callback when "Create Playlist" button is clicked.
    #[prop(optional)]
    on_create: Option<Callback<()>>,
    /// Size variant.
    #[prop(default = EmptyStateSize::Large)]
    size: EmptyStateSize,
) -> impl IntoView {
    view! {
        <EmptyState
            icon=EmptyStateIcon::Music
            title="Start building your music library"
            message="Paste a YouTube playlist URL to download tracks as MP3 files."
            hint="Downloads are stored locally and can be synced to any MP3 player."
            size=size
            class="no-playlists-empty"
        >
            {on_create.map(|callback| view! {
                <button
                    class="btn btn-primary"
                    on:click=move |_| callback.run(())
                >
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                        <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
                    </svg>
                    "Add YouTube Playlist"
                </button>
            })}
        </EmptyState>
    }
}

/// Empty state for when no device is connected.
///
/// Displays helpful guidance for connecting an MP3 player via USB.
#[component]

pub fn NoDeviceEmptyState(
    /// Callback when "Refresh" button is clicked.
    #[prop(optional)]
    on_refresh: Option<Callback<()>>,
    /// Size variant.
    #[prop(default = EmptyStateSize::Medium)]
    size: EmptyStateSize,
) -> impl IntoView {
    view! {
        <EmptyState
            icon=EmptyStateIcon::Device
            title="Connect your MP3 player"
            message="Plug in your MP3 player or USB drive via USB to sync music."
            hint="Most USB storage devices are detected automatically."
            size=size
            class="no-device-empty"
        >
            {on_refresh.map(|callback| view! {
                <button
                    class="btn btn-secondary"
                    on:click=move |_| callback.run(())
                >
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                        <path d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/>
                    </svg>
                    "Scan for Devices"
                </button>
            })}
        </EmptyState>
    }
}

/// Empty state for when a search returns no results.
///
/// Provides suggestions for modifying the search query.
#[component]

pub fn NoSearchResultsEmptyState(
    /// The search query that returned no results.
    #[prop(optional)]
    query: Option<String>,
    /// Callback when "Clear Search" button is clicked.
    #[prop(optional)]
    on_clear: Option<Callback<()>>,
    /// Size variant.
    #[prop(default = EmptyStateSize::Medium)]
    size: EmptyStateSize,
) -> impl IntoView {
    let message = query.map_or_else(
        || "No results found".to_string(),
        |q| format!("No results found for \"{q}\""),
    );

    view! {
        <EmptyState
            icon=EmptyStateIcon::Search
            title="No results found"
            message=Box::leak(message.into_boxed_str())
            hint="Try adjusting your search terms or check for typos."
            size=size
            class="no-search-results-empty"
        >
            {on_clear.map(|callback| view! {
                <button
                    class="btn btn-secondary"
                    on:click=move |_| callback.run(())
                >
                    "Clear Search"
                </button>
            })}
        </EmptyState>
    }
}

/// Empty state for when no tracks exist in a playlist.
#[component]

pub fn NoTracksEmptyState(
    /// Size variant.
    #[prop(default = EmptyStateSize::Medium)]
    size: EmptyStateSize,
) -> impl IntoView {
    view! {
        <EmptyState
            icon=EmptyStateIcon::Music
            title="No tracks in this playlist"
            message="This playlist doesn't have any tracks yet."
            hint="Tracks are downloaded when you create a playlist from a YouTube URL."
            size=size
            class="no-tracks-empty"
        />
    }
}

/// Empty state for when a download or sync operation has nothing to process.
#[component]

pub fn NothingToSyncEmptyState(
    /// Size variant.
    #[prop(default = EmptyStateSize::Medium)]
    size: EmptyStateSize,
) -> impl IntoView {
    view! {
        <EmptyState
            icon=EmptyStateIcon::Download
            title="Nothing to sync"
            message="All tracks are already on your device."
            hint="Add new playlists or update existing ones to sync new content."
            size=size
            class="nothing-to-sync-empty"
        />
    }
}

/// Empty state for generic errors with retry capability.
#[component]

pub fn ErrorEmptyState(
    /// The error message to display.
    message: String,
    /// Callback when "Retry" button is clicked.
    #[prop(optional)]
    on_retry: Option<Callback<()>>,
    /// Size variant.
    #[prop(default = EmptyStateSize::Medium)]
    size: EmptyStateSize,
) -> impl IntoView {
    view! {
        <EmptyState
            icon=EmptyStateIcon::Error
            title="Something went wrong"
            message=Box::leak(message.into_boxed_str())
            size=size
            class="error-empty-state"
        >
            {on_retry.map(|callback| view! {
                <button
                    class="btn btn-secondary"
                    on:click=move |_| callback.run(())
                >
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                        <path d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/>
                    </svg>
                    "Retry"
                </button>
            })}
        </EmptyState>
    }
}

/// Empty state for when a folder or directory is empty.
#[component]

pub fn EmptyFolderState(
    /// The folder name or path.
    #[prop(optional)]
    folder_name: Option<String>,
    /// Size variant.
    #[prop(default = EmptyStateSize::Medium)]
    size: EmptyStateSize,
) -> impl IntoView {
    let title = folder_name.as_ref().map_or_else(
        || "Folder is empty".to_string(),
        |n| format!("\"{n}\" is empty"),
    );

    view! {
        <EmptyState
            icon=EmptyStateIcon::Folder
            title=Box::leak(title.into_boxed_str())
            message="This folder doesn't contain any files."
            size=size
            class="empty-folder-state"
        />
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_state_size_icon_size() {
        assert_eq!(EmptyStateSize::Small.icon_size(), ("40", "40"));
        assert_eq!(EmptyStateSize::Medium.icon_size(), ("64", "64"));
        assert_eq!(EmptyStateSize::Large.icon_size(), ("80", "80"));
    }

    #[test]
    fn test_empty_state_size_class() {
        assert_eq!(EmptyStateSize::Small.class(), "empty-state-small");
        assert_eq!(EmptyStateSize::Medium.class(), "empty-state-medium");
        assert_eq!(EmptyStateSize::Large.class(), "empty-state-large");
    }
}
