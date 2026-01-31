//! Tauri API bindings for WASM.
//!
//! This module provides functions to call Tauri commands from the frontend.

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::types::{
    AppConfig, CapacityCheckResult, DeviceInfo, DownloadProgress, DownloadResult, FolderStatistics,
    FolderValidationResult, Mp3Metadata, PlaylistMetadata, SavedPlaylistMetadata, TaskCount,
    TaskId, TrackInfo, TransferOptions, TransferProgress, TransferResult, YouTubeUrlValidation,
};

#[wasm_bindgen]
extern "C" {
    /// The global Tauri invoke function (Tauri 2.x API).
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> Result<js_sys::Promise, JsValue>;

    /// Listen to Tauri events (Tauri 2.x API).
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    fn tauri_listen(
        event: &str,
        handler: &Closure<dyn Fn(JsValue)>,
    ) -> Result<js_sys::Promise, JsValue>;
}

/// Event names for device-related events.
pub mod device_events {
    /// Event emitted when a device is connected.
    pub const DEVICE_CONNECTED: &str = "device-connected";
    /// Event emitted when a device is disconnected.
    pub const DEVICE_DISCONNECTED: &str = "device-disconnected";
    /// Event emitted when the device list is refreshed.
    pub const DEVICES_REFRESHED: &str = "devices-refreshed";
}

/// A handle for an event listener that can be used to unlisten.
#[wasm_bindgen]
extern "C" {
    /// Unlisten function returned by `tauri_listen`.
    pub type UnlistenFn;

    #[wasm_bindgen(method, structural, js_name = "call")]
    fn call(this: &UnlistenFn);
}

/// Listen to a Tauri event.
///
/// Returns a closure that can be called to stop listening.
pub async fn listen_to_event<F>(event: &str, handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(JsValue) + 'static,
{
    if !is_tauri_available() {
        return Err("Tauri API not available".to_string());
    }

    let closure = Closure::new(handler);
    let promise = tauri_listen(event, &closure).map_err(|e| {
        e.as_string()
            .unwrap_or_else(|| "Failed to listen to event".to_string())
    })?;

    // Keep the closure alive
    closure.forget();

    let unlisten = JsFuture::from(promise).await.map_err(|e| {
        e.as_string()
            .unwrap_or_else(|| "Failed to set up event listener".to_string())
    })?;

    Ok(unlisten.unchecked_into())
}

/// Check if the Tauri API is available.
fn is_tauri_available() -> bool {
    let window = web_sys::window();
    if window.is_none() {
        return false;
    }

    let window = window.expect("window exists");
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"));

    tauri.is_ok() && !tauri.expect("tauri ok").is_undefined()
}

/// Call a Tauri command with the given arguments.
async fn invoke<T: serde::de::DeserializeOwned>(
    cmd: &str,
    args: impl serde::Serialize,
) -> Result<T, String> {
    if !is_tauri_available() {
        let msg = "Tauri API not available - are you running in a Tauri app?";
        leptos::logging::error!("{}", msg);
        return Err(msg.to_string());
    }

    let args_value = serde_wasm_bindgen::to_value(&args).map_err(|e| {
        let msg = format!("Failed to serialize args: {e}");
        leptos::logging::error!("{}", msg);
        msg
    })?;

    leptos::logging::log!("=== INVOKE {} START ===", cmd);

    let promise = tauri_invoke(cmd, args_value).map_err(|e| {
        let msg = e
            .as_string()
            .unwrap_or_else(|| "Failed to invoke Tauri command".to_string());
        leptos::logging::error!("=== INVOKE {} FAILED (tauri_invoke): {} ===", cmd, msg);
        msg
    })?;

    let result = JsFuture::from(promise).await.map_err(|e| {
        let msg = e
            .as_string()
            .unwrap_or_else(|| "Unknown error from Tauri command".to_string());
        leptos::logging::error!("=== INVOKE {} FAILED (promise): {} ===", cmd, msg);
        msg
    })?;

    leptos::logging::log!("=== INVOKE {} GOT RESULT ===", cmd);

    // Log the raw result for debugging
    if let Some(json_str) = js_sys::JSON::stringify(&result)
        .ok()
        .and_then(|s| s.as_string())
    {
        leptos::logging::log!("=== INVOKE {} RAW RESULT: {} ===", cmd, json_str);
    }

    let deserialized: T = serde_wasm_bindgen::from_value(result).map_err(|e| {
        let msg = format!("Failed to deserialize result: {e}");
        leptos::logging::error!("=== INVOKE {} FAILED (deserialize): {} ===", cmd, msg);
        msg
    })?;

    leptos::logging::log!("=== INVOKE {} SUCCESS ===", cmd);
    Ok(deserialized)
}

// =============================================================================
// Device API
// =============================================================================

/// List all detected USB devices.
pub async fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("list_devices", Args {}).await
}

