//! Device list component for displaying connected USB devices.

use leptos::prelude::*;

use crate::types::DeviceInfo;

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

/// Single device item component.
#[component]
fn DeviceItem(
    /// The device to display.
    device: DeviceInfo,
    /// Callback when device is selected.
    on_select: Callback<DeviceInfo>,
    /// Whether this device is selected.
    #[prop(default = false)]
    selected: bool,
) -> impl IntoView {
    let device_clone = device.clone();
    let usage = device.usage_percentage();

    view! {
        <div
            class=move || if selected { "device-item selected" } else { "device-item" }
            on:click=move |_| {
                on_select.run(device_clone.clone());
            }
        >
            <div class="device-icon">
                <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                    <path d="M15 7v4h1v2h-3V5h2l-3-4-3 4h2v8H8v-2.07c.7-.37 1.2-1.08 1.2-1.93 0-1.21-.99-2.2-2.2-2.2-1.21 0-2.2.99-2.2 2.2 0 .85.5 1.56 1.2 1.93V13c0 1.1.9 2 2 2h3v3.05c-.71.37-1.2 1.1-1.2 1.95 0 1.22.99 2.2 2.2 2.2 1.21 0 2.2-.98 2.2-2.2 0-.85-.49-1.58-1.2-1.95V15h3c1.1 0 2-.9 2-2v-2h1V7h-4z"/>
                </svg>
            </div>
            <div class="device-info">
                <div class="device-name">{device.name.clone()}</div>
                <div class="device-path">{device.mount_point.clone()}</div>
                <div class="device-storage">
                    <div class="storage-bar">
                        <div
                            class="storage-used"
                            style=format!("width: {}%", usage)
                        ></div>
                    </div>
                    <div class="storage-text">
                        {format_bytes(device.available_bytes)} " free of " {format_bytes(device.total_bytes)}
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Device list component.
#[component]
pub fn DeviceList(
    /// Signal containing list of devices.
    devices: ReadSignal<Vec<DeviceInfo>>,
    /// Signal containing the selected device.
    selected_device: ReadSignal<Option<DeviceInfo>>,
    /// Callback when a device is selected.
    on_select: Callback<DeviceInfo>,
    /// Callback to refresh the device list.
    on_refresh: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="device-list">
            <div class="device-list-header">
                <h3>"Connected Devices"</h3>
                <button
                    class="btn btn-ghost btn-icon"
                    on:click=move |_| on_refresh.run(())
                >
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                        <path d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/>
                    </svg>
                </button>
            </div>
            <div class="device-list-content">
                {move || {
                    let device_list = devices.get();
                    leptos::logging::log!("DeviceList render: {} devices", device_list.len());
                    if device_list.is_empty() {
                        view! {
                            <div class="empty-state">
                                <p>"No devices detected"</p>
                                <p class="hint">"Connect an MP3 player via USB"</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="devices">
                                {device_list.into_iter().map(|device| {
                                    leptos::logging::log!("Rendering device: {}", device.name);
                                    let is_selected = selected_device.get()
                                        .as_ref()
                                        .map(|s| s.mount_point == device.mount_point)
                                        .unwrap_or(false);
                                    view! {
                                        <DeviceItem
                                            device=device
                                            on_select=on_select
                                            selected=is_selected
                                        />
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }}
            </div>
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
