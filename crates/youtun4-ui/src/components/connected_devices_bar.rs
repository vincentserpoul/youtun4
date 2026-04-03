//! Connected devices info bar for the main content area.
//!
//! Shows device storage/battery info with scan and execute sync action buttons.

use leptos::prelude::*;

use crate::components::loading::LoadingState;
use crate::types::DeviceInfo;

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

/// Connected devices info bar displayed above the playlists table.
///
/// Shows storage and battery info for the selected device, with
/// scan and execute sync action buttons.
#[component]
#[allow(
    clippy::cast_precision_loss,
    reason = "precision loss is acceptable for display formatting"
)]
#[allow(
    clippy::float_arithmetic,
    reason = "float arithmetic needed for percentage display"
)]
pub fn ConnectedDevicesBar(
    /// The currently selected device.
    selected_device: ReadSignal<Option<DeviceInfo>>,
    /// Callback to scan/refresh devices.
    on_refresh: Callback<()>,
    /// Callback to execute sync.
    on_execute_sync: Callback<()>,
    /// Whether sync is currently running.
    syncing: ReadSignal<bool>,
    /// Device list loading state.
    state: ReadSignal<LoadingState>,
) -> impl IntoView {
    view! {
        <div class="connected-devices-bar">
            <div class="cdb-header">
                <h3 class="cdb-title">"CONNECTED DEVICES"</h3>
                <div class="cdb-actions">
                    <button
                        class=move || {
                            if state.get() == LoadingState::Loading {
                                "btn btn-scan scanning"
                            } else {
                                "btn btn-scan"
                            }
                        }
                        on:click=move |_| on_refresh.run(())
                        disabled=move || state.get() == LoadingState::Loading
                    >
                        <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor" class="scan-icon">
                            <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/>
                        </svg>
                        "SCAN FOR DEVICES"
                    </button>
                </div>
            </div>
            <div class="cdb-content">
                {move || {
                    if let Some(dev) = selected_device.get() {
                        let used = dev.used_bytes();
                        let total = dev.total_bytes;
                        let usage = dev.usage_percentage();
                        view! {
                            <div class="cdb-device-info">
                                // Connection indicator
                                <div class="cdb-status-dot connected">
                                    <span class="pulse-ring"></span>
                                </div>
                                // Device details
                                <div class="cdb-details">
                                    <div class="cdb-detail-row">
                                        <span class="cdb-detail-label">"Storage:"</span>
                                        <span class="cdb-detail-value">
                                            {format_bytes(used)} " / " {format_bytes(total)}
                                        </span>
                                    </div>
                                </div>
                                // Storage mini bar
                                <div class="cdb-storage-bar">
                                    <div
                                        class="cdb-storage-fill"
                                        style=format!("width: {}%", usage)
                                    ></div>
                                </div>
                            </div>
                            <button
                                class="btn btn-execute-sync"
                                on:click=move |_| on_execute_sync.run(())
                                disabled=move || syncing.get()
                            >
                                <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                                    <path d="M19 8l-4 4h3c0 3.31-2.69 6-6 6-1.01 0-1.97-.25-2.8-.7l-1.46 1.46C8.97 19.54 10.43 20 12 20c4.42 0 8-3.58 8-8h3l-4-4zM6 12c0-3.31 2.69-6 6-6 1.01 0 1.97.25 2.8.7l1.46-1.46C15.03 4.46 13.57 4 12 4c-4.42 0-8 3.58-8 8H1l4 4 4-4H6z"/>
                                </svg>
                                {move || {
                                    if syncing.get() {
                                        "SYNCING..."
                                    } else {
                                        "EXECUTE SYNC"
                                    }
                                }}
                            </button>
                        }.into_any()
                    } else {
                        view! {
                            <div class="cdb-no-device">
                                <span class="cdb-status-dot disconnected"></span>
                                <span class="cdb-no-device-text">"No device connected"</span>
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