/// Get information about a specific device by mount point.
///
/// Returns detailed device information including name, capacity, available space,
/// file system type, and whether the device is removable.
pub async fn get_device_info(mount_point: &str) -> Result<DeviceInfo, String> {
    #[derive(serde::Serialize)]
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
    struct Args {}

    invoke("start_device_watcher", Args {}).await
}

/// Stop watching for USB device connections/disconnections.
///
/// Returns `true` if the watcher was stopped, `false` if it wasn't running.
pub async fn stop_device_watcher() -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("stop_device_watcher", Args {}).await
}

/// Check if the device watcher is currently running.
pub async fn is_device_watcher_running() -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("is_device_watcher_running", Args {}).await
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

// =============================================================================
// Playlist API
// =============================================================================

/// List all playlists.
pub async fn list_playlists() -> Result<Vec<PlaylistMetadata>, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("list_playlists", Args {}).await
}

/// Create a new playlist.
pub async fn create_playlist(name: &str, source_url: Option<&str>) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
        source_url: Option<&'a str>,
        thumbnail_url: Option<&'a str>,
    }

    invoke(
        "create_playlist",
        Args {
            name,
            source_url,
            thumbnail_url: None,
        },
    )
    .await
}

/// Delete a playlist.
pub async fn delete_playlist(name: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("delete_playlist", Args { name }).await
}

/// Sync a playlist to a device.
pub async fn sync_playlist(playlist_name: &str, device_mount_point: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        playlist_name: &'a str,
        device_mount_point: &'a str,
    }

    invoke(
        "sync_playlist",
        Args {
            playlist_name,
            device_mount_point,
        },
    )
    .await
}

/// Get tracks for a playlist with MP3 metadata.
pub async fn get_playlist_tracks(name: &str) -> Result<Vec<TrackInfo>, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_tracks", Args { name }).await
}

/// Get tracks for a playlist without metadata extraction (faster).
pub async fn get_playlist_tracks_fast(name: &str) -> Result<Vec<TrackInfo>, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_tracks_fast", Args { name }).await
}

/// Extract MP3 metadata (ID3 tags) from a single file.
///
/// Returns metadata including title, artist, album, duration, track number, etc.
/// If the file has no tags or is not a valid MP3, returns empty metadata.
pub async fn extract_track_metadata(path: &str) -> Result<Mp3Metadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        path: &'a str,
    }

    invoke("extract_track_metadata", Args { path }).await
}

/// Get detailed metadata for a specific playlist.
///
/// Returns playlist metadata including name, source URL, creation time,
/// modification time, track count, and total size.
pub async fn get_playlist_details(name: &str) -> Result<PlaylistMetadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_details", Args { name }).await
}

/// Validate a playlist folder structure.
///
/// Checks if the folder exists, has valid metadata, and contains audio files.
/// Returns a validation result with details about any issues found.
pub async fn validate_playlist_folder(name: &str) -> Result<FolderValidationResult, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("validate_playlist_folder", Args { name }).await
}

/// Get statistics about a playlist folder.
///
/// Returns information about file counts, sizes, and metadata status.
pub async fn get_playlist_statistics(name: &str) -> Result<FolderStatistics, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_statistics", Args { name }).await
}

/// Repair a playlist folder by fixing common issues.
///
/// Currently this creates missing metadata files and fixes corrupted metadata.
/// Returns a list of repairs that were made.
pub async fn repair_playlist_folder(name: &str) -> Result<Vec<String>, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("repair_playlist_folder", Args { name }).await
}

/// Import an existing folder as a playlist.
///
/// Creates metadata for a folder that already contains audio files.
/// The folder must be in the playlists directory.
pub async fn import_playlist_folder(
    folder_name: &str,
    source_url: Option<&str>,
) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        folder_name: &'a str,
        source_url: Option<&'a str>,
    }

    invoke(
        "import_playlist_folder",
        Args {
            folder_name,
            source_url,
        },
    )
    .await
}

