//! Transfer progress panel component for displaying file transfer status to USB devices.

use leptos::prelude::*;

use crate::types::{TaskId, TransferProgress, TransferStatus};

/// State of the transfer panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TransferPanelState {
    /// No active transfer.
    #[default]
    Idle,
    /// Transfer is preparing.
    Preparing,
    /// Transfer is in progress.
    Transferring,
    /// Transfer is verifying file integrity.
    Verifying,
    /// Transfer completed successfully.
    Completed,
    /// Transfer failed with error.
    Failed(String),
    /// Transfer was cancelled.
    Cancelled,
}

impl TransferPanelState {
    /// Check if the panel should be visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    /// Check if transfer is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Preparing | Self::Transferring | Self::Verifying)
    }

    /// Check if transfer has ended (completed, failed, or cancelled).
    #[must_use]
    pub const fn is_ended(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed(_) | Self::Cancelled)
    }

    /// Create from `TransferStatus`.
    #[must_use]
    pub fn from_status(status: TransferStatus) -> Self {
        match status {
            TransferStatus::Preparing => Self::Preparing,
            TransferStatus::Transferring => Self::Transferring,
            TransferStatus::Verifying => Self::Verifying,
            TransferStatus::Completed => Self::Completed,
            TransferStatus::Failed => Self::Failed("Transfer failed".to_string()),
            TransferStatus::Cancelled => Self::Cancelled,
        }
    }
}

/// Transfer progress panel component.
///
/// Displays a progress indicator for file transfers to USB devices including:
/// - Overall progress bar
/// - Current file being transferred
/// - Current file progress bar
/// - Transfer speed
/// - Time remaining
/// - File counts (completed/skipped/failed)
/// - Cancel button
#[component]

