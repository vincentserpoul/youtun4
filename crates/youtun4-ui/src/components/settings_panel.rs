//! Settings panel component for configuring application settings.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::tauri_api;
use crate::types::{
    AppConfig, DownloadQuality, NotificationPreferences, Theme, UpdateInfo, UpdateProgress,
};

/// Bytes per megabyte, for progress labels.
const BYTES_PER_MB: u64 = 1024 * 1024;

/// State of the self-update flow.
#[derive(Debug, Clone, PartialEq, Eq)]
enum UpdateStatus {
    /// No check has been run yet.
    Idle,
    /// Contacting the update endpoint.
    Checking,
    /// The running version is the published one.
    UpToDate,
    /// A newer version is published.
    Available(UpdateInfo),
    /// The update package is downloading and installing.
    Installing,
    /// The check or the install failed.
    Failed(String),
}

/// Render a download size as `"12 MB / 34 MB"`, or `"12 MB"` when the total
/// size is unknown.
fn progress_label(progress: UpdateProgress) -> String {
    let downloaded = progress.downloaded / BYTES_PER_MB;
    progress.total.map_or_else(
        || format!("{downloaded} MB"),
        |total| format!("{downloaded} MB / {} MB", total / BYTES_PER_MB),
    )
}

/// Settings panel component for configuring application preferences.
#[component]