/// Rename a playlist.
///
/// This renames the playlist folder and updates any metadata as needed.
pub async fn rename_playlist(old_name: &str, new_name: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        old_name: &'a str,
        new_name: &'a str,
    }

    invoke("rename_playlist", Args { old_name, new_name }).await
}

/// Check if a playlist exists.
pub async fn playlist_exists(name: &str) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("playlist_exists", Args { name }).await
}

/// Ensure a playlist folder has proper structure.
///
/// Creates the metadata file if it doesn't exist.
pub async fn ensure_playlist_structure(name: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("ensure_playlist_structure", Args { name }).await
}

// =============================================================================
// Playlist Metadata Management API
// =============================================================================

/// Get the saved metadata for a playlist.
///
/// Returns the raw metadata stored in playlist.json, including title,
/// description, source URL, timestamps, track count, and total size.
pub async fn get_playlist_saved_metadata(name: &str) -> Result<SavedPlaylistMetadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_saved_metadata", Args { name }).await
}

/// Update playlist metadata.
///
/// Updates the playlist.json file with new metadata values.
/// Pass `None` for fields that should not be changed.
///
/// # Arguments
///
/// * `name` - Playlist name
/// * `title` - New title (None to keep existing, Some("") to clear)
/// * `description` - New description (None to keep existing, Some("") to clear)
/// * `source_url` - New source URL (None to keep existing, Some(None) to clear, Some(Some(url)) to set)
pub async fn update_playlist_metadata(
    name: &str,
    title: Option<&str>,
    description: Option<&str>,
    source_url: Option<Option<&str>>,
) -> Result<SavedPlaylistMetadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
        title: Option<&'a str>,
        description: Option<&'a str>,
        source_url: Option<Option<&'a str>>,
    }

    invoke(
        "update_playlist_metadata",
        Args {
            name,
            title,
            description,
            source_url,
        },
    )
    .await
}

/// Refresh the cached track count and total size for a playlist.
///
/// Scans the playlist folder and updates the `track_count` and `total_size_bytes`
/// fields in the metadata file.
pub async fn refresh_playlist_stats(name: &str) -> Result<SavedPlaylistMetadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("refresh_playlist_stats", Args { name }).await
}

// =============================================================================
// Task Management API
// =============================================================================

/// Get the status of a running task.
///
/// Returns the status as a string (e.g., "Running", "Completed", "Failed(error)", "Cancelled").
pub async fn get_task_status(task_id: TaskId) -> Result<Option<String>, String> {
    #[derive(serde::Serialize)]
    struct Args {
        task_id: TaskId,
    }

    invoke("get_task_status", Args { task_id }).await
}

/// Get all running tasks count by category.
pub async fn get_running_tasks() -> Result<Vec<TaskCount>, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    let result: Vec<(String, usize)> = invoke("get_running_tasks", Args {}).await?;
    Ok(result
        .into_iter()
        .map(|(category, count)| TaskCount { category, count })
        .collect())
}

/// Cancel a running task.
///
/// Returns `true` if the task was successfully cancelled, `false` otherwise.
pub async fn cancel_task(task_id: TaskId) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args {
        task_id: TaskId,
    }

    invoke("cancel_task", Args { task_id }).await
}

// =============================================================================
// Configuration API
// =============================================================================

/// Get the current application configuration.
pub async fn get_config() -> Result<AppConfig, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("get_config", Args {}).await
}

/// Update the application configuration.
pub async fn update_config(config: &AppConfig) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        config: &'a AppConfig,
    }

    invoke("update_config", Args { config }).await
}

/// Get the current playlists storage directory.
pub async fn get_storage_directory() -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("get_storage_directory", Args {}).await
}

/// Set the playlists storage directory.
pub async fn set_storage_directory(path: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        path: &'a str,
    }

    invoke("set_storage_directory", Args { path }).await
}

/// Get the default storage directory.
pub async fn get_default_storage_directory() -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("get_default_storage_directory", Args {}).await
}

// =============================================================================
// File Transfer API
// =============================================================================

/// Event name for transfer progress updates.
pub const TRANSFER_PROGRESS_EVENT: &str = "transfer-progress";