pub fn TransferProgressPanel(
    /// The current transfer progress (None if no active transfer).
    progress: ReadSignal<Option<TransferProgress>>,
    /// The current state of the transfer panel.
    state: ReadSignal<TransferPanelState>,
    /// Callback when cancel is clicked. Receives the task ID.
    on_cancel: Callback<TaskId>,
    /// Callback when dismiss is clicked (for completed/failed states).
    on_dismiss: Callback<()>,
    /// Reactive task ID for cancellation.
    task_id: ReadSignal<Option<TaskId>>,
) -> impl IntoView {
    let is_visible = move || state.get().is_visible();
    let is_active = move || state.get().is_active();
    let is_ended = move || state.get().is_ended();

    let panel_class = move || {
        let mut classes = vec!["transfer-progress-panel"];
        if is_visible() {
            classes.push("visible");
        }
        match state.get() {
            TransferPanelState::Completed => classes.push("completed"),
            TransferPanelState::Failed(_) => classes.push("failed"),
            TransferPanelState::Cancelled => classes.push("cancelled"),
            TransferPanelState::Verifying => classes.push("verifying"),
            _ => {}
        }
        classes.join(" ")
    };

    let status_icon = move || {
        match state.get() {
            TransferPanelState::Idle => view! {
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                    <path d="M9 16h6v-6h4l-7-7-7 7h4zm-4 2h14v2H5z"/>
                </svg>
            }.into_any(),
            TransferPanelState::Preparing => view! {
                <div class="transfer-spinner"></div>
            }.into_any(),
            TransferPanelState::Transferring => view! {
                <div class="transfer-spinner"></div>
            }.into_any(),
            TransferPanelState::Verifying => view! {
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor" class="verifying-icon">
                    <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm-2 16l-4-4 1.41-1.41L10 14.17l6.59-6.59L18 9l-8 8z"/>
                </svg>
            }.into_any(),
            TransferPanelState::Completed => view! {
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor" class="success-icon">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                </svg>
            }.into_any(),
            TransferPanelState::Failed(_) => view! {
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor" class="error-icon">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                </svg>
            }.into_any(),
            TransferPanelState::Cancelled => view! {
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor" class="warning-icon">
                    <path d="M12 2C6.47 2 2 6.47 2 12s4.47 10 10 10 10-4.47 10-10S17.53 2 12 2zm5 13.59L15.59 17 12 13.41 8.41 17 7 15.59 10.59 12 7 8.41 8.41 7 12 10.59 15.59 7 17 8.41 13.41 12 17 15.59z"/>
                </svg>
            }.into_any(),
        }
    };

    let status_title = move || match state.get() {
        TransferPanelState::Idle => "Ready".to_string(),
        TransferPanelState::Preparing => "Preparing transfer...".to_string(),
        TransferPanelState::Transferring => "Transferring files...".to_string(),
        TransferPanelState::Verifying => "Verifying integrity...".to_string(),
        TransferPanelState::Completed => "Transfer Complete".to_string(),
        TransferPanelState::Failed(ref msg) => format!("Transfer Failed: {msg}"),
        TransferPanelState::Cancelled => "Transfer Cancelled".to_string(),
    };

    let handle_cancel = move |_: web_sys::MouseEvent| {
        if let Some(id) = task_id.get() {
            on_cancel.run(id);
        }
    };

    let handle_dismiss = move |_: web_sys::MouseEvent| {
        on_dismiss.run(());
    };

    view! {
        <div class=panel_class data-testid="transfer-progress-panel">
            // Header section
            <div class="transfer-progress-header">
                <div class="transfer-status-icon" data-testid="transfer-status-icon">
                    {status_icon}
                </div>
                <div class="transfer-status-info">
                    <div class="transfer-status-title" data-testid="transfer-status-title">
                        {status_title}
                    </div>
                    {move || {
                        if let Some(p) = progress.get() {
                            if is_active() {
                                Some(view! {
                                    <div class="transfer-status-subtitle" data-testid="transfer-status-subtitle">
                                        <span class="file-counter">
                                            {format!("{} of {} files", p.current_file_index, p.total_files)}
                                        </span>
                                    </div>
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }}
                </div>
                // Action buttons
                <div class="transfer-progress-actions">
                    {move || {
                        if is_active() {
                            view! {
                                <button
                                    class="btn btn-ghost btn-icon transfer-cancel-btn"
                                    title="Cancel transfer"
                                    on:click=handle_cancel
                                    data-testid="transfer-cancel-btn"
                                >
                                    <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                        <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                                    </svg>
                                </button>
                            }.into_any()
                        } else if is_ended() {
                            view! {
                                <button
                                    class="btn btn-ghost btn-icon transfer-dismiss-btn"
                                    title="Dismiss"
                                    on:click=handle_dismiss
                                    data-testid="transfer-dismiss-btn"
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

            // Progress content (only shown when active or has progress data)
            {move || {
                if let Some(p) = progress.get() {
                    Some(view! {
                        <div class="transfer-progress-content">
                            // Current file info
                            <div class="transfer-current-file" data-testid="transfer-current-file">
                                <div class="current-file-icon">
                                    <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                                        <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                                    </svg>
                                </div>
                                <div class="current-file-name" title=p.current_file_name.clone()>
                                    {p.current_file_name.clone()}
                                </div>
                            </div>

                            // Current file progress bar
                            {move || {
                                let p = progress.get()?;
                                if p.current_file_total > 0 {
                                    Some(view! {
                                        <div class="transfer-file-progress-container">
                                            <div class="transfer-file-progress-bar" data-testid="transfer-file-progress-bar">
                                                <div
                                                    class="transfer-file-progress-fill"
                                                    style=move || format!("width: {}%", progress.get().map_or(0.0, |p| p.current_file_progress_percent()))
                                                    data-testid="transfer-file-progress-fill"
                                                ></div>
                                            </div>
                                            <div class="transfer-file-progress-label">
                                                <span class="file-bytes">
                                                    {format_bytes(p.current_file_bytes)} " / " {format_bytes(p.current_file_total)}
                                                </span>
                                            </div>
                                        </div>
                                    })
                                } else {
                                    None
                                }
                            }}

                            // Overall progress bar
                            <div class="transfer-progress-bar-container">
                                <div class="transfer-progress-label-row">
                                    <span class="progress-label">"Overall Progress"</span>
                                    <span class="transfer-progress-percent" data-testid="transfer-progress-percent">
                                        {move || format!("{:.0}%", progress.get().map_or(0.0, |p| p.overall_progress_percent()))}
                                    </span>
                                </div>
                                <div class="transfer-progress-bar" data-testid="transfer-progress-bar">
                                    <div
                                        class="transfer-progress-fill"
                                        style=move || format!("width: {}%", progress.get().map_or(0.0, |p| p.overall_progress_percent()))
                                        data-testid="transfer-progress-fill"
                                    ></div>
                                </div>
                                <div class="transfer-bytes-row">
                                    <span class="bytes-transferred" data-testid="bytes-transferred">
                                        {format_bytes(p.total_bytes_transferred)} " / " {format_bytes(p.total_bytes)}
                                    </span>
                                </div>
                            </div>

                            // Stats row
                            <div class="transfer-stats" data-testid="transfer-stats">
                                // Speed
                                <div class="transfer-stat">
                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                        <path d="M15 4v2H5.17L19 19.83V10h2v12H9v-2h9.83L5 6.17V16H3V4h12z"/>
                                    </svg>
                                    <span class="stat-value" data-testid="transfer-speed">{p.formatted_speed()}</span>
                                </div>

                                // ETA
                                {move || {
                                    progress.get().and_then(|p| {
                                        if p.estimated_remaining_secs.is_some() {
                                            Some(view! {
                                                <div class="transfer-stat">
                                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                                        <path d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z"/>
                                                    </svg>
                                                    <span class="stat-value" data-testid="transfer-eta">{p.formatted_remaining_time()}</span>
                                                </div>
                                            })
                                        } else {
                                            None
                                        }
                                    })
                                }}

                                // Elapsed
                                <div class="transfer-stat">
                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                        <path d="M13 3c-4.97 0-9 4.03-9 9H1l3.89 3.89.07.14L9 12H6c0-3.87 3.13-7 7-7s7 3.13 7 7-3.13 7-7 7c-1.93 0-3.68-.79-4.94-2.06l-1.42 1.42C8.27 19.99 10.51 21 13 21c4.97 0 9-4.03 9-9s-4.03-9-9-9zm-1 5v5l4.28 2.54.72-1.21-3.5-2.08V8H12z"/>
                                    </svg>
                                    <span class="stat-value" data-testid="transfer-elapsed">{format_elapsed(p.elapsed_secs)}</span>
                                    <span class="stat-label">"elapsed"</span>
                                </div>
                            </div>

                            // File counts
                            <div class="transfer-counts" data-testid="transfer-counts">
                                {move || {
                                    let p = progress.get()?;
                                    Some(view! {
                                        <>
                                            {(p.files_completed > 0).then(|| view! {
                                                <span class="count-badge completed" data-testid="files-completed">
                                                    <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                                        <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                                                    </svg>
                                                    {p.files_completed} " completed"
                                                </span>
                                            })}
                                            {(p.files_skipped > 0).then(|| view! {
                                                <span class="count-badge skipped" data-testid="files-skipped">
                                                    <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                                        <path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z"/>
                                                    </svg>
                                                    {p.files_skipped} " skipped"
                                                </span>
                                            })}
                                            {(p.files_failed > 0).then(|| view! {
                                                <span class="count-badge failed" data-testid="files-failed">
                                                    <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                                        <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                                                    </svg>
                                                    {p.files_failed} " failed"
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

/// Compact transfer progress indicator for use in headers or sidebars.
///
/// Shows a minimal progress indicator with essential information.
#[component]

pub fn TransferProgressIndicator(
    /// The current transfer progress (None if no active transfer).
    progress: ReadSignal<Option<TransferProgress>>,
    /// Whether a transfer is currently active.
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
            class="transfer-progress-indicator"
            class:visible=move || is_active.get()
            class:clickable=on_click.is_some()
            on:click=handle_click
            data-testid="transfer-progress-indicator"
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
                                <span class="indicator-label">"Transferring"</span>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="indicator-content">
                                <div class="indicator-spinner"></div>
                                <span class="indicator-label">"Preparing..."</span>
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

/// Format bytes as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.2} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

/// Format elapsed time as a human-readable string.
fn format_elapsed(secs: f64) -> String {
    if secs >= 3600.0 {
        let hours = (secs / 3600.0).floor();
        let mins = ((secs % 3600.0) / 60.0).floor();
        let s = (secs % 60.0).floor();
        format!("{}:{:02}:{:02}", hours as u32, mins as u32, s as u32)
    } else if secs >= 60.0 {
        let mins = (secs / 60.0).floor();
        let s = (secs % 60.0).floor();
        format!("{}:{:02}", mins as u32, s as u32)
    } else {
        format!("0:{:02}", secs as u32)
    }
}
