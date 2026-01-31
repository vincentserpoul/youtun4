//! Sync button component for triggering playlist synchronization.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::tauri_api;
use crate::types::{CapacityCheckResult, CapacityWarningLevel, DeviceInfo, PlaylistMetadata};

/// Sync button component that triggers playlist synchronization.
///
/// This component displays a prominent sync button that:
/// - Is disabled when no device is connected
/// - Is disabled when no playlist is selected
/// - Shows capacity warnings if the playlist is too large for the device
/// - Shows tooltip explanations for disabled states
#[component]

pub fn SyncButton(
    /// The currently selected device (None if no device connected/selected).
    selected_device: ReadSignal<Option<DeviceInfo>>,
    /// The currently selected playlist (None if no playlist selected).
    selected_playlist: ReadSignal<Option<PlaylistMetadata>>,
    /// Callback when sync is triggered. Receives the playlist name.
    on_sync: Callback<String>,
    /// Whether a sync operation is currently in progress (as a signal for reactivity).
    syncing: ReadSignal<bool>,
) -> impl IntoView {
    // Signal to store capacity check result
    let (capacity_result, set_capacity_result) = signal::<Option<CapacityCheckResult>>(None);
    let (checking_capacity, set_checking_capacity) = signal(false);

    // Effect to check capacity when device or playlist changes
    Effect::new(move |_| {
        let device = selected_device.get();
        let playlist = selected_playlist.get();

        if let (Some(d), Some(p)) = (device, playlist) {
            set_checking_capacity.set(true);
            let mount_point = d.mount_point;
            let playlist_name = p.name;

            spawn_local(async move {
                match tauri_api::check_sync_capacity(vec![playlist_name], &mount_point).await {
                    Ok(result) => {
                        set_capacity_result.set(Some(result));
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to check capacity: {}", e);
                        set_capacity_result.set(None);
                    }
                }
                set_checking_capacity.set(false);
            });
        } else {
            set_capacity_result.set(None);
        }
    });

    // Determine disabled state and tooltip message
    let button_state = move || {
        let device = selected_device.get();
        let playlist = selected_playlist.get();
        let capacity = capacity_result.get();

        match (device.is_some(), playlist.is_some()) {
            (false, false) => SyncButtonState::Disabled {
                reason: "Connect a device and select a playlist to sync".to_string(),
            },
            (false, true) => SyncButtonState::Disabled {
                reason: "Connect a USB device to sync".to_string(),
            },
            (true, false) => SyncButtonState::Disabled {
                reason: "Select a playlist to sync".to_string(),
            },
            (true, true) => {
                // Check capacity result
                if let Some(cap) = capacity {
                    if !cap.can_fit {
                        return SyncButtonState::InsufficientSpace { capacity: cap };
                    }
                    if cap.warning_level == CapacityWarningLevel::Warning {
                        return SyncButtonState::LimitedSpace { capacity: cap };
                    }
                }
                SyncButtonState::Enabled
            }
        }
    };

    let is_disabled = move || {
        matches!(
            button_state(),
            SyncButtonState::Disabled { .. } | SyncButtonState::InsufficientSpace { .. }
        ) || syncing.get()
            || checking_capacity.get()
    };

    let tooltip = move || {
        if syncing.get() {
            "Syncing in progress...".to_string()
        } else if checking_capacity.get() {
            "Checking available space...".to_string()
        } else {
            match button_state() {
                SyncButtonState::Disabled { reason } => reason,
                SyncButtonState::InsufficientSpace { capacity } => capacity.message,
                SyncButtonState::LimitedSpace { capacity } => {
                    let playlist = selected_playlist.get();
                    let device = selected_device.get();
                    match (playlist, device) {
                        (Some(p), Some(d)) => {
                            format!("Sync \"{}\" to {} ({})", p.name, d.name, capacity.message)
                        }
                        _ => capacity.message,
                    }
                }
                SyncButtonState::Enabled => {
                    let playlist = selected_playlist.get();
                    let device = selected_device.get();
                    match (playlist, device) {
                        (Some(p), Some(d)) => format!("Sync \"{}\" to {}", p.name, d.name),
                        _ => "Sync playlist to device".to_string(),
                    }
                }
            }
        }
    };

    let handle_click = move |_: web_sys::MouseEvent| {
        if !is_disabled()
            && let Some(playlist) = selected_playlist.get()
        {
            on_sync.run(playlist.name);
        }
    };

    // Determine button class based on state
    let button_class = move || {
        let base = "btn sync-button";
        match button_state() {
            SyncButtonState::InsufficientSpace { .. } => format!("{base} btn-danger"),
            SyncButtonState::LimitedSpace { .. } => format!("{base} btn-warning"),
            _ => format!("{base} btn-primary"),
        }
    };

    view! {
        <div class="sync-button-container" data-testid="sync-button-container">
            <button
                class=button_class
                class:syncing=move || syncing.get()
                disabled=is_disabled
                title=tooltip
                on:click=handle_click
                data-testid="sync-button"
            >
                {move || {
                    if syncing.get() {
                        view! {
                            <span class="spinner"></span>
                            <span>"Syncing..."</span>
                        }.into_any()
                    } else if checking_capacity.get() {
                        view! {
                            <span class="spinner"></span>
                            <span>"Checking space..."</span>
                        }.into_any()
                    } else {
                        view! {
                            <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" class="sync-icon">
                                <path d="M19 8l-4 4h3c0 3.31-2.69 6-6 6-1.01 0-1.97-.25-2.8-.7l-1.46 1.46C8.97 19.54 10.43 20 12 20c4.42 0 8-3.58 8-8h3l-4-4zM6 12c0-3.31 2.69-6 6-6 1.01 0 1.97.25 2.8.7l1.46-1.46C15.03 4.46 13.57 4 12 4c-4.42 0-8 3.58-8 8H1l4 4 4-4H6z"/>
                            </svg>
                            <span>"Sync to Device"</span>
                        }.into_any()
                    }
                }}
            </button>
            // Capacity warning/error indicator below button
            {move || {
                match button_state() {
                    SyncButtonState::InsufficientSpace { capacity } => {
                        Some(view! {
                            <div class="sync-button-hint sync-button-error" data-testid="sync-capacity-error">
                                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                                </svg>
                                <span>{capacity.message.clone()}</span>
                                <span class="capacity-details">
                                    {format!(" (need {}, have {})", capacity.formatted_required(), capacity.formatted_available())}
                                </span>
                            </div>
                        }.into_any())
                    }
                    SyncButtonState::LimitedSpace { capacity } => {
                        Some(view! {
                            <div class="sync-button-hint sync-button-warning" data-testid="sync-capacity-warning">
                                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                    <path d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"/>
                                </svg>
                                <span>{format!("Low space: {:.0}% full after sync", capacity.usage_after_sync_percent)}</span>
                            </div>
                        }.into_any())
                    }
                    SyncButtonState::Disabled { reason } if !syncing.get() => {
                        Some(view! {
                            <div class="sync-button-hint" data-testid="sync-button-hint">
                                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                                </svg>
                                <span>{reason}</span>
                            </div>
                        }.into_any())
                    }
                    _ => None
                }
            }}
        </div>
    }
}

/// Internal state for the sync button.
#[derive(Clone, Debug)]
enum SyncButtonState {
    /// Button is enabled and ready to sync.
    Enabled,
    /// Button is disabled with a reason message.
    Disabled { reason: String },
    /// Insufficient space on device - sync cannot proceed.
    InsufficientSpace { capacity: CapacityCheckResult },
    /// Limited space warning - sync can proceed with caution.
    LimitedSpace { capacity: CapacityCheckResult },
}