/// Sync a playlist to a device with progress tracking.
///
/// This enhanced sync operation provides:
/// - Chunked file transfers for better performance
/// - Progress callbacks via Tauri events
/// - Optional integrity verification
/// - Detailed transfer statistics
///
/// Subscribe to "transfer-progress" events to receive progress updates.
pub async fn sync_playlist_with_progress(
    playlist_name: &str,
    device_mount_point: &str,
    verify_integrity: bool,
    skip_existing: bool,
) -> Result<TransferResult, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        playlist_name: &'a str,
        device_mount_point: &'a str,
        verify_integrity: bool,
        skip_existing: bool,
    }

    invoke(
        "sync_playlist_with_progress",
        Args {
            playlist_name,
            device_mount_point,
            verify_integrity,
            skip_existing,
        },
    )
    .await
}

/// Get default transfer options.
pub async fn get_default_transfer_options() -> Result<TransferOptions, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("get_default_transfer_options", Args {}).await
}

/// Get fast transfer options (optimized for speed, no verification).
pub async fn get_fast_transfer_options() -> Result<TransferOptions, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("get_fast_transfer_options", Args {}).await
}

/// Get reliable transfer options (full integrity verification).
pub async fn get_reliable_transfer_options() -> Result<TransferOptions, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("get_reliable_transfer_options", Args {}).await
}

/// Transfer specific files to a device.
///
/// Subscribe to "transfer-progress" events to receive progress updates.
pub async fn transfer_files_to_device(
    source_files: Vec<String>,
    device_mount_point: &str,
    options: &TransferOptions,
) -> Result<TransferResult, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        source_files: Vec<String>,
        device_mount_point: &'a str,
        options: &'a TransferOptions,
    }

    invoke(
        "transfer_files_to_device",
        Args {
            source_files,
            device_mount_point,
            options,
        },
    )
    .await
}

/// Compute the SHA-256 checksum of a file.
pub async fn compute_file_checksum(file_path: &str) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        file_path: &'a str,
    }

    invoke("compute_file_checksum", Args { file_path }).await
}

/// Verify integrity of a transferred file by comparing checksums.
///
/// Returns `true` if source and destination have matching checksums.
pub async fn verify_file_integrity(
    source_path: &str,
    destination_path: &str,
) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        source_path: &'a str,
        destination_path: &'a str,
    }

    invoke(
        "verify_file_integrity",
        Args {
            source_path,
            destination_path,
        },
    )
    .await
}

/// Listen to transfer progress events.
///
/// Returns a function to stop listening.
pub async fn listen_to_transfer_progress<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(TransferProgress) + 'static,
{
    listen_to_event(TRANSFER_PROGRESS_EVENT, move |value| {
        if let Ok(progress) = serde_wasm_bindgen::from_value::<TransferProgress>(value) {
            handler(progress);
        }
    })
    .await
}

// =============================================================================
// Sync API
// =============================================================================

/// Event names for sync events.
pub mod sync_events {
    /// Event emitted when a sync operation starts.
    pub const SYNC_STARTED: &str = "sync-started";
    /// Event emitted for sync progress updates.
    pub const SYNC_PROGRESS: &str = "sync-progress";
    /// Event emitted when a sync operation completes successfully.
    pub const SYNC_COMPLETED: &str = "sync-completed";
    /// Event emitted when a sync operation fails.
    pub const SYNC_FAILED: &str = "sync-failed";
    /// Event emitted when a sync operation is cancelled.
    pub const SYNC_CANCELLED: &str = "sync-cancelled";
}

/// Information about an active sync task.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncTaskInfo {
    /// Task ID for this sync.
    pub task_id: TaskId,
    /// Playlist being synced.
    pub playlist_name: String,
    /// Device mount point.
    pub device_mount_point: String,
    /// Whether the sync uses integrity verification.
    pub verify_integrity: bool,
    /// Whether to skip existing files.
    pub skip_existing: bool,
}

/// Start a sync operation to transfer a playlist to a device.
///
/// This spawns a background task that transfers files with progress tracking.
/// Subscribe to sync events to receive progress updates and completion notifications.
///
/// Returns the task ID that can be used to track or cancel the sync.
pub async fn start_sync(
    playlist_name: &str,
    device_mount_point: &str,
    verify_integrity: bool,
    skip_existing: bool,
) -> Result<TaskId, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        playlist_name: &'a str,
        device_mount_point: &'a str,
        verify_integrity: bool,
        skip_existing: bool,
    }

    invoke(
        "start_sync",
        Args {
            playlist_name,
            device_mount_point,
            verify_integrity,
            skip_existing,
        },
    )
    .await
}

