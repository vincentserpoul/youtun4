//! Device status indicator component showing connection status and device info.
//!
//! Compact inline display for the dashboard sidebar showing the selected device.

use leptos::prelude::*;

use crate::components::device_selector_panel::DeviceCard;
use crate::types::DeviceInfo;

/// Connection status for display purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Device is connected and accessible.
    Connected,
    /// No device is connected.
    Disconnected,
    /// Checking device status.
    Checking,
}

/// A compact indicator showing the connection status of a USB MP3 device.
///
/// Displays the selected device using the shared `DeviceCard` component.
#[component]

pub fn DeviceStatusIndicator(
    /// The currently selected/connected device, if any.
    device: ReadSignal<Option<DeviceInfo>>,
    /// Whether the device watcher is actively checking for devices.
    #[prop(default = false)]
    is_checking: bool,
) -> impl IntoView {
    let connection_status = move || {
        if is_checking {
            ConnectionStatus::Checking
        } else if device.get().is_some() {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        }
    };

    view! {
        <div class="device-management-panel" data-testid="device-status-indicator">
            <div class="panel-header">
                <h3 class="panel-title">"SELECTED DEVICE"</h3>
            </div>
            {move || {
                match connection_status() {
                    ConnectionStatus::Connected => {
                        if let Some(dev) = device.get() {
                            view! {
                                <DeviceCard device=dev active=true />
                            }.into_any()
                        } else {
                            view! {
                                <DeviceInfoDisconnected />
                            }.into_any()
                        }
                    }
                    ConnectionStatus::Disconnected => {
                        view! {
                            <DeviceInfoDisconnected />
                        }.into_any()
                    }
                    ConnectionStatus::Checking => {
                        view! {
                            <DeviceInfoChecking />
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

/// Disconnected device display.
#[component]
fn DeviceInfoDisconnected() -> impl IntoView {
    view! {
        <div class="device-card disconnected" data-testid="device-status-disconnected">
            <div class="device-card-header">
                <span class="led-dot disconnected"></span>
                <span class="device-card-name muted">"No device connected"</span>
            </div>
            <p class="device-hint">"Plug in your MP3 player via USB"</p>
        </div>
    }
}

/// Checking/scanning device display.
#[component]
fn DeviceInfoChecking() -> impl IntoView {
    view! {
        <div class="device-card checking" data-testid="device-status-checking">
            <div class="device-card-header">
                <span class="spinner spinner-sm"></span>
                <span class="device-card-name muted">"Scanning..."</span>
            </div>
        </div>
    }
}
