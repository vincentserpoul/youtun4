//! Device selector panel and shared device card component.
//!
//! Provides `DeviceCard` for consistent device display across the UI,
//! and `DeviceSelectorPanel` for choosing which USB device to sync to.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::layout::MobileMenuContext;
use crate::components::loading::LoadingState;
use crate::tauri_api;
use crate::types::DeviceInfo;
use crate::utils::format_bytes;

/// Shared device card showing name, storage stats, and retro bar.
///
/// Used by both the "Selected Device" panel and the device selector list.
#[component]
#[allow(
    clippy::cast_precision_loss,
    reason = "precision loss is acceptable for display formatting"
)]
#[allow(
    clippy::float_arithmetic,
    reason = "float arithmetic needed for percentage calculation"
)]
pub fn DeviceCard(
    /// The device to display.
    device: DeviceInfo,
    /// Whether to show the active LED pulse animation.
    #[prop(default = false)]
    active: bool,
) -> impl IntoView {
    let usage_percent = device.usage_percentage();
    let used_str = format_bytes(device.used_bytes());
    let total_str = format_bytes(device.total_bytes);
    let usage_display = format!("{usage_percent:.1}%");

    let led_class = if active {
        "led-dot connected"
    } else {
        "led-dot connected no-pulse"
    };

    view! {
        <div class="device-card">
            <div class="device-card-header">
                <span class=led_class>
                    {active.then(|| view! { <span class="led-pulse-ring"></span> })}
                </span>
                <span class="device-card-name">{device.name}</span>
            </div>
            <div class="device-card-storage">
                <span class="device-card-percent">{usage_display}</span>
                <span class="device-card-bytes">{used_str}" / "{total_str}</span>
            </div>
            <div class="storage-bar-retro">
                <div
                    class="storage-bar-fill"
                    style=format!("width: {usage_percent:.1}%")
                ></div>
            </div>
        </div>
    }
}

/// Device selector panel component.
///
/// When open, shows a list of available devices for selection.
/// Each device displays a radio indicator, device card, and eject button.
#[component]