/// Cancel a running sync operation.
///
/// Returns `true` if the cancellation was requested successfully, `false` if
/// the sync task was not found (may have already completed).
pub async fn cancel_sync(task_id: TaskId) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args {
        task_id: TaskId,
    }

    invoke("cancel_sync", Args { task_id }).await
}

/// Get the status of a sync operation.
///
/// Returns information about the sync task, or None if the task was not found.
pub async fn get_sync_status(task_id: TaskId) -> Result<Option<SyncTaskInfo>, String> {
    #[derive(serde::Serialize)]
    struct Args {
        task_id: TaskId,
    }

    invoke("get_sync_status", Args { task_id }).await
}

/// Get all currently active sync operations.
///
/// Returns a list of all sync tasks that are currently running.
pub async fn list_active_syncs() -> Result<Vec<SyncTaskInfo>, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("list_active_syncs", Args {}).await
}

/// Listen to sync started events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_started<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(SyncTaskInfo) + 'static,
{
    listen_to_event(sync_events::SYNC_STARTED, move |value| {
        if let Ok(info) = serde_wasm_bindgen::from_value::<SyncTaskInfo>(value) {
            handler(info);
        }
    })
    .await
}

/// Sync progress payload from events.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncProgressPayload {
    /// Task ID for this sync operation.
    pub task_id: TaskId,
    /// Current status of the sync.
    pub status: String,
    /// Playlist name being synced.
    pub playlist_name: String,
    /// Device mount point.
    pub device_mount_point: String,
    /// Index of the current file being transferred (1-based).
    pub current_file_index: usize,
    /// Total number of files to transfer.
    pub total_files: usize,
    /// Name of the current file being transferred.
    pub current_file_name: String,
    /// Bytes transferred for the current file.
    pub current_file_bytes: u64,
    /// Total size of the current file in bytes.
    pub current_file_total: u64,
    /// Total bytes transferred across all files.
    pub total_bytes_transferred: u64,
    /// Total bytes to transfer across all files.
    pub total_bytes: u64,
    /// Number of files successfully transferred.
    pub files_completed: usize,
    /// Number of files skipped (already exist).
    pub files_skipped: usize,
    /// Number of files that failed to transfer.
    pub files_failed: usize,
    /// Transfer speed in bytes per second.
    pub transfer_speed_bps: f64,
    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,
    /// Elapsed time in seconds.
    pub elapsed_secs: f64,
    /// Overall progress as percentage (0.0 - 100.0).
    pub overall_progress_percent: f64,
}

/// Listen to sync progress events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_progress<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(SyncProgressPayload) + 'static,
{
    listen_to_event(sync_events::SYNC_PROGRESS, move |value| {
        if let Ok(progress) = serde_wasm_bindgen::from_value::<SyncProgressPayload>(value) {
            handler(progress);
        }
    })
    .await
}

/// Sync result payload from completion events.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncResultPayload {
    /// Task ID for this sync operation.
    pub task_id: TaskId,
    /// Whether the sync was successful.
    pub success: bool,
    /// Whether the sync was cancelled.
    pub was_cancelled: bool,
    /// Playlist name that was synced.
    pub playlist_name: String,
    /// Device mount point.
    pub device_mount_point: String,
    /// Total number of files processed.
    pub total_files: usize,
    /// Number of files successfully transferred.
    pub files_transferred: usize,
    /// Number of files skipped (already existed).
    pub files_skipped: usize,
    /// Number of files that failed to transfer.
    pub files_failed: usize,
    /// Total bytes transferred.
    pub bytes_transferred: u64,
    /// Total duration of the sync operation in seconds.
    pub duration_secs: f64,
    /// Error message if the sync failed.
    pub error_message: Option<String>,
}

/// Listen to sync completed events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_completed<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(SyncResultPayload) + 'static,
{
    listen_to_event(sync_events::SYNC_COMPLETED, move |value| {
        if let Ok(result) = serde_wasm_bindgen::from_value::<SyncResultPayload>(value) {
            handler(result);
        }
    })
    .await
}

