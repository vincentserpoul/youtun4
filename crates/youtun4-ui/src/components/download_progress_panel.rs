//! Download progress panel component for displaying `YouTube` download status.

use leptos::prelude::*;

use crate::types::{DownloadProgress, DownloadResult, TaskId, YouTubeErrorCategory};

/// Detailed error information for display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadErrorInfo {
    /// Error category for icon/styling.
    pub category: YouTubeErrorCategory,
    /// User-friendly error title.
    pub title: String,
    /// User-friendly error description.
    pub description: String,
    /// Technical error message (for debugging).
    pub technical_message: Option<String>,
    /// Whether the error is retryable.
    pub is_retryable: bool,
}

impl DownloadErrorInfo {
    /// Create from a `DownloadResult`.
    #[must_use]
    pub fn from_result(result: &DownloadResult) -> Self {
        let category = result
            .error_category
            .unwrap_or(YouTubeErrorCategory::Unknown);
        Self {
            category,
            title: result
                .error_title
                .clone()
                .unwrap_or_else(|| category.title().to_string()),
            description: result
                .error_description
                .clone()
                .unwrap_or_else(|| category.description().to_string()),
            technical_message: result.error_message.clone(),
            is_retryable: category.is_retryable(),
        }
    }

    /// Create from an error message string (fallback).
    #[must_use]
    pub fn from_message(message: String) -> Self {
        Self {
            category: YouTubeErrorCategory::Unknown,
            title: "Download Failed".to_string(),
            description: message.clone(),
            technical_message: Some(message),
            is_retryable: false,
        }
    }
}

impl Default for DownloadErrorInfo {
    fn default() -> Self {
        Self {
            category: YouTubeErrorCategory::Unknown,
            title: "Error".to_string(),
            description: "An unexpected error occurred.".to_string(),
            technical_message: None,
            is_retryable: false,
        }
    }
}

/// State of the download panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DownloadPanelState {
    /// No active download.
    #[default]
    Idle,
    /// Download is in progress.
    Downloading,
    /// Download completed successfully.
    Completed,
    /// Download failed with detailed error info.
    Failed(DownloadErrorInfo),
    /// Download was cancelled.
    Cancelled,
}

impl DownloadPanelState {
    /// Check if the panel should be visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    /// Check if download is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Downloading)
    }

    /// Check if download has ended (completed, failed, or cancelled).
    #[must_use]
    pub const fn is_ended(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed(_) | Self::Cancelled)
    }
}

/// Download progress panel component.
///
/// Displays a progress indicator for YouTube downloads including:
/// - Overall progress bar
/// - Current track being downloaded
/// - Download speed
/// - Time remaining
/// - Cancel button
#[component]