pub fn DeviceSelectorPanel(
    /// Signal containing list of available devices.
    devices: ReadSignal<Vec<DeviceInfo>>,
    /// Signal containing the currently selected device.
    selected_device: ReadSignal<Option<DeviceInfo>>,
    /// Callback when a device is selected.
    on_select: Callback<DeviceInfo>,
    /// Callback when a device is ejected (receives mount point).
    on_eject: Callback<String>,
    /// Callback to refresh/scan for devices.
    on_refresh: Callback<()>,
    /// Loading state of the device list.
    state: ReadSignal<LoadingState>,
    /// Whether the panel is open/visible.
    open: ReadSignal<bool>,
) -> impl IntoView {
    view! {
        {move || {
            if !open.get() {
                return None;
            }

            Some(view! {
                <div class="device-selector-panel" data-testid="device-selector-panel">
                    <div class="selector-header">
                        <span class="selector-title">"AVAILABLE DEVICES"</span>
                        <button
                            class="btn btn-ghost btn-icon"
                            title="Scan for devices"
                            disabled=move || state.get() == LoadingState::Loading
                            on:click=move |_| on_refresh.run(())
                        >
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                                <path d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/>
                            </svg>
                        </button>
                    </div>
                    {move || {
                        match state.get() {
                            LoadingState::Loading => {
                                view! {
                                    <div class="selector-loading">
                                        <span class="spinner"></span>
                                        <span class="selector-loading-text">"Scanning..."</span>
                                    </div>
                                }.into_any()
                            }
                            LoadingState::Error => {
                                view! {
                                    <div class="selector-empty">
                                        <p>"Failed to detect devices"</p>
                                        <button
                                            class="btn btn-ghost btn-sm"
                                            on:click=move |_| on_refresh.run(())
                                        >
                                            "Retry"
                                        </button>
                                    </div>
                                }.into_any()
                            }
                            LoadingState::Loaded => {
                                let device_list = devices.get();
                                if device_list.is_empty() {
                                    view! {
                                        <div class="selector-empty">
                                            <p>"No devices found"</p>
                                            <button
                                                class="btn btn-ghost btn-sm"
                                                on:click=move |_| on_refresh.run(())
                                            >
                                                "Scan"
                                            </button>
                                        </div>
                                    }.into_any()
                                } else {
                                    // Partition: selected device first, then others
                                    let selected = selected_device.get();
                                    let (selected_devices, other_devices): (Vec<_>, Vec<_>) =
                                        device_list.into_iter().partition(|d| {
                                            selected.as_ref().is_some_and(|s| s.mount_point == d.mount_point)
                                        });
                                    let has_selected = !selected_devices.is_empty();
                                    let has_others = !other_devices.is_empty();

                                    view! {
                                        <div class="selector-list">
                                            // Selected device at top
                                            {selected_devices.into_iter().map(|device| {
                                                view! {
                                                    <SelectorItem
                                                        device=device
                                                        selected=true
                                                        on_select=on_select
                                                        on_eject=on_eject
                                                    />
                                                }
                                            }).collect_view()}
                                            // Separator between selected and others
                                            {(has_selected && has_others).then(|| view! {
                                                <div class="selector-separator"></div>
                                            })}
                                            // Remaining devices
                                            {other_devices.into_iter().map(|device| {
                                                view! {
                                                    <SelectorItem
                                                        device=device
                                                        selected=false
                                                        on_select=on_select
                                                        on_eject=on_eject
                                                    />
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }
                            }
                        }
                    }}
                </div>
            })
        }}
    }
}

/// A single device item in the selector panel.
#[component]
fn SelectorItem(
    /// The device to display.
    device: DeviceInfo,
    /// Whether this device is currently selected.
    #[prop(default = false)]
    selected: bool,
    /// Callback when device is selected.
    on_select: Callback<DeviceInfo>,
    /// Callback when device is ejected.
    on_eject: Callback<String>,
) -> impl IntoView {
    let device_for_click = device.clone();
    let device_for_eject = device.clone();
    let device_for_card = device;

    let menu_ctx = use_context::<MobileMenuContext>();

    let (ejecting, set_ejecting) = signal(false);

    let handle_eject = move |e: web_sys::MouseEvent| {
        e.stop_propagation();
        if ejecting.get() {
            return;
        }

        set_ejecting.set(true);
        let mount_point = device_for_eject.mount_point.clone();
        let on_eject = on_eject;

        spawn_local(async move {
            match tauri_api::eject_device(&mount_point).await {
                Ok(result) => {
                    if result.success {
                        leptos::logging::log!("Device ejected successfully: {}", mount_point);
                        on_eject.run(mount_point);
                    } else {
                        leptos::logging::error!("Failed to eject device: {}", mount_point);
                    }
                }
                Err(e) => {
                    leptos::logging::error!("Failed to eject device: {}", e);
                }
            }
            set_ejecting.set(false);
        });
    };

    let item_class = if selected {
        "selector-item selected"
    } else {
        "selector-item"
    };

    view! {
        <div
            class=item_class
            on:click=move |_| {
                on_select.run(device_for_click.clone());
                if let Some(ctx) = menu_ctx {
                    ctx.close();
                }
            }
            data-testid="selector-item"
        >
            <div class="radio-indicator">
                {selected.then(|| view! { <span class="radio-dot"></span> })}
            </div>
            <div class="selector-item-content">
                <DeviceCard device=device_for_card />
            </div>
            <button
                class="btn btn-ghost btn-icon btn-eject-sm"
                title="Safely eject device"
                disabled=move || ejecting.get()
                on:click=handle_eject
            >
                {move || {
                    if ejecting.get() {
                        view! { <span class="spinner spinner-sm"></span> }.into_any()
                    } else {
                        view! {
                            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                                <path d="M5 17h14v2H5zm7-12L5.33 15h13.34z"/>
                            </svg>
                        }.into_any()
                    }
                }}
            </button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }
}
