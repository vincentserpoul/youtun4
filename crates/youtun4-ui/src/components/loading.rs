//! Loading state components for consistent loading indicators across the app.
//!
//! This module provides reusable loading components:
//! - `Spinner` - A rotating loading indicator
//! - `LoadingOverlay` - A full-container loading overlay
//! - `ContentLoader` - A wrapper for content with loading/error states
//! - `Skeleton` - Base skeleton placeholder component
//! - `SkeletonText` - Text placeholder
//! - `SkeletonBlock` - Block/rectangle placeholder

use leptos::prelude::*;

/// Loading state enum for components that need loading/loaded/error states.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LoadingState {
    /// Initial loading state.
    Loading,
    /// Data loaded successfully.
    #[default]
    Loaded,
    /// Error occurred while loading.
    Error,
}

/// A spinning loading indicator.
///
/// Customizable size via the `size` prop (in pixels).
#[component]

pub fn Spinner(
    /// Size of the spinner in pixels.
    #[prop(default = 16)]
    size: u32,
    /// Optional additional CSS class.
    #[prop(optional, into)]
    class: Option<String>,
) -> impl IntoView {
    let class_str = format!("spinner {}", class.unwrap_or_default());
    let style = format!("width: {size}px; height: {size}px;");

    view! {
        <div class=class_str style=style></div>
    }
}

/// A loading indicator with optional text.
///
/// Displays a spinner with an optional label below it.
#[component]

pub fn LoadingIndicator(
    /// Optional label to display below the spinner.
    #[prop(optional, into)]
    label: Option<String>,
    /// Size of the spinner in pixels.
    #[prop(default = 24)]
    size: u32,
) -> impl IntoView {
    view! {
        <div class="loading-indicator">
            <Spinner size=size />
            {label.map(|text| {
                view! {
                    <span class="loading-indicator-label">{text}</span>
                }
            })}
        </div>
    }
}

/// A full-container loading overlay.
///
/// Covers the parent container with a semi-transparent overlay
/// and centered loading indicator.
#[component]

pub fn LoadingOverlay(
    /// Whether the overlay is visible.
    #[prop(default = true)]
    visible: bool,
    /// Optional label to display.
    #[prop(optional, into)]
    label: Option<String>,
) -> impl IntoView {
    let class = if visible {
        "loading-overlay visible"
    } else {
        "loading-overlay"
    };

    view! {
        <div class=class>
            {if let Some(text) = label {
                view! { <LoadingIndicator label=text size=32 /> }.into_any()
            } else {
                view! { <LoadingIndicator size=32 /> }.into_any()
            }}
        </div>
    }
}

/// Base skeleton placeholder component.
///
/// Provides the animated shimmer effect for loading placeholders.
#[component]

pub fn Skeleton(
    /// Optional width (CSS value).
    #[prop(optional, into)]
    width: Option<String>,
    /// Optional height (CSS value).
    #[prop(optional, into)]
    height: Option<String>,
    /// Optional border radius (CSS value).
    #[prop(optional, into)]
    radius: Option<String>,
    /// Optional additional CSS class.
    #[prop(optional, into)]
    class: Option<String>,
) -> impl IntoView {
    let class_str = format!("skeleton-pulse {}", class.unwrap_or_default());

    let style = format!(
        "{}{}{}",
        width.map(|w| format!("width: {w};")).unwrap_or_default(),
        height.map(|h| format!("height: {h};")).unwrap_or_default(),
        radius
            .map(|r| format!("border-radius: {r};"))
            .unwrap_or_default(),
    );

    view! {
        <div class=class_str style=style></div>
    }
}

/// Skeleton text placeholder.
///
/// A text-line shaped skeleton with customizable width.
#[component]

pub fn SkeletonText(
    /// Width of the text line (CSS value, e.g., "70%", "200px").
    #[prop(default = "100%".to_string())]
    width: String,
    /// Height of the text line (CSS value).
    #[prop(default = "1em".to_string())]
    height: String,
) -> impl IntoView {
    view! {
        <div
            class="skeleton-text skeleton-pulse"
            style=format!("width: {width}; height: {height};")
        ></div>
    }
}

/// Skeleton block placeholder.
///
/// A rectangular block skeleton for images, cards, etc.
#[component]

