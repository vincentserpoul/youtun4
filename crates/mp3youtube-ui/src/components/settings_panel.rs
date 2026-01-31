//! Settings panel component for configuring application settings.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::tauri_api;
use crate::types::{AppConfig, DownloadQuality, NotificationPreferences, Theme};

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
    let (theme, set_theme) = signal::<Theme>(Theme::Dark);
    let (notif_download, set_notif_download) = signal(true);
    let (notif_sync, set_notif_sync) = signal(true);
    let (notif_errors, set_notif_errors) = signal(true);
    let (notif_device, set_notif_device) = signal(true);

    // UI state
    let (is_loading, set_is_loading) = signal(false);
    let (error_message, set_error_message) = signal::<Option<String>>(None);
    let (success_message, set_success_message) = signal::<Option<String>>(None);
    let (active_tab, set_active_tab) = signal::<&'static str>("storage");

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
                        set_theme.set(config.theme);
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

                set_is_loading.set(false);
            });
        }
    });

    // Save settings handler
    let on_save = move |_| {
        let new_dir = storage_dir.get();
        let new_quality = download_quality.get();
        let new_theme = theme.get();
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
                theme: new_theme,
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
        set_theme.set(Theme::Dark);
        set_notif_download.set(true);
        set_notif_sync.set(true);
        set_notif_errors.set(true);
        set_notif_device.set(true);
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
                        class:active=move || active_tab.get() == "appearance"
                        on:click=move |_| set_active_tab.set("appearance")
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                            <path d="M12 3c-4.97 0-9 4.03-9 9s4.03 9 9 9c.83 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-.99 0-.83.67-1.5 1.5-1.5H16c2.76 0 5-2.24 5-5 0-4.42-4.03-8-9-8zm-5.5 9c-.83 0-1.5-.67-1.5-1.5S5.67 9 6.5 9 8 9.67 8 10.5 7.33 12 6.5 12zm3-4C8.67 8 8 7.33 8 6.5S8.67 5 9.5 5s1.5.67 1.5 1.5S10.33 8 9.5 8zm5 0c-.83 0-1.5-.67-1.5-1.5S13.67 5 14.5 5s1.5.67 1.5 1.5S15.33 8 14.5 8zm3 4c-.83 0-1.5-.67-1.5-1.5S16.67 9 17.5 9s1.5.67 1.5 1.5-.67 1.5-1.5 1.5z"/>
                        </svg>
                        "Appearance"
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

                    // Appearance Tab
                    <div class="settings-tab-content" class:hidden=move || active_tab.get() != "appearance">
                        <div class="settings-section">
                            <h3>"Theme"</h3>
                            <p class="settings-description">
                                "Choose the appearance of the application."
                            </p>

                            <div class="settings-field">
                                <div class="settings-theme-options">
                                    <button
                                        class="settings-theme-option"
                                        class:selected=move || theme.get() == Theme::Dark
                                        on:click=move |_| set_theme.set(Theme::Dark)
                                        disabled=move || is_loading.get()
                                    >
                                        <div class="theme-preview theme-preview-dark">
                                            <div class="theme-preview-header"></div>
                                            <div class="theme-preview-content">
                                                <div class="theme-preview-sidebar"></div>
                                                <div class="theme-preview-main"></div>
                                            </div>
                                        </div>
                                        <span class="theme-label">"Dark"</span>
                                    </button>
                                    <button
                                        class="settings-theme-option"
                                        class:selected=move || theme.get() == Theme::Light
                                        on:click=move |_| set_theme.set(Theme::Light)
                                        disabled=move || is_loading.get()
                                    >
                                        <div class="theme-preview theme-preview-light">
                                            <div class="theme-preview-header"></div>
                                            <div class="theme-preview-content">
                                                <div class="theme-preview-sidebar"></div>
                                                <div class="theme-preview-main"></div>
                                            </div>
                                        </div>
                                        <span class="theme-label">"Light"</span>
                                    </button>
                                    <button
                                        class="settings-theme-option"
                                        class:selected=move || theme.get() == Theme::System
                                        on:click=move |_| set_theme.set(Theme::System)
                                        disabled=move || is_loading.get()
                                    >
                                        <div class="theme-preview theme-preview-system">
                                            <div class="theme-preview-header"></div>
                                            <div class="theme-preview-content">
                                                <div class="theme-preview-sidebar"></div>
                                                <div class="theme-preview-main"></div>
                                            </div>
                                        </div>
                                        <span class="theme-label">"System"</span>
                                    </button>
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
