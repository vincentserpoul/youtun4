//! Dialog component for creating new playlists from `YouTube` URLs.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::tauri_api;
use crate::types::{YouTubeUrlType, YouTubeUrlValidation};

/// State of the URL validation process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UrlValidationState {
    /// No validation in progress, waiting for input.
    #[default]
    Idle,
    /// Validation is in progress.
    Validating,
    /// URL is valid.
    Valid,
    /// URL is invalid.
    Invalid,
}

/// Dialog component for creating new playlists from YouTube URLs.
#[component]

pub fn CreatePlaylistDialog(
    /// Whether the dialog is open.
    is_open: ReadSignal<bool>,
    /// Callback when a playlist is successfully created.
    on_create: Callback<String>,
    /// Callback when the dialog is closed (cancelled or after creation).
    on_close: Callback<()>,
) -> impl IntoView {
    // Form state
    let (url_input, set_url_input) = signal(String::new());
    let (name_input, set_name_input) = signal(String::new());
    let (name_touched, set_name_touched) = signal(false);

    // Validation state
    let (url_validation_state, set_url_validation_state) = signal(UrlValidationState::Idle);
    let (url_validation, set_url_validation) = signal(YouTubeUrlValidation::pending());
    let (name_error, set_name_error) = signal::<Option<String>>(None);

    // Loading and error state
    let (is_creating, set_is_creating) = signal(false);
    let (create_error, set_create_error) = signal::<Option<String>>(None);

    // Reset form when dialog opens
    Effect::new(move || {
        if is_open.get() {
            set_url_input.set(String::new());
            set_name_input.set(String::new());
            set_name_touched.set(false);
            set_url_validation_state.set(UrlValidationState::Idle);
            set_url_validation.set(YouTubeUrlValidation::pending());
            set_name_error.set(None);
            set_is_creating.set(false);
            set_create_error.set(None);
        }
    });

    // URL validation effect - validates URL when input changes (with debouncing effect)
    let validate_url = move |url: String| {
        if url.trim().is_empty() {
            set_url_validation_state.set(UrlValidationState::Idle);
            set_url_validation.set(YouTubeUrlValidation::pending());
            return;
        }

        set_url_validation_state.set(UrlValidationState::Validating);

        spawn_local(async move {
            match tauri_api::validate_youtube_playlist_url(&url).await {
                Ok(validation) => {
                    if validation.is_valid {
                        set_url_validation_state.set(UrlValidationState::Valid);
                    } else {
                        set_url_validation_state.set(UrlValidationState::Invalid);
                    }
                    set_url_validation.set(validation);
                }
                Err(e) => {
                    leptos::logging::error!("URL validation error: {}", e);
                    set_url_validation_state.set(UrlValidationState::Invalid);
                    set_url_validation.set(YouTubeUrlValidation {
                        is_valid: false,
                        playlist_id: None,
                        normalized_url: None,
                        error_message: Some(format!("Validation error: {e}")),
                        url_type: YouTubeUrlType::Invalid,
                    });
                }
            }
        });
    };

    // Name validation
    let validate_name = move |name: &str| -> Option<String> {
        if name.trim().is_empty() {
            return Some("Playlist name is required".to_string());
        }
        if name.len() > 255 {
            return Some("Name must be 255 characters or less".to_string());
        }
        // Check for invalid filesystem characters
        let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
        for c in invalid_chars {
            if name.contains(c) {
                return Some(format!("Name cannot contain '{c}'"));
            }
        }
        // Check for reserved Windows names
        let reserved_names = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        let upper_name = name.trim().to_uppercase();
        if reserved_names.contains(&upper_name.as_str()) {
            return Some("This name is reserved by the system".to_string());
        }
        None
    };

    // Handle URL input change
    let on_url_change = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        set_url_input.set(value.clone());
        set_create_error.set(None);
        validate_url(value);
    };

    // Handle name input change
    let on_name_change = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        set_name_input.set(value.clone());
        set_create_error.set(None);

        if name_touched.get() {
            set_name_error.set(validate_name(&value));
        }
    };

    // Handle name input blur (for validation)
    let on_name_blur = move |_| {
        set_name_touched.set(true);
        set_name_error.set(validate_name(&name_input.get()));
    };

    // Check if form is valid for submission
    let is_form_valid = move || {
        let url_valid = url_validation_state.get() == UrlValidationState::Valid;
        let name_valid = validate_name(&name_input.get()).is_none();
        url_valid && name_valid && !is_creating.get()
    };

    // Handle create button click
    let on_create_click = move |_| {
        let name = name_input.get().trim().to_string();
        let url = url_input.get().trim().to_string();
        let validation = url_validation.get();

        // Final validation before submit
        if !is_form_valid() {
            return;
        }

        set_is_creating.set(true);
        set_create_error.set(None);

        // Use normalized URL if available
        let source_url = validation.normalized_url.unwrap_or(url);

        spawn_local(async move {
            // First create the playlist
            leptos::logging::log!("UI: About to call create_playlist for: {}", name);
            match tauri_api::create_playlist(&name, Some(&source_url)).await {
                Ok(created_name) => {
                    leptos::logging::log!("UI: Playlist created: {}", created_name);

                    // Start downloading tracks from the YouTube playlist
                    leptos::logging::log!(
                        "UI: Starting download for playlist: {} from {}",
                        name,
                        source_url
                    );

                    // Start download BEFORE closing dialog
                    let download_result =
                        tauri_api::download_youtube_to_playlist(&source_url, &name).await;

                    match download_result {
                        Ok(task_id) => {
                            leptos::logging::log!(
                                "UI: Download started with task ID: {:?}",
                                task_id
                            );
                        }
                        Err(e) => {
                            // Log the error but don't fail the playlist creation
                            // The playlist is created, user can retry download later
                            leptos::logging::error!("UI: Failed to start download: {}", e);
                        }
                    }

                    // NOW close dialog and notify
                    on_create.run(created_name);
                    on_close.run(());
                }
                Err(e) => {
                    leptos::logging::error!("UI: Failed to create playlist: {}", e);
                    set_create_error.set(Some(e));
                    set_is_creating.set(false);
                }
            }
        });
    };

    // Handle cancel/close
    let on_cancel = move |_| {
        on_close.run(());
    };

    // Handle overlay click
    let on_overlay_click = move |_| {
        if !is_creating.get() {
            on_close.run(());
        }
    };

    view! {
        <div
            class="create-playlist-dialog-overlay"
            class:visible=move || is_open.get()
            on:click=on_overlay_click
        >
            <div
                class="create-playlist-dialog"
                on:click=move |e| e.stop_propagation()
                role="dialog"
                aria-modal="true"
                aria-labelledby="create-playlist-dialog-title"
            >
                // Header
                <div class="create-playlist-dialog-header">
                    <h2 id="create-playlist-dialog-title">"New Playlist"</h2>
                    <button
                        class="btn btn-ghost btn-icon"
                        on:click=on_cancel
                        disabled=move || is_creating.get()
                        aria-label="Close dialog"
                    >
                        <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                            <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                        </svg>
                    </button>
                </div>

                // Body
                <div class="create-playlist-dialog-body">
                    // Error message from creation
                    {move || create_error.get().map(|msg| view! {
                        <div class="create-playlist-error">
                            <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                            </svg>
                            <span>{msg}</span>
                        </div>
                    })}

                    // YouTube URL Field
                    <div class="create-playlist-field">
                        <label for="playlist-url">"YouTube Playlist URL"</label>
                        <div class="create-playlist-input-wrapper">
                            <input
                                id="playlist-url"
                                type="url"
                                class="create-playlist-input"
                                class:error=move || url_validation_state.get() == UrlValidationState::Invalid
                                class:valid=move || url_validation_state.get() == UrlValidationState::Valid
                                prop:value=move || url_input.get()
                                on:input=on_url_change
                                placeholder="https://www.youtube.com/playlist?list=..."
                                disabled=move || is_creating.get()
                                autocomplete="off"
                                spellcheck="false"
                            />
                            // Validation indicator
                            <div class="create-playlist-input-indicator">
                                {move || match url_validation_state.get() {
                                    UrlValidationState::Idle => view! { <span></span> }.into_any(),
                                    UrlValidationState::Validating => view! {
                                        <span class="spinner"></span>
                                    }.into_any(),
                                    UrlValidationState::Valid => view! {
                                        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" class="check-icon">
                                            <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                                        </svg>
                                    }.into_any(),
                                    UrlValidationState::Invalid => view! {
                                        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" class="error-icon">
                                            <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                                        </svg>
                                    }.into_any(),
                                }}
                            </div>
                        </div>

                        // URL validation feedback
                        <div class="create-playlist-feedback">
                            {move || {
                                let validation = url_validation.get();
                                let state = url_validation_state.get();

                                match state {
                                    UrlValidationState::Idle => {
                                        view! {
                                            <p class="create-playlist-hint">
                                                "Paste a YouTube playlist URL to get started"
                                            </p>
                                        }.into_any()
                                    }
                                    UrlValidationState::Validating => {
                                        view! {
                                            <p class="create-playlist-hint validating">
                                                "Validating URL..."
                                            </p>
                                        }.into_any()
                                    }
                                    UrlValidationState::Valid => {
                                        view! {
                                            <div class="create-playlist-validation-success">
                                                <div class="validation-row">
                                                    <span class="validation-label">"Type:"</span>
                                                    <span class="validation-value">{url_type_label(validation.url_type)}</span>
                                                </div>
                                                {validation.playlist_id.map(|id| view! {
                                                    <div class="validation-row">
                                                        <span class="validation-label">"Playlist ID:"</span>
                                                        <span class="validation-value playlist-id">{id}</span>
                                                    </div>
                                                })}
                                            </div>
                                        }.into_any()
                                    }
                                    UrlValidationState::Invalid => {
                                        view! {
                                            <p class="create-playlist-error-text">
                                                {validation.error_message.unwrap_or_else(|| "Invalid URL".to_string())}
                                            </p>
                                        }.into_any()
                                    }
                                }
                            }}
                        </div>
                    </div>

                    // Playlist Name Field
                    <div class="create-playlist-field">
                        <label for="playlist-name">"Playlist Name"</label>
                        <input
                            id="playlist-name"
                            type="text"
                            class="create-playlist-input"
                            class:error=move || name_error.get().is_some()
                            prop:value=move || name_input.get()
                            on:input=on_name_change
                            on:blur=on_name_blur
                            placeholder="Enter a name for your playlist"
                            disabled=move || is_creating.get()
                            maxlength="255"
                        />
                        // Name validation feedback
                        {move || name_error.get().map(|err| view! {
                            <p class="create-playlist-error-text">{err}</p>
                        })}
                    </div>
                </div>

                // Footer
                <div class="create-playlist-dialog-footer">
                    <button
                        class="btn btn-secondary"
                        on:click=on_cancel
                        disabled=move || is_creating.get()
                    >
                        "Cancel"
                    </button>
                    <button
                        class="btn btn-primary"
                        on:click=on_create_click
                        disabled=move || !is_form_valid()
                    >
                        {move || if is_creating.get() {
                            view! {
                                <span class="spinner"></span>
                                " Creating..."
                            }.into_any()
                        } else {
                            view! {
                                <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                                    <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
                                </svg>
                                " Create Playlist"
                            }.into_any()
                        }}
                    </button>
                </div>
            </div>
        </div>
    }
}

/// Helper function to get a user-friendly label for the URL type.
const fn url_type_label(url_type: YouTubeUrlType) -> &'static str {
    match url_type {
        YouTubeUrlType::Playlist => "YouTube Playlist",
        YouTubeUrlType::WatchWithPlaylist => "Video with Playlist",
        YouTubeUrlType::SingleVideo => "Single Video (no playlist)",
        YouTubeUrlType::ShortUrl => "Short URL",
        YouTubeUrlType::Invalid => "Invalid URL",
    }
}