pub fn DownloadProgressPanel(
    /// The current download progress (None if no active download).
    progress: ReadSignal<Option<DownloadProgress>>,
    /// The current state of the download panel.
    state: ReadSignal<DownloadPanelState>,
    /// Callback when cancel is clicked. Receives the task ID.
    on_cancel: Callback<TaskId>,
    /// Callback when dismiss is clicked (for completed/failed states).
    on_dismiss: Callback<()>,
) -> impl IntoView {
    let is_visible = move || state.get().is_visible();
    let is_active = move || state.get().is_active();
    let is_ended = move || state.get().is_ended();

    let panel_class = move || {
        let mut classes = vec!["download-progress-panel"];
        if is_visible() {
            classes.push("visible");
        }
        match state.get() {
            DownloadPanelState::Completed => classes.push("completed"),
            DownloadPanelState::Failed(_) => classes.push("failed"),
            DownloadPanelState::Cancelled => classes.push("cancelled"),
            _ => {}
        }
        classes.join(" ")
    };

    let status_icon = move || {
        match state.get() {
            DownloadPanelState::Idle => view! {
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                    <path d="M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z"/>
                </svg>
            }.into_any(),
            DownloadPanelState::Downloading => view! {
                <div class="download-spinner"></div>
            }.into_any(),
            DownloadPanelState::Completed => view! {
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor" class="success-icon">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                </svg>
            }.into_any(),
            DownloadPanelState::Failed(ref err) => {
                // Use category-specific icons for different error types
                let icon_path = match err.category {
                    YouTubeErrorCategory::Network => "M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z", // warning triangle
                    YouTubeErrorCategory::YouTubeService => "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z",
                    YouTubeErrorCategory::PlaylistNotFound => "M20 5.41L18.59 4 7 15.59V9H5v10h10v-2H8.41L20 5.41z", // broken link
                    YouTubeErrorCategory::VideoUnavailable => "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.42 0-8-3.58-8-8 0-1.85.63-3.55 1.69-4.9L16.9 18.31C15.55 19.37 13.85 20 12 20zm6.31-3.1L7.1 5.69C8.45 4.63 10.15 4 12 4c4.42 0 8 3.58 8 8 0 1.85-.63 3.55-1.69 4.9z", // blocked
                    YouTubeErrorCategory::AgeRestricted => "M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z", // shield
                    YouTubeErrorCategory::GeoRestricted => "M12 2C8.13 2 5 5.13 5 9c0 5.25 7 13 7 13s7-7.75 7-13c0-3.87-3.13-7-7-7zm0 9.5c-1.38 0-2.5-1.12-2.5-2.5s1.12-2.5 2.5-2.5 2.5 1.12 2.5 2.5-1.12 2.5-2.5 2.5z", // location off
                    _ => "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z", // default error
                };
                view! {
                    <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor" class="error-icon">
                        <path d=icon_path/>
                    </svg>
                }.into_any()
            },
            DownloadPanelState::Cancelled => view! {
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor" class="warning-icon">
                    <path d="M12 2C6.47 2 2 6.47 2 12s4.47 10 10 10 10-4.47 10-10S17.53 2 12 2zm5 13.59L15.59 17 12 13.41 8.41 17 7 15.59 10.59 12 7 8.41 8.41 7 12 10.59 15.59 7 17 8.41 13.41 12 17 15.59z"/>
                </svg>
            }.into_any(),
        }
    };

    let status_title = move || match state.get() {
        DownloadPanelState::Idle => "Ready".to_string(),
        DownloadPanelState::Downloading => "Downloading...".to_string(),
        DownloadPanelState::Completed => "Download Complete".to_string(),
        DownloadPanelState::Failed(ref err) => err.title.clone(),
        DownloadPanelState::Cancelled => "Download Cancelled".to_string(),
    };

    // Get error description for failed state
    let error_description = move || {
        if let DownloadPanelState::Failed(ref err) = state.get() {
            Some(err.description.clone())
        } else {
            None
        }
    };

    // Check if error is retryable
    let is_retryable = move || {
        if let DownloadPanelState::Failed(ref err) = state.get() {
            err.is_retryable
        } else {
            false
        }
    };

    let handle_cancel = move |_: web_sys::MouseEvent| {
        if let Some(p) = progress.get() {
            on_cancel.run(p.task_id);
        }
    };

    let handle_dismiss = move |_: web_sys::MouseEvent| {
        on_dismiss.run(());
    };

    view! {
        <div class=panel_class data-testid="download-progress-panel">
            // Header section
            <div class="download-progress-header">
                <div class="download-status-icon" data-testid="download-status-icon">
                    {status_icon}
                </div>
                <div class="download-status-info">
                    <div class="download-status-title" data-testid="download-status-title">
                        {status_title}
                    </div>
                    // Show error description for failed state OR progress subtitle
                    {move || {
                        if let Some(desc) = error_description() {
                            view! {
                                <div class="download-error-description" data-testid="download-error-description">
                                    {desc}
                                </div>
                            }.into_any()
                        } else if let Some(p) = progress.get() {
                            if is_active() {
                                view! {
                                    <div class="download-status-subtitle" data-testid="download-status-subtitle">
                                        {format!("{} of {} videos", p.current_index, p.total_videos)}
                                    </div>
                                }.into_any()
                            } else {
                                view! { <span></span> }.into_any()
                            }
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
                    // Show retry hint for retryable errors
                    {move || {
                        if is_retryable() {
                            view! {
                                <div class="download-retry-hint" data-testid="download-retry-hint">
                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                        <path d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/>
                                    </svg>
                                    "You can try again"
                                </div>
                            }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
                </div>
                // Action buttons
                <div class="download-progress-actions">
                    {move || {
                        if is_active() {
                            view! {
                                <button
                                    class="btn btn-ghost btn-icon download-cancel-btn"
                                    title="Cancel download"
                                    on:click=handle_cancel
                                    data-testid="download-cancel-btn"
                                >
                                    <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                        <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                                    </svg>
                                </button>
                            }.into_any()
                        } else if is_ended() {
                            view! {
                                <button
                                    class="btn btn-ghost btn-icon download-dismiss-btn"
                                    title="Dismiss"
                                    on:click=handle_dismiss
                                    data-testid="download-dismiss-btn"
                                >
                                    <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                        <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                                    </svg>
                                </button>
                            }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
                </div>
            </div>

            // Progress content (only shown when downloading)
            {move || {
                if let Some(p) = progress.get() {
                    Some(view! {
                        <div class="download-progress-content">
                            // Current track info
                            <div class="download-current-track" data-testid="download-current-track">
                                <div class="current-track-icon">
                                    <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                                        <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                                    </svg>
                                </div>
                                <div class="current-track-title" title=p.current_title.clone()>
                                    {p.current_title.clone()}
                                </div>
                            </div>

                            // Overall progress bar
                            <div class="download-progress-bar-container">
                                <div class="download-progress-bar" data-testid="download-progress-bar">
                                    <div
                                        class="download-progress-fill"
                                        style=move || format!("width: {}%", progress.get().map_or(0.0, |p| p.overall_progress_percent()))
                                        data-testid="download-progress-fill"
                                    ></div>
                                </div>
                                <div class="download-progress-percent" data-testid="download-progress-percent">
                                    {format!("{:.0}%", p.overall_progress_percent())}
                                </div>
                            </div>

                            // Stats row
                            <div class="download-stats" data-testid="download-stats">
                                // Speed
                                <div class="download-stat">
                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                        <path d="M15 4v2H5.17L19 19.83V10h2v12H9v-2h9.83L5 6.17V16H3V4h12z"/>
                                    </svg>
                                    <span class="stat-value" data-testid="download-speed">{p.formatted_speed.clone()}</span>
                                </div>

                                // ETA
                                {move || {
                                    progress.get().and_then(|p| p.formatted_eta).map(|eta| {
                                        view! {
                                            <div class="download-stat">
                                                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                                    <path d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z"/>
                                                </svg>
                                                <span class="stat-value" data-testid="download-eta">{eta}</span>
                                                <span class="stat-label">"remaining"</span>
                                            </div>
                                        }
                                    })
                                }}

                                // Elapsed
                                <div class="download-stat">
                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                        <path d="M13 3c-4.97 0-9 4.03-9 9H1l3.89 3.89.07.14L9 12H6c0-3.87 3.13-7 7-7s7 3.13 7 7-3.13 7-7 7c-1.93 0-3.68-.79-4.94-2.06l-1.42 1.42C8.27 19.99 10.51 21 13 21c4.97 0 9-4.03 9-9s-4.03-9-9-9zm-1 5v5l4.28 2.54.72-1.21-3.5-2.08V8H12z"/>
                                    </svg>
                                    <span class="stat-value" data-testid="download-elapsed">{p.formatted_elapsed}</span>
                                    <span class="stat-label">"elapsed"</span>
                                </div>
                            </div>

                            // Video counts
                            <div class="download-counts" data-testid="download-counts">
                                {move || {
                                    let p = progress.get()?;
                                    Some(view! {
                                        <>
                                            {(p.videos_completed > 0).then(|| view! {
                                                <span class="count-badge completed" data-testid="videos-completed">
                                                    <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                                        <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                                                    </svg>
                                                    {p.videos_completed}
                                                </span>
                                            })}
                                            {(p.videos_skipped > 0).then(|| view! {
                                                <span class="count-badge skipped" data-testid="videos-skipped">
                                                    <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                                        <path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z"/>
                                                    </svg>
                                                    {p.videos_skipped}
                                                </span>
                                            })}
                                            {(p.videos_failed > 0).then(|| view! {
                                                <span class="count-badge failed" data-testid="videos-failed">
                                                    <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                                        <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                                                    </svg>
                                                    {p.videos_failed}
                                                </span>
                                            })}
                                        </>
                                    })
                                }}
                            </div>
                        </div>
                    })
                } else {
                    None
                }
            }}
        </div>
    }
}

/// Compact download progress indicator for use in headers or sidebars.
///
/// Shows a minimal progress indicator with essential information.
#[component]

pub fn DownloadProgressIndicator(
    /// The current download progress (None if no active download).
    progress: ReadSignal<Option<DownloadProgress>>,
    /// Whether a download is currently active.
    is_active: ReadSignal<bool>,
    /// Optional callback when clicked.
    #[prop(optional)]
    on_click: Option<Callback<()>>,
) -> impl IntoView {
    let handle_click = move |_: web_sys::MouseEvent| {
        if let Some(callback) = on_click {
            callback.run(());
        }
    };

    view! {
        <div
            class="download-progress-indicator"
            class:visible=move || is_active.get()
            class:clickable=on_click.is_some()
            on:click=handle_click
            data-testid="download-progress-indicator"
        >
            {move || {
                if is_active.get() {
                    if let Some(p) = progress.get() {
                        view! {
                            <div class="indicator-content">
                                <div class="indicator-spinner"></div>
                                <span class="indicator-progress" data-testid="indicator-progress">
                                    {format!("{:.0}%", p.overall_progress_percent())}
                                </span>
                                <span class="indicator-label">"Downloading"</span>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="indicator-content">
                                <div class="indicator-spinner"></div>
                                <span class="indicator-label">"Starting..."</span>
                            </div>
                        }.into_any()
                    }
                } else {
                    view! { <span></span> }.into_any()
                }
            }}
        </div>
    }
}
