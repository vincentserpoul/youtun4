//! Confirmation dialog component for destructive actions.

use leptos::prelude::*;

/// Confirmation dialog component for confirming destructive actions like deletion.
#[component]

pub fn ConfirmDialog(
    /// Whether the dialog is open.
    is_open: ReadSignal<bool>,
    /// Title of the dialog.
    title: String,
    /// Message to display in the dialog.
    message: String,
    /// Text for the confirm button.
    #[prop(default = "Delete".to_string())]
    confirm_text: String,
    /// Text for the cancel button.
    #[prop(default = "Cancel".to_string())]
    cancel_text: String,
    /// Whether the confirm button should be styled as dangerous.
    #[prop(default = true)]
    is_dangerous: bool,
    /// Callback when the user confirms the action.
    on_confirm: Callback<()>,
    /// Callback when the user cancels or closes the dialog.
    on_cancel: Callback<()>,
) -> impl IntoView {
    // Clone values for the closures
    let title_clone = title;
    let message_clone = message;
    let confirm_text_clone = confirm_text;
    let cancel_text_clone = cancel_text;

    view! {
        <div
            class="confirm-dialog-overlay"
            class:visible=move || is_open.get()
            on:click=move |_| on_cancel.run(())
        >
            <div
                class="confirm-dialog"
                on:click=move |e| e.stop_propagation()
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="confirm-dialog-title"
                aria-describedby="confirm-dialog-message"
            >
                <div class="confirm-dialog-icon">
                    <svg viewBox="0 0 24 24" width="48" height="48" fill="currentColor">
                        <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                    </svg>
                </div>
                <h3 id="confirm-dialog-title" class="confirm-dialog-title">{title_clone}</h3>
                <p id="confirm-dialog-message" class="confirm-dialog-message">{message_clone}</p>
                <div class="confirm-dialog-actions">
                    <button
                        class="btn btn-secondary"
                        on:click=move |_| on_cancel.run(())
                    >
                        {cancel_text_clone}
                    </button>
                    <button
                        class=move || if is_dangerous { "btn btn-danger-solid" } else { "btn btn-primary" }
                        on:click=move |_| on_confirm.run(())
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                            <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
                        </svg>
                        {confirm_text_clone}
                    </button>
                </div>
            </div>
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
        format!("{bytes} bytes")
    }
}

/// Props for the DeletePlaylistDialog component.
#[component]

pub fn DeletePlaylistDialog(
    /// Whether the dialog is open.
    is_open: ReadSignal<bool>,
    /// Name of the playlist to delete.
    playlist_name: ReadSignal<Option<String>>,
    /// Track count of the playlist (for display).
    track_count: ReadSignal<Option<usize>>,
    /// Total size in bytes of the playlist (for display).
    #[prop(optional)]
    total_bytes: Option<ReadSignal<Option<u64>>>,
    /// Source URL of the playlist (if from YouTube).
    #[prop(optional)]
    source_url: Option<ReadSignal<Option<String>>>,
    /// Callback when the user confirms deletion.
    on_confirm: Callback<()>,
    /// Callback when the user cancels.
    on_cancel: Callback<()>,
) -> impl IntoView {
    view! {
        <div
            class="confirm-dialog-overlay"
            class:visible=move || is_open.get()
            on:click=move |_| on_cancel.run(())
        >
            <div
                class="confirm-dialog delete-playlist-dialog"
                on:click=move |e| e.stop_propagation()
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="delete-dialog-title"
                aria-describedby="delete-dialog-message"
            >
                <div class="confirm-dialog-icon danger">
                    <svg viewBox="0 0 24 24" width="48" height="48" fill="currentColor">
                        <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
                    </svg>
                </div>
                <h3 id="delete-dialog-title" class="confirm-dialog-title">
                    "Delete Playlist"
                </h3>
                <p id="delete-dialog-message" class="confirm-dialog-message">
                    {move || {
                        let name = playlist_name.get().unwrap_or_default();
                        format!(
                            "Are you sure you want to delete \"{name}\"?"
                        )
                    }}
                </p>

                // Playlist details section
                <div class="delete-dialog-details">
                    <div class="delete-dialog-detail-row">
                        <span class="detail-label">"Tracks:"</span>
                        <span class="detail-value">
                            {move || {
                                let tracks = track_count.get().unwrap_or(0);
                                format!("{} track{}", tracks, if tracks == 1 { "" } else { "s" })
                            }}
                        </span>
                    </div>
                    {move || {
                        total_bytes.map(|tb| {
                            view! {
                                <div class="delete-dialog-detail-row">
                                    <span class="detail-label">"Size:"</span>
                                    <span class="detail-value">
                                        {move || format_bytes(tb.get().unwrap_or(0))}
                                    </span>
                                </div>
                            }
                        })
                    }}
                    {move || {
                        source_url.and_then(|su| {
                            su.get().map(|url| {
                                view! {
                                    <div class="delete-dialog-detail-row source-row">
                                        <span class="detail-label">"Source:"</span>
                                        <span class="detail-value source-url" title=url>
                                            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" class="youtube-icon">
                                                <path d="M21.582 7.186c-.23-.869-.908-1.553-1.775-1.784C18.254 5 12 5 12 5s-6.254 0-7.807.402c-.867.23-1.545.915-1.775 1.784C2 8.746 2 12 2 12s0 3.254.418 4.814c.23.869.908 1.553 1.775 1.784C5.746 19 12 19 12 19s6.254 0 7.807-.402c.867-.23 1.545-.915 1.775-1.784C22 15.254 22 12 22 12s0-3.254-.418-4.814zM10 15V9l5.196 3L10 15z"/>
                                            </svg>
                                            "YouTube Playlist"
                                        </span>
                                    </div>
                                }
                            })
                        })
                    }}
                </div>

                <div class="confirm-dialog-warning">
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                        <path d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"/>
                    </svg>
                    <span>"This action cannot be undone. All tracks and files will be permanently deleted from your local storage."</span>
                </div>
                <div class="confirm-dialog-actions">
                    <button
                        class="btn btn-secondary"
                        on:click=move |_| on_cancel.run(())
                    >
                        "Cancel"
                    </button>
                    <button
                        class="btn btn-danger-solid"
                        on:click=move |_| on_confirm.run(())
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                            <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
                        </svg>
                        "Delete Playlist"
                    </button>
                </div>
            </div>
        </div>
    }
}
