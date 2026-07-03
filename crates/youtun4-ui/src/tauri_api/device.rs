//! Device API.

use crate::types::{CapacityCheckResult, DeviceInfo};

use super::{invoke, listen_to_event};

/// Event names for device-related events.
pub mod device_events {
    /// Event emitted when a device is connected.
    pub const DEVICE_CONNECTED: &str = "device-connected";
    /// Event emitted when a device is disconnected.
    pub const DEVICE_DISCONNECTED: &str = "device-disconnected";
    /// Event emitted when the device list is refreshed.
    pub const DEVICES_REFRESHED: &str = "devices-refreshed";
}

/// List all detected USB devices.
pub async fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("list_devices", Args {}).await
}

/// Get information about a specific device by mount point.
///
/// Returns detailed device information including name, capacity, available space,
/// file system type, and whether the device is removable.
pub async fn get_device_info(mount_point: &str) -> Result<DeviceInfo, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        mount_point: &'a str,
    }

    invoke("get_device_info", Args { mount_point }).await
}

/// Check if a device is currently connected and available.
///
/// Returns `true` if the device at the specified mount point is connected,
/// mounted, and accessible; `false` otherwise.
pub async fn check_device_available(mount_point: &str) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        mount_point: &'a str,
    }

    invoke("check_device_available", Args { mount_point }).await
}

/// Verify that a device has sufficient space for a transfer.
///
/// Checks if the device at the specified mount point has at least `required_bytes`
/// of available space. Returns `Ok(true)` if space is sufficient, or an error
/// with details about available vs required space if insufficient.
pub async fn verify_device_space(mount_point: &str, required_bytes: u64) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        mount_point: &'a str,
        required_bytes: u64,
    }

    invoke(
        "verify_device_space",
        Args {
            mount_point,
            required_bytes,
        },
    )
    .await
}

/// Check if playlists can fit on a device before syncing.
///
/// This pre-flight check calculates the total size of selected playlists
/// and compares it against the available space on the device. It provides
/// detailed information about:
/// - Whether the sync can proceed (`can_fit`)
/// - Total required space
/// - Available space
/// - Usage percentage after sync
/// - Warning level (Ok, Warning, Critical)
///
/// The warning levels are:
/// - `Ok`: Plenty of space available (usage < 85% after sync)
/// - `Warning`: Space is limited (usage 85-95% after sync)
/// - `Critical`: Cannot fit or would exceed 95% usage
pub async fn check_sync_capacity(
    playlist_names: Vec<String>,
    device_mount_point: &str,
) -> Result<CapacityCheckResult, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        playlist_names: Vec<String>,
        device_mount_point: &'a str,
    }

    invoke(
        "check_sync_capacity",
        Args {
            playlist_names,
            device_mount_point,
        },
    )
    .await
}

/// Start watching for USB device connections/disconnections.
///
/// This starts a background task that polls for device changes and emits
/// events to the frontend when devices are connected or disconnected.
///
/// Returns `true` if the watcher was started, `false` if it was already running.
pub async fn start_device_watcher() -> Result<bool, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("start_device_watcher", Args {}).await
}

/// Stop watching for USB device connections/disconnections.
///
/// Returns `true` if the watcher was stopped, `false` if it wasn't running.
pub async fn stop_device_watcher() -> Result<bool, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("stop_device_watcher", Args {}).await
}

/// Check if the device watcher is currently running.
pub async fn is_device_watcher_running() -> Result<bool, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("is_device_watcher_running", Args {}).await
}

/// Eject result from eject operations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EjectResult {
    /// The mount point that was ejected.
    pub mount_point: String,
    /// Whether the eject was successful.
    pub success: bool,
}

/// Safely eject a device.
///
/// This unmounts the device and prepares it for safe removal.
/// On macOS this calls `diskutil eject`, on Windows `mountvol /d`,
/// and on Linux `eject` or `udisksctl unmount`.
///
/// Returns the eject result indicating success.
pub async fn eject_device(mount_point: &str) -> Result<EjectResult, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        mount_point: &'a str,
    }

    // The backend returns UnmountResult, which we map to our EjectResult
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BackendResult {
        mount_point: String,
        success: bool,
    }

    let result: BackendResult = invoke("eject_device", Args { mount_point }).await?;
    Ok(EjectResult {
        mount_point: result.mount_point,
        success: result.success,
    })
}

/// Listen to device connected events.
///
/// This event is emitted when a new USB device is connected.
/// The handler receives the `DeviceInfo` of the newly connected device.
///
/// Returns a function to stop listening.
pub async fn listen_to_device_connected<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DeviceInfo) + 'static,
{
    listen_to_event(device_events::DEVICE_CONNECTED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(device) = serde_wasm_bindgen::from_value::<DeviceInfo>(payload)
        {
            handler(device);
        }
    })
    .await
}

/// Listen to device disconnected events.
///
/// This event is emitted when a USB device is disconnected.
/// The handler receives the `DeviceInfo` of the disconnected device.
///
/// Returns a function to stop listening.
pub async fn listen_to_device_disconnected<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DeviceInfo) + 'static,
{
    listen_to_event(device_events::DEVICE_DISCONNECTED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(device) = serde_wasm_bindgen::from_value::<DeviceInfo>(payload)
        {
            handler(device);
        }
    })
    .await
}

/// Listen to devices refreshed events.
///
/// This event is emitted when the device list is refreshed/polled.
/// The handler receives the complete list of currently connected devices.
///
/// Returns a function to stop listening.
pub async fn listen_to_devices_refreshed<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(Vec<DeviceInfo>) + 'static,
{
    listen_to_event(device_events::DEVICES_REFRESHED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(devices) = serde_wasm_bindgen::from_value::<Vec<DeviceInfo>>(payload)
        {
            handler(devices);
        }
    })
    .await
}
