//! Device status indicator component showing connection status and device info.

use leptos::prelude::*;

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

/// Format bytes to human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// A visual indicator showing the connection status of a USB MP3 device.
///
/// Displays device name, capacity, available space, and connection state
/// with real-time updates.
#[component]

pub fn DeviceStatusIndicator(
    /// The currently selected/connected device, if any.
    device: ReadSignal<Option<DeviceInfo>>,
    /// Whether the device watcher is actively checking for devices.
    #[prop(default = false)]
    is_checking: bool,
) -> impl IntoView {
    // Derive connection status from device presence
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
        <div class="device-status-indicator" data-testid="device-status-indicator">
            {move || {
                match connection_status() {
                    ConnectionStatus::Connected => {
                        let dev = device.get().expect("Device should exist when connected");
                        view! {
                            <DeviceStatusConnected device=dev />
                        }.into_any()
                    }
                    ConnectionStatus::Disconnected => {
                        view! {
                            <DeviceStatusDisconnected />
                        }.into_any()
                    }
                    ConnectionStatus::Checking => {
                        view! {
                            <DeviceStatusChecking />
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

/// Connected device status display.
#[component]
fn DeviceStatusConnected(device: DeviceInfo) -> impl IntoView {
    let usage_percent = device.usage_percentage();
    let usage_class = if usage_percent > 90.0 {
        "critical"
    } else if usage_percent > 75.0 {
        "warning"
    } else {
        "normal"
    };

    view! {
        <div class="device-status-content connected" data-testid="device-status-connected">
            // Connection indicator dot
            <div class="status-indicator-dot connected" data-testid="connection-indicator">
                <span class="pulse-ring"></span>
            </div>

            // Device info section
            <div class="device-status-info">
                // Device name and connection status
                <div class="device-status-header">
                    <span class="device-status-name" data-testid="device-name">{device.name.clone()}</span>
                    <span class="device-status-badge connected">"Connected"</span>
                </div>

                // Mount point
                <div class="device-status-mount" data-testid="device-mount-point">
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                        <path d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
                    </svg>
                    <span>{device.mount_point.clone()}</span>
                </div>

                // Storage capacity bar
                <div class="device-status-storage">
                    <div class="storage-capacity-bar">
                        <div
                            class=format!("storage-capacity-fill {usage_class}")
                            style=format!("width: {}%", usage_percent)
                            data-testid="storage-bar"
                        ></div>
                    </div>
                    <div class="storage-capacity-text" data-testid="storage-text">
                        <span class="available">{format_bytes(device.available_bytes)}" free"</span>
                        <span class="separator">" / "</span>
                        <span class="total">{format_bytes(device.total_bytes)}</span>
                    </div>
                </div>

                // Additional device details
                <div class="device-status-details">
                    <span class="device-fs-type" data-testid="device-fs-type">
                        <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                            <path d="M2 20h20v-4H2v4zm2-3h2v2H4v-2zM2 4v4h20V4H2zm4 3H4V5h2v2zm-4 7h20v-4H2v4zm2-3h2v2H4v-2z"/>
                        </svg>
                        {device.file_system.clone()}
                    </span>
                    {device.is_removable.then(|| view! {
                        <span class="device-removable" data-testid="device-removable">
                            <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                <path d="M15 7v4h1v2h-3V5h2l-3-4-3 4h2v8H8v-2.07c.7-.37 1.2-1.08 1.2-1.93 0-1.21-.99-2.2-2.2-2.2-1.21 0-2.2.99-2.2 2.2 0 .85.5 1.56 1.2 1.93V13c0 1.1.9 2 2 2h3v3.05c-.71.37-1.2 1.1-1.2 1.95 0 1.22.99 2.2 2.2 2.2 1.21 0 2.2-.98 2.2-2.2 0-.85-.49-1.58-1.2-1.95V15h3c1.1 0 2-.9 2-2v-2h1V7h-4z"/>
                            </svg>
                            "Removable"
                        </span>
                    })}
                </div>
            </div>
        </div>
    }
}

/// Disconnected status display.
#[component]
fn DeviceStatusDisconnected() -> impl IntoView {
    view! {
        <div class="device-status-content disconnected" data-testid="device-status-disconnected">
            // Disconnected indicator dot
            <div class="status-indicator-dot disconnected" data-testid="connection-indicator">
            </div>

            // Disconnected message
            <div class="device-status-info">
                <div class="device-status-header">
                    <span class="device-status-name muted">"No Device"</span>
                    <span class="device-status-badge disconnected">"Disconnected"</span>
                </div>
                <p class="device-status-hint">
                    "Connect an MP3 player or USB drive to get started"
                </p>
            </div>
        </div>
    }
}

/// Checking status display (loading state).
#[component]
fn DeviceStatusChecking() -> impl IntoView {
    view! {
        <div class="device-status-content checking" data-testid="device-status-checking">
            // Checking indicator (spinner)
            <div class="status-indicator-dot checking" data-testid="connection-indicator">
                <span class="spinner-ring"></span>
            </div>

            // Checking message
            <div class="device-status-info">
                <div class="device-status-header">
                    <span class="device-status-name muted">"Detecting..."</span>
                    <span class="device-status-badge checking">"Scanning"</span>
                </div>
                <p class="device-status-hint">
                    "Looking for connected devices..."
                </p>
            </div>
        </div>
    }
}

/// Compact version of the device status indicator for use in headers or small spaces.
#[component]

pub fn DeviceStatusIndicatorCompact(
    /// The currently selected/connected device, if any.
    device: ReadSignal<Option<DeviceInfo>>,
) -> impl IntoView {
    view! {
        <div class="device-status-indicator-compact" data-testid="device-status-indicator-compact">
            {move || {
                if let Some(dev) = device.get() {
                    view! {
                        <div class="device-status-compact connected">
                            <div class="status-dot connected"></div>
                            <span class="device-name">{dev.name}</span>
                            <span class="device-space">{format_bytes(dev.available_bytes)}" free"</span>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="device-status-compact disconnected">
                            <div class="status-dot disconnected"></div>
                            <span class="device-name muted">"No device connected"</span>
                        </div>
                    }.into_any()
                }
            }}
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
