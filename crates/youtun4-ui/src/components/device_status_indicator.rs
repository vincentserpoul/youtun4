//! Device status indicator component showing connection status and device info.
//!
//! Features a circular gauge for storage visualization in the dashboard sidebar.

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
#[allow(
    clippy::cast_precision_loss,
    reason = "precision loss is acceptable for display formatting"
)]
#[allow(
    clippy::float_arithmetic,
    reason = "float arithmetic needed for byte unit conversion"
)]
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

/// Format bytes to compact human-readable string (no decimal for MB).
#[allow(
    clippy::cast_precision_loss,
    reason = "precision loss is acceptable for display formatting"
)]
#[allow(
    clippy::float_arithmetic,
    reason = "float arithmetic needed for byte unit conversion"
)]
fn format_bytes_compact(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// A visual indicator showing the connection status of a USB MP3 device.
///
/// Dashboard-style with circular gauge for storage, segmented bars for
/// storage and battery indicators, matching the neon retro aesthetic.
#[component]

pub fn DeviceStatusIndicator(
    /// The currently selected/connected device, if any.
    device: ReadSignal<Option<DeviceInfo>>,
    /// Whether the device watcher is actively checking for devices.
    #[prop(default = false)]
    is_checking: bool,
    /// Callback to scan/refresh devices.
    #[prop(optional)]
    on_refresh: Option<Callback<()>>,
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
                <h3 class="panel-title">"DEVICE MANAGEMENT"</h3>
                {on_refresh.map(|cb| view! {
                    <button
                        class="btn btn-ghost btn-icon header-action-icon"
                        title="Scan for devices"
                        on:click=move |_| cb.run(())
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                            <path d="M19 8l-4 4h3c0 3.31-2.69 6-6 6-1.01 0-1.97-.25-2.8-.7l-1.46 1.46C8.97 19.54 10.43 20 12 20c4.42 0 8-3.58 8-8h3l-4-4zM6 12c0-3.31 2.69-6 6-6 1.01 0 1.97.25 2.8.7l1.46-1.46C15.03 4.46 13.57 4 12 4c-4.42 0-8 3.58-8 8H1l4 4 4-4H6z"/>
                        </svg>
                    </button>
                })}
            </div>
            {move || {
                match connection_status() {
                    ConnectionStatus::Connected => {
                        if let Some(dev) = device.get() {
                            view! {
                                <DeviceGaugeConnected device=dev />
                            }.into_any()
                        } else {
                            view! {
                                <DeviceGaugeDisconnected />
                            }.into_any()
                        }
                    }
                    ConnectionStatus::Disconnected => {
                        view! {
                            <DeviceGaugeDisconnected />
                        }.into_any()
                    }
                    ConnectionStatus::Checking => {
                        view! {
                            <DeviceGaugeChecking />
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

/// Connected device gauge display with circular storage indicator.
#[component]
#[allow(
    clippy::cast_precision_loss,
    reason = "precision loss is acceptable for display formatting"
)]
#[allow(
    clippy::float_arithmetic,
    reason = "float arithmetic needed for gauge calculations"
)]
fn DeviceGaugeConnected(device: DeviceInfo) -> impl IntoView {
    let usage_percent = device.usage_percentage();
    // SVG circle math: radius=70, circumference=2*pi*70=439.82
    let circumference = 439.82_f64;
    let stroke_offset = circumference * (1.0 - usage_percent / 100.0);

    let used_str = format_bytes_compact(device.used_bytes());
    let total_str = format_bytes_compact(device.total_bytes);
    let usage_display = format!("{usage_percent:.1}%");

    let gauge_class = if usage_percent > 90.0 {
        "gauge-critical"
    } else if usage_percent > 75.0 {
        "gauge-warning"
    } else {
        "gauge-normal"
    };

    // Storage bar segments (10 segments)
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "truncation and sign loss are acceptable for segment count (0-10)"
    )]
    let storage_filled = ((usage_percent / 10.0).round() as usize).min(10);

    view! {
        <div class="gauge-container" data-testid="device-status-connected">
            // Circular gauge SVG
            <div class=format!("circular-gauge {gauge_class}")>
                <svg viewBox="0 0 160 160" class="gauge-svg">
                    // Background circle
                    <circle
                        cx="80" cy="80" r="70"
                        fill="none"
                        stroke="rgba(34, 211, 238, 0.1)"
                        stroke-width="8"
                    />
                    // Progress arc
                    <circle
                        cx="80" cy="80" r="70"
                        fill="none"
                        class="gauge-progress"
                        stroke-width="8"
                        stroke-linecap="round"
                        stroke-dasharray=format!("{circumference}")
                        stroke-dashoffset=format!("{stroke_offset}")
                        transform="rotate(-90 80 80)"
                    />
                    // Outer glow ring
                    <circle
                        cx="80" cy="80" r="75"
                        fill="none"
                        class="gauge-glow-ring"
                        stroke-width="1"
                    />
                </svg>
                // Center text overlay
                <div class="gauge-center-text">
                    <span class="gauge-value">{used_str}</span>
                    <span class="gauge-label">{total_str}</span>
                    <span class="gauge-percent">{usage_display}</span>
                </div>
                // Connection status dot
                <div class="gauge-status-dot connected">
                    <span class="pulse-ring"></span>
                </div>
            </div>

            // Segmented storage bar
            <div class="gauge-bars">
                <div class="gauge-bar-row">
                    <span class="gauge-bar-label">"STORAGE"</span>
                    <div class="segmented-bar">
                        {(0..10).map(|i| {
                            let active = i < storage_filled;
                            let class_name = if active { "segment active" } else { "segment" };
                            view! { <div class=class_name></div> }
                        }).collect_view()}
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Disconnected gauge display.
#[component]
fn DeviceGaugeDisconnected() -> impl IntoView {
    view! {
        <div class="gauge-container disconnected" data-testid="device-status-disconnected">
            <div class="circular-gauge gauge-empty">
                <svg viewBox="0 0 160 160" class="gauge-svg">
                    <circle
                        cx="80" cy="80" r="70"
                        fill="none"
                        stroke="rgba(34, 211, 238, 0.06)"
                        stroke-width="8"
                    />
                    <circle
                        cx="80" cy="80" r="75"
                        fill="none"
                        stroke="rgba(34, 211, 238, 0.04)"
                        stroke-width="1"
                    />
                </svg>
                <div class="gauge-center-text">
                    <span class="gauge-value muted">"0.0 MB"</span>
                    <span class="gauge-label muted">"No Device"</span>
                </div>
                <div class="gauge-status-dot disconnected"></div>
            </div>

            <div class="gauge-bars">
                <div class="gauge-bar-row">
                    <span class="gauge-bar-label">"STORAGE"</span>
                    <div class="segmented-bar">
                        {(0..10).map(|_| {
                            view! { <div class="segment"></div> }
                        }).collect_view()}
                    </div>
                </div>
            </div>

            <p class="gauge-hint">"Plug in your MP3 player via USB"</p>
        </div>
    }
}

/// Checking gauge display (loading state).
#[component]
fn DeviceGaugeChecking() -> impl IntoView {
    view! {
        <div class="gauge-container checking" data-testid="device-status-checking">
            <div class="circular-gauge gauge-scanning">
                <svg viewBox="0 0 160 160" class="gauge-svg">
                    <circle
                        cx="80" cy="80" r="70"
                        fill="none"
                        stroke="rgba(34, 211, 238, 0.06)"
                        stroke-width="8"
                    />
                    <circle
                        cx="80" cy="80" r="70"
                        fill="none"
                        class="gauge-scanning-arc"
                        stroke-width="8"
                        stroke-linecap="round"
                        stroke-dasharray="80 360"
                        transform="rotate(-90 80 80)"
                    />
                </svg>
                <div class="gauge-center-text">
                    <span class="gauge-value muted">"Scanning..."</span>
                </div>
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

    #[test]
    fn test_format_bytes_compact() {
        assert_eq!(format_bytes_compact(500), "500 B");
        assert_eq!(format_bytes_compact(1024), "1 KB");
        assert_eq!(format_bytes_compact(1_048_576), "1.0 MB");
    }
}