pub fn SettingsPanel(
    /// Whether the settings panel is open.
    is_open: ReadSignal<bool>,
    /// Callback to close the settings panel.
    on_close: Callback<()>,
) -> impl IntoView {
    // Local state for settings
    let (storage_dir, set_storage_dir) = signal::<String>(String::new());
    let (default_dir, set_default_dir) = signal::<String>(String::new());
    let (download_quality, set_download_quality) =
        signal::<DownloadQuality>(DownloadQuality::Medium);
    let (notif_download, set_notif_download) = signal(true);
    let (notif_sync, set_notif_sync) = signal(true);
    let (notif_errors, set_notif_errors) = signal(true);
    let (notif_device, set_notif_device) = signal(true);

    // UI state
    let (is_loading, set_is_loading) = signal(false);
    let (error_message, set_error_message) = signal::<Option<String>>(None);
    let (success_message, set_success_message) = signal::<Option<String>>(None);
    let (active_tab, set_active_tab) = signal::<&'static str>("storage");

    // Self-update state
    let (app_version, set_app_version) = signal::<String>(String::new());
    let (update_status, set_update_status) = signal(UpdateStatus::Idle);
    let (update_progress, set_update_progress) = signal::<Option<UpdateProgress>>(None);

    // Subscribe to download progress once; the panel outlives any single
    // update, and the app restarts as soon as one finishes installing.
    Effect::new(move || {
        spawn_local(async move {
            let listener =
                tauri_api::listen_to_update_progress(move |p| set_update_progress.set(Some(p)))
                    .await;
            if let Err(e) = listener {
                leptos::logging::error!("Failed to listen for update progress: {}", e);
            }
        });
    });

    // Load current settings when panel opens
    Effect::new(move || {
        if is_open.get() {
            spawn_local(async move {
                set_is_loading.set(true);
                set_error_message.set(None);
                set_success_message.set(None);

                // Load current configuration
                match tauri_api::get_config().await {
                    Ok(config) => {
                        set_storage_dir.set(config.playlists_directory);
                        set_download_quality.set(config.download_quality);
                        set_notif_download.set(config.notification_preferences.download_complete);
                        set_notif_sync.set(config.notification_preferences.sync_complete);
                        set_notif_errors.set(config.notification_preferences.errors);
                        set_notif_device.set(config.notification_preferences.device_connected);
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to load config: {}", e);
                        set_error_message.set(Some(format!("Failed to load settings: {e}")));
                    }
                }

                // Load default directory
                match tauri_api::get_default_storage_directory().await {
                    Ok(dir) => {
                        set_default_dir.set(dir);
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to load default directory: {}", e);
                    }
                }

                // Load the running version for the Updates tab
                match tauri_api::get_app_version().await {
                    Ok(version) => {
                        set_app_version.set(version);
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to load app version: {}", e);
                    }
                }

                set_is_loading.set(false);
            });
        }
    });

    // Save settings handler
    let on_save = move |_| {
        let new_dir = storage_dir.get();
        let new_quality = download_quality.get();
        let notif_prefs = NotificationPreferences {
            download_complete: notif_download.get(),
            sync_complete: notif_sync.get(),
            errors: notif_errors.get(),
            device_connected: notif_device.get(),
        };

        spawn_local(async move {
            set_is_loading.set(true);
            set_error_message.set(None);
            set_success_message.set(None);

            let config = AppConfig {
                playlists_directory: new_dir,
                download_quality: new_quality,
                theme: Theme::Dark,
                notification_preferences: notif_prefs,
            };

            match tauri_api::update_config(&config).await {
                Ok(()) => {
                    leptos::logging::log!("Configuration updated successfully");
                    set_success_message.set(Some("Settings saved successfully!".to_string()));
                }
                Err(e) => {
                    leptos::logging::error!("Failed to save config: {}", e);
                    set_error_message.set(Some(format!("Failed to save settings: {e}")));
                }
            }

            set_is_loading.set(false);
        });
    };

    // Reset to default handler
    let on_reset = move |_| {
        let default = default_dir.get();
        set_storage_dir.set(default);
        set_download_quality.set(DownloadQuality::Medium);
        set_notif_download.set(true);
        set_notif_sync.set(true);
        set_notif_errors.set(true);
        set_notif_device.set(true);
    };

    // Check for a newer published version
    let on_check_update = move |_| {
        spawn_local(async move {
            set_update_status.set(UpdateStatus::Checking);
            match tauri_api::check_for_update().await {
                Ok(Some(info)) => set_update_status.set(UpdateStatus::Available(info)),
                Ok(None) => set_update_status.set(UpdateStatus::UpToDate),
                Err(e) => {
                    leptos::logging::error!("Update check failed: {}", e);
                    set_update_status.set(UpdateStatus::Failed(e));
                }
            }
        });
    };

    // Download and install the update; the app restarts on success, so the
    // only outcome handled here is failure.
    let on_install_update = move |_| {
        spawn_local(async move {
            set_update_progress.set(None);
            set_update_status.set(UpdateStatus::Installing);
            if let Err(e) = tauri_api::install_update().await {
                leptos::logging::error!("Update install failed: {}", e);
                set_update_status.set(UpdateStatus::Failed(e));
            }
        });
    };

    view! {
        <div
            class="settings-overlay"
            class:visible=move || is_open.get()
            on:click=move |_| on_close.run(())
        >
            <div
                class="settings-panel"
                on:click=move |e| e.stop_propagation()
            >
                <div class="settings-header">
                    <h2>"Settings"</h2>
                    <button
                        class="btn btn-ghost btn-icon"
                        on:click=move |_| on_close.run(())
                    >
                        <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                            <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                        </svg>
                    </button>
                </div>

                // Tab navigation
                <div class="settings-tabs">
                    <button
                        class="settings-tab"
                        class:active=move || active_tab.get() == "storage"
                        on:click=move |_| set_active_tab.set("storage")
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                            <path d="M20 6h-8l-2-2H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2z"/>
                        </svg>
                        "Storage"
                    </button>
                    <button
                        class="settings-tab"
                        class:active=move || active_tab.get() == "downloads"
                        on:click=move |_| set_active_tab.set("downloads")
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                            <path d="M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z"/>
                        </svg>
                        "Downloads"
                    </button>
                    <button
                        class="settings-tab"
                        class:active=move || active_tab.get() == "notifications"
                        on:click=move |_| set_active_tab.set("notifications")
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                            <path d="M12 22c1.1 0 2-.9 2-2h-4c0 1.1.89 2 2 2zm6-6v-5c0-3.07-1.64-5.64-4.5-6.32V4c0-.83-.67-1.5-1.5-1.5s-1.5.67-1.5 1.5v.68C7.63 5.36 6 7.92 6 11v5l-2 2v1h16v-1l-2-2z"/>
                        </svg>
                        "Notifications"
                    </button>
                    <button
                        class="settings-tab"
                        class:active=move || active_tab.get() == "updates"
                        on:click=move |_| set_active_tab.set("updates")
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                            <path d="M12 4V1L8 5l4 4V6c3.31 0 6 2.69 6 6 0 1.01-.25 1.97-.7 2.8l1.46 1.46C19.54 15.03 20 13.57 20 12c0-4.42-3.58-8-8-8zm0 14c-3.31 0-6-2.69-6-6 0-1.01.25-1.97.7-2.8L5.24 7.74C4.46 8.97 4 10.43 4 12c0 4.42 3.58 8 8 8v3l4-4-4-4v3z"/>
                        </svg>
                        "Updates"
                    </button>
                </div>

                <div class="settings-body">
                    // Error message
                    {move || error_message.get().map(|msg| view! {
                        <div class="settings-message settings-error">
                            <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                            </svg>
                            <span>{msg}</span>
                        </div>
                    })}

                    // Success message
                    {move || success_message.get().map(|msg| view! {
                        <div class="settings-message settings-success">
                            <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                            </svg>
                            <span>{msg}</span>
                        </div>
                    })}

                    // Storage Tab
                    <div class="settings-tab-content" class:hidden=move || active_tab.get() != "storage">
                        <div class="settings-section">
                            <h3>"Storage Location"</h3>
                            <p class="settings-description">
                                "Choose where your playlists are stored on your computer."
                            </p>

                            <div class="settings-field">
                                <label for="storage-dir">"Playlists Directory"</label>
                                <div class="settings-input-group">
                                    <input
                                        id="storage-dir"
                                        type="text"
                                        class="settings-input"
                                        prop:value=move || storage_dir.get()
                                        on:input=move |ev| {
                                            set_storage_dir.set(event_target_value(&ev));
                                        }
                                        placeholder="Enter directory path..."
                                        disabled=move || is_loading.get()
                                    />
                                </div>
                                <p class="settings-hint">
                                    "Default: " {move || default_dir.get()}
                                </p>
                            </div>
                        </div>
                    </div>

                    // Downloads Tab
                    <div class="settings-tab-content" class:hidden=move || active_tab.get() != "downloads">
                        <div class="settings-section">
                            <h3>"Download Quality"</h3>
                            <p class="settings-description">
                                "Select the audio quality for YouTube downloads."
                            </p>

                            <div class="settings-field">
                                <div class="settings-radio-group">
                                    <label class="settings-radio-option">
                                        <input
                                            type="radio"
                                            name="download-quality"
                                            checked=move || download_quality.get() == DownloadQuality::Low
                                            on:change=move |_| set_download_quality.set(DownloadQuality::Low)
                                            disabled=move || is_loading.get()
                                        />
                                        <span class="settings-radio-label">
                                            <span class="settings-radio-title">"Low"</span>
                                            <span class="settings-radio-description">"128 kbps - Smaller files, lower quality"</span>
                                        </span>
                                    </label>
                                    <label class="settings-radio-option">
                                        <input
                                            type="radio"
                                            name="download-quality"
                                            checked=move || download_quality.get() == DownloadQuality::Medium
                                            on:change=move |_| set_download_quality.set(DownloadQuality::Medium)
                                            disabled=move || is_loading.get()
                                        />
                                        <span class="settings-radio-label">
                                            <span class="settings-radio-title">"Medium"</span>
                                            <span class="settings-radio-description">"192 kbps - Balanced quality and size"</span>
                                        </span>
                                    </label>
                                    <label class="settings-radio-option">
                                        <input
                                            type="radio"
                                            name="download-quality"
                                            checked=move || download_quality.get() == DownloadQuality::High
                                            on:change=move |_| set_download_quality.set(DownloadQuality::High)
                                            disabled=move || is_loading.get()
                                        />
                                        <span class="settings-radio-label">
                                            <span class="settings-radio-title">"High"</span>
                                            <span class="settings-radio-description">"320 kbps - Best quality, larger files"</span>
                                        </span>
                                    </label>
                                </div>
                            </div>
                        </div>
                    </div>

                    // Notifications Tab
                    <div class="settings-tab-content" class:hidden=move || active_tab.get() != "notifications">
                        <div class="settings-section">
                            <h3>"Notification Preferences"</h3>
                            <p class="settings-description">
                                "Choose which notifications you want to receive."
                            </p>

                            <div class="settings-field">
                                <div class="settings-toggle-group">
                                    <label class="settings-toggle-option">
                                        <span class="settings-toggle-label">
                                            <span class="settings-toggle-title">"Download Complete"</span>
                                            <span class="settings-toggle-description">"Notify when a playlist download finishes"</span>
                                        </span>
                                        <input
                                            type="checkbox"
                                            class="settings-toggle"
                                            checked=move || notif_download.get()
                                            on:change=move |ev| set_notif_download.set(event_target_checked(&ev))
                                            disabled=move || is_loading.get()
                                        />
                                    </label>
                                    <label class="settings-toggle-option">
                                        <span class="settings-toggle-label">
                                            <span class="settings-toggle-title">"Sync Complete"</span>
                                            <span class="settings-toggle-description">"Notify when syncing to a device finishes"</span>
                                        </span>
                                        <input
                                            type="checkbox"
                                            class="settings-toggle"
                                            checked=move || notif_sync.get()
                                            on:change=move |ev| set_notif_sync.set(event_target_checked(&ev))
                                            disabled=move || is_loading.get()
                                        />
                                    </label>
                                    <label class="settings-toggle-option">
                                        <span class="settings-toggle-label">
                                            <span class="settings-toggle-title">"Errors"</span>
                                            <span class="settings-toggle-description">"Show notifications for errors and warnings"</span>
                                        </span>
                                        <input
                                            type="checkbox"
                                            class="settings-toggle"
                                            checked=move || notif_errors.get()
                                            on:change=move |ev| set_notif_errors.set(event_target_checked(&ev))
                                            disabled=move || is_loading.get()
                                        />
                                    </label>
                                    <label class="settings-toggle-option">
                                        <span class="settings-toggle-label">
                                            <span class="settings-toggle-title">"Device Connected"</span>
                                            <span class="settings-toggle-description">"Notify when a USB device is connected"</span>
                                        </span>
                                        <input
                                            type="checkbox"
                                            class="settings-toggle"
                                            checked=move || notif_device.get()
                                            on:change=move |ev| set_notif_device.set(event_target_checked(&ev))
                                            disabled=move || is_loading.get()
                                        />
                                    </label>
                                </div>
                            </div>
                        </div>
                    </div>

                    // Updates Tab
                    <div class="settings-tab-content" class:hidden=move || active_tab.get() != "updates">
                        <div class="settings-section">
                            <h3>"Application Updates"</h3>
                            <p class="settings-description">
                                "Youtun4 updates itself from signed GitHub releases."
                            </p>

                            <div class="settings-field">
                                <p class="settings-hint">
                                    "Installed version: " {move || app_version.get()}
                                </p>
                            </div>

                            {move || match update_status.get() {
                                UpdateStatus::Idle => None,
                                UpdateStatus::Checking => Some(view! {
                                    <p class="settings-hint">"Checking for updates..."</p>
                                }.into_any()),
                                UpdateStatus::UpToDate => Some(view! {
                                    <p class="settings-hint">"You are running the latest version."</p>
                                }.into_any()),
                                UpdateStatus::Available(info) => {
                                    let headline = info.date.as_ref().map_or_else(
                                        || format!("Version {} is available", info.version),
                                        |date| format!("Version {} is available ({date})", info.version),
                                    );
                                    let notes = info.notes.clone();
                                    Some(view! {
                                        <div class="settings-message settings-success">
                                            <span>{headline}</span>
                                        </div>
                                        {notes.map(|notes| view! {
                                            <p class="settings-hint">{notes}</p>
                                        })}
                                    }.into_any())
                                }
                                UpdateStatus::Installing => Some(view! {
                                    <div class="settings-field">
                                        <div class="download-progress-bar-container">
                                            <div class="download-progress-bar">
                                                <div
                                                    class="download-progress-fill"
                                                    style:width=move || update_progress.get()
                                                        .and_then(|p| p.percent())
                                                        .map_or_else(
                                                            || "0%".to_string(),
                                                            |percent| format!("{percent}%"),
                                                        )
                                                ></div>
                                            </div>
                                            <div class="download-progress-percent">
                                                {move || update_progress.get()
                                                    .and_then(|p| p.percent())
                                                    .map_or_else(
                                                        || "--".to_string(),
                                                        |percent| format!("{percent}%"),
                                                    )}
                                            </div>
                                        </div>
                                        <p class="settings-hint">
                                            {move || update_progress.get().map_or_else(
                                                || "Starting download...".to_string(),
                                                progress_label,
                                            )}
                                        </p>
                                    </div>
                                }.into_any()),
                                UpdateStatus::Failed(message) => Some(view! {
                                    <div class="settings-message settings-error">
                                        <span>{message}</span>
                                    </div>
                                }.into_any()),
                            }}

                            <div class="settings-field">
                                {move || match update_status.get() {
                                    UpdateStatus::Available(_) => view! {
                                        <button class="btn btn-primary" on:click=on_install_update>
                                            "Download & Install"
                                        </button>
                                    }.into_any(),
                                    UpdateStatus::Installing => view! {
                                        <button class="btn btn-primary" disabled=true>
                                            <span class="spinner"></span>
                                            " Installing..."
                                        </button>
                                    }.into_any(),
                                    UpdateStatus::Idle
                                    | UpdateStatus::Checking
                                    | UpdateStatus::UpToDate
                                    | UpdateStatus::Failed(_) => view! {
                                        <button
                                            class="btn btn-secondary"
                                            on:click=on_check_update
                                            disabled=move || update_status.get() == UpdateStatus::Checking
                                        >
                                            "Check for Updates"
                                        </button>
                                    }.into_any(),
                                }}
                            </div>
                        </div>
                    </div>
                </div>

                <div class="settings-footer">
                    <button
                        class="btn btn-secondary"
                        on:click=on_reset
                        disabled=move || is_loading.get()
                    >
                        "Reset to Default"
                    </button>
                    <div class="settings-footer-right">
                        <button
                            class="btn btn-ghost"
                            on:click=move |_| on_close.run(())
                            disabled=move || is_loading.get()
                        >
                            "Cancel"
                        </button>
                        <button
                            class="btn btn-primary"
                            on:click=on_save
                            disabled=move || is_loading.get()
                        >
                            {move || if is_loading.get() {
                                view! { <span class="spinner"></span> " Saving..." }.into_any()
                            } else {
                                view! { "Save Settings" }.into_any()
                            }}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