/// Listen to sync failed events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_failed<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(SyncResultPayload) + 'static,
{
    listen_to_event(sync_events::SYNC_FAILED, move |value| {
        if let Ok(result) = serde_wasm_bindgen::from_value::<SyncResultPayload>(value) {
            handler(result);
        }
    })
    .await
}

/// Listen to sync cancelled events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_cancelled<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(SyncResultPayload) + 'static,
{
    listen_to_event(sync_events::SYNC_CANCELLED, move |value| {
        if let Ok(result) = serde_wasm_bindgen::from_value::<SyncResultPayload>(value) {
            handler(result);
        }
    })
    .await
}

// =============================================================================
// Sync Orchestrator API
// =============================================================================

/// Event names for sync orchestrator events (multi-playlist sync).
pub mod sync_orchestrator_events {
    /// Event emitted during sync orchestrator progress.
    pub const SYNC_ORCHESTRATOR_PROGRESS: &str = "sync-orchestrator-progress";
    /// Event emitted when sync orchestrator completes successfully.
    pub const SYNC_ORCHESTRATOR_COMPLETED: &str = "sync-orchestrator-completed";
    /// Event emitted when sync orchestrator fails.
    pub const SYNC_ORCHESTRATOR_FAILED: &str = "sync-orchestrator-failed";
    /// Event emitted when sync orchestrator is cancelled.
    pub const SYNC_ORCHESTRATOR_CANCELLED: &str = "sync-orchestrator-cancelled";
}

/// Phase of a sync orchestrator operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SyncOrchestratorPhase {
    /// Verifying device connection and playlists.
    Verifying,
    /// Cleaning up device before transfer.
    Cleaning,
    /// Transferring files to device.
    Transferring,
    /// Sync completed successfully.
    Completed,
    /// Sync failed.
    Failed,
    /// Sync was cancelled.
    Cancelled,
}

impl std::fmt::Display for SyncOrchestratorPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Verifying => write!(f, "Verifying"),
            Self::Cleaning => write!(f, "Cleaning"),
            Self::Transferring => write!(f, "Transferring"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Progress information for sync orchestrator operations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncOrchestratorProgress {
    /// Current phase of the sync operation.
    pub phase: String,
    /// Overall progress as a percentage (0.0 - 100.0).
    pub overall_progress_percent: f64,
    /// Progress within the current phase (0.0 - 100.0).
    pub phase_progress_percent: f64,
    /// Name of the current playlist being synced (if any).
    pub current_playlist: Option<String>,
    /// Index of the current playlist (1-based).
    pub current_playlist_index: usize,
    /// Total number of playlists to sync.
    pub total_playlists: usize,
    /// Name of the current file being transferred (if any).
    pub current_file: Option<String>,
    /// Total bytes to transfer.
    pub total_bytes: u64,
    /// Bytes transferred so far.
    pub bytes_transferred: u64,
    /// Transfer speed in bytes per second.
    pub transfer_speed_bps: f64,
    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,
    /// Elapsed time in seconds.
    pub elapsed_secs: f64,
    /// Human-readable status message.
    pub message: String,
}

impl SyncOrchestratorProgress {
    /// Format transfer speed as a human-readable string.
    #[must_use]
    pub fn formatted_speed(&self) -> String {
        let speed = self.transfer_speed_bps;
        if speed >= 1_000_000_000.0 {
            format!("{:.1} GB/s", speed / 1_000_000_000.0)
        } else if speed >= 1_000_000.0 {
            format!("{:.1} MB/s", speed / 1_000_000.0)
        } else if speed >= 1_000.0 {
            format!("{:.1} KB/s", speed / 1_000.0)
        } else {
            format!("{speed:.0} B/s")
        }
    }

    /// Format estimated remaining time as a human-readable string.
    #[must_use]
    pub fn formatted_eta(&self) -> Option<String> {
        self.estimated_remaining_secs.map(|secs| {
            if secs >= 3600.0 {
                let hours = (secs / 3600.0).floor();
                let mins = ((secs % 3600.0) / 60.0).floor();
                format!("{}:{:02}:00", hours as u32, mins as u32)
            } else if secs >= 60.0 {
                let mins = (secs / 60.0).floor();
                let s = (secs % 60.0).floor();
                format!("{}:{:02}", mins as u32, s as u32)
            } else {
                format!("0:{:02}", secs as u32)
            }
        })
    }
}

/// Result of a sync orchestrator operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncOrchestratorResult {
    /// Whether the sync was successful.
    pub success: bool,
    /// Whether the sync was cancelled.
    pub was_cancelled: bool,
    /// Final phase of the sync.
    pub final_phase: String,
    /// Total files transferred across all playlists.
    pub total_files_transferred: usize,
    /// Total files skipped.
    pub total_files_skipped: usize,
    /// Total files that failed.
    pub total_files_failed: usize,
    /// Total bytes transferred.
    pub total_bytes_transferred: u64,
    /// Duration of the sync in seconds.
    pub duration_secs: f64,
    /// Average transfer speed in bytes per second.
    pub average_speed_bps: f64,
    /// Error message if failed.
    pub error_message: Option<String>,
}

