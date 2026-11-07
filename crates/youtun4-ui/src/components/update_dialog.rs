//! Dialog offering to install a newer published version.
//!
//! Shown at startup when the update check finds a newer release. The user can
//! install right away or postpone; the "Check for Updates" button in the
//! settings panel remains available for a manual check later.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::tauri_api;
use crate::types::{UpdateInfo, UpdateProgress};

/// Progress of the install started from this dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
enum InstallState {
    /// Waiting for the user to decide.
    Idle,
    /// The update package is downloading and installing.
    Installing,
    /// The install failed; the user may retry or postpone.
    Failed(String),
}

/// Dialog shown when a newer version of the application is available.
#[component]
pub fn UpdateDialog(
    /// The update to offer. The dialog is hidden while this is `None`.
    update: ReadSignal<Option<UpdateInfo>>,
    /// Callback when the user postpones the update.
    on_dismiss: Callback<()>,
) -> impl IntoView {
    let (install_state, set_install_state) = signal(InstallState::Idle);
    let (progress, set_progress) = signal::<Option<UpdateProgress>>(None);

    // Subscribe to download progress once; the app restarts as soon as an
    // install finishes, so the listener never needs tearing down.
    Effect::new(move || {
        spawn_local(async move {
            let listener =
                tauri_api::listen_to_update_progress(move |p| set_progress.set(Some(p))).await;
            if let Err(e) = listener {
                leptos::logging::error!("Failed to listen for update progress: {}", e);
            }
        });
    });

    let is_installing = move || install_state.get() == InstallState::Installing;

    // Postponing is only possible while nothing is being installed.
    let dismiss = move || {
        if !is_installing() {
            on_dismiss.run(());
        }
    };

    // Download and install; the app restarts on success, so the only outcome
    // handled here is failure.
    let on_install = move |_| {
        spawn_local(async move {
            set_progress.set(None);
            set_install_state.set(InstallState::Installing);
            if let Err(e) = tauri_api::install_update().await {
                leptos::logging::error!("Update install failed: {}", e);
                set_install_state.set(InstallState::Failed(e));
            }
        });
    };

    view! {
        <div
            class="confirm-dialog-overlay"
            class:visible=move || update.get().is_some()
            on:click=move |_| dismiss()
        >
            <div
                class="confirm-dialog update-dialog"
                on:click=move |e| e.stop_propagation()
                role="dialog"
                aria-modal="true"
                aria-labelledby="update-dialog-title"
                aria-describedby="update-dialog-message"
            >
                <div class="confirm-dialog-icon update">
                    <svg viewBox="0 0 24 24" width="48" height="48" fill="currentColor">
                        <path d="M12 16.5l4-4h-3v-9h-2v9H8l4 4zm9-13h-6v1.99h6v14.03H3V5.49h6V3.5H3c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2v-14c0-1.1-.9-2-2-2z"/>
                    </svg>
                </div>
                <h3 id="update-dialog-title" class="confirm-dialog-title">
                    "Update Available"
                </h3>
                <p id="update-dialog-message" class="confirm-dialog-message">
                    {move || update.get().map(|info| {
                        format!(
                            "Youtun4 {} is available. You are running {}.",
                            info.version, info.current_version
                        )
                    })}
                </p>

                {move || update.get().and_then(|info| info.date).map(|date| view! {
                    <p class="update-dialog-date">{format!("Released {date}")}</p>
                })}

                {move || update.get().and_then(|info| info.notes).map(|notes| view! {
                    <div class="update-dialog-notes">
                        <span class="update-dialog-notes-label">"What's new"</span>
                        <pre class="update-dialog-notes-body">{notes}</pre>
                    </div>
                })}

                {move || match install_state.get() {
                    InstallState::Idle => None,
                    InstallState::Installing => Some(view! {
                        <div class="update-dialog-progress">
                            <div class="download-progress-bar-container">
                                <div class="download-progress-bar">
                                    <div
                                        class="download-progress-fill"
                                        style:width=move || progress.get()
                                            .and_then(|p| p.percent())
                                            .map_or_else(
                                                || "0%".to_string(),
                                                |percent| format!("{percent}%"),
                                            )
                                    ></div>
                                </div>
                                <div class="download-progress-percent">
                                    {move || progress.get()
                                        .and_then(|p| p.percent())
                                        .map_or_else(
                                            || "--".to_string(),
                                            |percent| format!("{percent}%"),
                                        )}
                                </div>
                            </div>
                            <p class="settings-hint">
                                {move || progress.get().map_or_else(
                                    || "Starting download...".to_string(),
                                    |p| p.size_label(),
                                )}
                            </p>
                        </div>
                    }.into_any()),
                    InstallState::Failed(message) => Some(view! {
                        <div class="settings-message settings-error update-dialog-error">
                            <span>{message}</span>
                        </div>
                    }.into_any()),
                }}

                <div class="confirm-dialog-actions">
                    <button
                        class="btn btn-secondary"
                        on:click=move |_| dismiss()
                        disabled=is_installing
                    >
                        "Later"
                    </button>
                    <button
                        class="btn btn-primary"
                        on:click=on_install
                        disabled=is_installing
                    >
                        {move || if is_installing() {
                            view! { <span class="spinner"></span> " Installing..." }.into_any()
                        } else {
                            view! { "Download & Install" }.into_any()
                        }}
                    </button>
                </div>
            </div>
        </div>
    }
}