pub fn SkeletonBlock(
    /// Width of the block (CSS value).
    #[prop(default = "100%".to_string())]
    width: String,
    /// Height of the block (CSS value).
    #[prop(default = "100px".to_string())]
    height: String,
    /// Border radius (CSS value).
    #[prop(default = "var(--radius-md)".to_string())]
    radius: String,
) -> impl IntoView {
    view! {
        <div
            class="skeleton-block skeleton-pulse"
            style=format!("width: {width}; height: {height}; border-radius: {radius};")
        ></div>
    }
}

/// Skeleton for a list item.
///
/// Pre-built skeleton for common list item patterns (icon + text).
#[component]

pub fn SkeletonListItem(
    /// Whether to show an icon placeholder.
    #[prop(default = true)]
    show_icon: bool,
    /// Number of text lines to show.
    #[prop(default = 2)]
    lines: usize,
) -> impl IntoView {
    view! {
        <div class="skeleton-list-item">
            {if show_icon {
                Some(view! {
                    <Skeleton
                        width="40px".to_string()
                        height="40px".to_string()
                        radius="var(--radius-md)".to_string()
                    />
                })
            } else {
                None
            }}
            <div class="skeleton-list-item-content">
                {(0..lines).map(|i| {
                    let width = if i == 0 { "70%" } else { "50%" };
                    view! {
                        <SkeletonText width=width.to_string() />
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

/// Content wrapper with loading state transitions.
///
/// Provides smooth fade transitions between loading and loaded states.
#[component]
pub fn ContentLoader<F, IV>(
    /// The loading state.
    state: LoadingState,
    /// The content to display when loaded.
    children: F,
    /// Skeleton count for loading state.
    #[prop(default = 3)]
    skeleton_count: usize,
    /// Skeleton type: "list" or "card".
    #[prop(default = "list".to_string())]
    skeleton_type: String,
) -> impl IntoView
where
    F: Fn() -> IV + 'static,
    IV: IntoView + 'static,
{
    let is_loading = state == LoadingState::Loading;
    let is_error = state == LoadingState::Error;
    let is_loaded = state == LoadingState::Loaded;

    view! {
        <div class="content-loader">
            // Loading state skeleton
            <div
                class="content-loader-skeleton"
                class:visible=is_loading
            >
                {(0..skeleton_count).map(|_| {
                    if skeleton_type == "card" {
                        view! {
                            <div class="skeleton-card">
                                <SkeletonBlock height="120px".to_string() />
                                <div class="skeleton-card-body">
                                    <SkeletonText width="80%".to_string() />
                                    <SkeletonText width="60%".to_string() />
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <SkeletonListItem />
                        }.into_any()
                    }
                }).collect_view()}
            </div>

            // Error state
            <div
                class="content-loader-error"
                class:visible=is_error
            >
                <div class="content-loader-error-icon">
                    <svg viewBox="0 0 24 24" width="48" height="48" fill="var(--accent-error)">
                        <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                    </svg>
                </div>
                <h3 class="content-loader-error-title">"Something went wrong"</h3>
                <p class="content-loader-error-message">"Please try again"</p>
            </div>

            // Loaded content
            <div
                class="content-loader-content"
                class:visible=is_loaded
            >
                {children()}
            </div>
        </div>
    }
}

/// Inline loading state with minimal height.
///
/// Useful for inline elements that need a loading state without
/// taking up vertical space.
#[component]

pub fn InlineLoader(
    /// Whether the loader is active.
    #[prop(default = true)]
    active: bool,
    /// Optional label.
    #[prop(optional, into)]
    label: Option<String>,
) -> impl IntoView {
    let class = if active {
        "inline-loader visible"
    } else {
        "inline-loader"
    };

    view! {
        <span class=class>
            <Spinner size=14 />
            {label.map(|text| {
                view! {
                    <span class="inline-loader-label">{text}</span>
                }
            })}
        </span>
    }
}

/// Button loading state wrapper.
///
/// Wraps button content with a loading spinner overlay.
#[component]
pub fn ButtonLoader<F, IV>(
    /// Whether the button is loading.
    loading: bool,
    /// Button content.
    children: F,
) -> impl IntoView
where
    F: Fn() -> IV + 'static,
    IV: IntoView + 'static,
{
    view! {
        <span class="button-loader">
            <span class="button-loader-content" class:loading=loading>
                {children()}
            </span>
            {if loading {
                Some(view! {
                    <span class="button-loader-spinner">
                        <Spinner size=16 />
                    </span>
                })
            } else {
                None
            }}
        </span>
    }
}