/// Start a multi-playlist sync operation using the orchestrator.
///
/// This enhanced sync supports:
/// - Multiple playlists in one operation
/// - Device cleanup before transfer
/// - Detailed progress tracking per phase
///
/// Subscribe to sync orchestrator events to receive progress updates.
pub async fn start_orchestrated_sync(
    playlists: Vec<String>,
    device_mount_point: &str,
    cleanup_enabled: bool,
    verify_integrity: bool,
    skip_existing: bool,
) -> Result<TaskId, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        playlists: Vec<String>,
        device_mount_point: &'a str,
        cleanup_enabled: bool,
        verify_integrity: bool,
        skip_existing: bool,
    }

    invoke(
        "start_orchestrated_sync",
        Args {
            playlists,
            device_mount_point,
            cleanup_enabled,
            verify_integrity,
            skip_existing,
        },
    )
    .await
}

/// Listen to sync orchestrator progress events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_orchestrator_progress<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(SyncOrchestratorProgress) + 'static,
{
    listen_to_event(
        sync_orchestrator_events::SYNC_ORCHESTRATOR_PROGRESS,
        move |value| {
            if let Ok(payload) =
                js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
                && let Ok(progress) =
                    serde_wasm_bindgen::from_value::<SyncOrchestratorProgress>(payload)
            {
                handler(progress);
            }
        },
    )
    .await
}

/// Listen to sync orchestrator completed events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_orchestrator_completed<F>(
    handler: F,
) -> Result<js_sys::Function, String>
where
    F: Fn(SyncOrchestratorResult) + 'static,
{
    listen_to_event(
        sync_orchestrator_events::SYNC_ORCHESTRATOR_COMPLETED,
        move |value| {
            if let Ok(payload) =
                js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
                && let Ok(result) =
                    serde_wasm_bindgen::from_value::<SyncOrchestratorResult>(payload)
            {
                handler(result);
            }
        },
    )
    .await
}

/// Listen to sync orchestrator failed events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_orchestrator_failed<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(SyncOrchestratorResult) + 'static,
{
    listen_to_event(
        sync_orchestrator_events::SYNC_ORCHESTRATOR_FAILED,
        move |value| {
            if let Ok(payload) =
                js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
                && let Ok(result) =
                    serde_wasm_bindgen::from_value::<SyncOrchestratorResult>(payload)
            {
                handler(result);
            }
        },
    )
    .await
}

/// Listen to sync orchestrator cancelled events.
///
/// Returns a function to stop listening.
pub async fn listen_to_sync_orchestrator_cancelled<F>(
    handler: F,
) -> Result<js_sys::Function, String>
where
    F: Fn(SyncOrchestratorResult) + 'static,
{
    listen_to_event(
        sync_orchestrator_events::SYNC_ORCHESTRATOR_CANCELLED,
        move |value| {
            if let Ok(payload) =
                js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
                && let Ok(result) =
                    serde_wasm_bindgen::from_value::<SyncOrchestratorResult>(payload)
            {
                handler(result);
            }
        },
    )
    .await
}

// =============================================================================
// YouTube URL Validation API
// =============================================================================

/// Validate a `YouTube` URL and extract playlist information.
///
/// This function validates whether a given URL is a valid `YouTube` playlist URL
/// and extracts the playlist ID if valid. It supports multiple URL formats:
///
/// - Standard playlist URLs: `https://www.youtube.com/playlist?list=PLxxxxxxxx`
/// - Watch URLs with playlist: `https://www.youtube.com/watch?v=xxx&list=PLxxxxxxxx`
/// - Short URLs with playlist: `https://youtu.be/xxx?list=PLxxxxxxxx`
///
/// Returns a `YouTubeUrlValidation` object containing validation result and details.
pub async fn validate_youtube_playlist_url(url: &str) -> Result<YouTubeUrlValidation, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        url: &'a str,
    }

    invoke("validate_youtube_playlist_url", Args { url }).await
}

/// Check if a URL is a valid `YouTube` playlist URL.
///
/// This is a simpler version that just returns true/false.
pub async fn is_valid_youtube_playlist_url(url: &str) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        url: &'a str,
    }

    invoke("is_valid_youtube_playlist_url", Args { url }).await
}

/// Extract the playlist ID from a `YouTube` URL.
///
/// Returns the playlist ID if the URL is valid, or an error message if not.
pub async fn extract_youtube_playlist_id(url: &str) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        url: &'a str,
    }

    invoke("extract_youtube_playlist_id", Args { url }).await
}

// =============================================================================
// YouTube Download API
// =============================================================================

/// Event names for `YouTube` download events.
pub mod youtube_events {
    /// Event emitted when a download starts.
    pub const DOWNLOAD_STARTED: &str = "youtube-download-started";
    /// Event emitted for download progress updates.
    pub const DOWNLOAD_PROGRESS: &str = "youtube-download-progress";
    /// Event emitted when a download completes successfully.
    pub const DOWNLOAD_COMPLETED: &str = "youtube-download-completed";
    /// Event emitted when a download fails.
    pub const DOWNLOAD_FAILED: &str = "youtube-download-failed";
    /// Event emitted when a download is cancelled.
    pub const DOWNLOAD_CANCELLED: &str = "youtube-download-cancelled";
}

/// Check if yt-dlp is available on the system.
///
/// Returns the version string if yt-dlp is found, or an error if not.
pub async fn check_yt_dlp_available() -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("check_yt_dlp_available", Args {}).await
}

/// Download a `YouTube` playlist to a local directory.
///
/// Returns the task ID that can be used to track the download.
pub async fn download_youtube_playlist(
    url: &str,
    output_dir: &str,
    audio_quality: Option<&str>,
    embed_thumbnail: Option<bool>,
) -> Result<TaskId, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        url: &'a str,
        output_dir: &'a str,
        audio_quality: Option<&'a str>,
        embed_thumbnail: Option<bool>,
    }

    invoke(
        "download_youtube_playlist",
        Args {
            url,
            output_dir,
            audio_quality,
            embed_thumbnail,
        },
    )
    .await
}

/// Download a `YouTube` playlist directly to a local playlist folder.
///
/// Returns the task ID that can be used to track the download.
pub async fn download_youtube_to_playlist(
    url: &str,
    playlist_name: &str,
) -> Result<TaskId, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        url: &'a str,
        playlist_name: &'a str,
    }

    invoke("download_youtube_to_playlist", Args { url, playlist_name }).await
}

/// Cancel a running download task.
///
/// Returns `true` if the task was successfully cancelled.
pub async fn cancel_download(task_id: TaskId) -> Result<bool, String> {
    cancel_task(task_id).await
}

/// Listen to `YouTube` download started events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_started<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(TaskId) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_STARTED, move |value| {
        if let Ok(task_id) = serde_wasm_bindgen::from_value::<TaskId>(value) {
            handler(task_id);
        }
    })
    .await
}

/// Listen to `YouTube` download progress events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_progress<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DownloadProgress) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_PROGRESS, move |value| {
        // The payload is wrapped in an event object with a "payload" field
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(progress) = serde_wasm_bindgen::from_value::<DownloadProgress>(payload)
        {
            handler(progress);
        }
    })
    .await
}

/// Listen to `YouTube` download completed events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_completed<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DownloadResult) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_COMPLETED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<DownloadResult>(payload)
        {
            handler(result);
        }
    })
    .await
}

/// Listen to `YouTube` download failed events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_failed<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DownloadResult) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_FAILED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<DownloadResult>(payload)
        {
            handler(result);
        }
    })
    .await
}

/// Listen to `YouTube` download cancelled events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_cancelled<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DownloadResult) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_CANCELLED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<DownloadResult>(payload)
        {
            handler(result);
        }
    })
    .await
}
