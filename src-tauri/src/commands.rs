//! Tauri commands for the `MP3YouTube` application.
//!
//! These commands are invoked from the frontend via Tauri's IPC mechanism.

#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::option_option)]
#![allow(clippy::similar_names)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::if_same_then_else)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use mp3youtube_core::{
    AppConfig, Error, ErrorKind, Result,
    cleanup::{CleanupOptions, CleanupResult, DeviceCleanupHandler},
    config::ConfigManager,
    device::{
        DeviceDetector, DeviceEvent, DeviceInfo, DeviceManager, DeviceMountHandler, DeviceWatcher,
        DeviceWatcherHandle, MountResult, MountStatus, PlatformMountHandler, UnmountResult,
    },
    integrity::{
        ChecksumManifest, FileChecksum, IntegrityVerifier, VerificationOptions,
        VerificationProgress, VerificationResult,
    },
    metadata::{Mp3Metadata, extract_metadata},
    playlist::{PlaylistManager, PlaylistMetadata, SavedPlaylistMetadata, TrackInfo},
    queue::{
        DownloadPriority, DownloadQueueManager, DownloadRequest, QueueConfig, QueueItem,
        QueueItemId, QueueStats,
    },
    sync::{
        SyncOptions, SyncOrchestrator, SyncProgress, SyncRequest, SyncResult as CoreSyncResult,
    },
    transfer::{TransferOptions, TransferProgress, TransferResult},
    youtube::{
        DownloadProgress, DownloadStatus, PlaylistInfo, RustyYtdlConfig, RustyYtdlDownloader,
        YouTubeDownloader, YouTubeUrlValidation, validate_youtube_url,
    },
};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::runtime::{AsyncRuntime, ProgressSender, TaskCategory, TaskId, TaskStatus};

/// Information about an active sync operation.
#[derive(Debug, Clone, serde::Serialize)]
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

/// Application state managed by Tauri.
pub struct AppState {
    /// Configuration manager (async-safe).
    config_manager: Arc<RwLock<ConfigManager>>,
    /// Device manager for detecting USB devices (async-safe).
    device_manager: Arc<RwLock<DeviceManager>>,
    /// Playlist manager for local playlist operations (async-safe).
    playlist_manager: Arc<RwLock<PlaylistManager>>,
    /// Async runtime for spawning and managing tasks.
    runtime: Arc<AsyncRuntime>,
    /// Handle for the device watcher (if running).
    device_watcher_handle: Arc<RwLock<Option<DeviceWatcherHandle>>>,
    /// Mount handler for device mount/unmount operations.
    mount_handler: Arc<PlatformMountHandler>,
    /// Active sync tasks with their cancellation tokens.
    sync_tasks: Arc<RwLock<HashMap<TaskId, (SyncTaskInfo, Arc<AtomicBool>)>>>,
    /// Download queue manager for handling multiple playlist downloads.
    download_queue: Arc<DownloadQueueManager>,
}

impl AppState {
    /// Create a new application state using configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the config, playlist manager, or async runtime cannot be created.
    pub fn new() -> Result<Self> {
        let config_manager = ConfigManager::new()?;
        let playlists_dir = config_manager.playlists_directory().to_path_buf();
        let queue_config = config_manager.config().queue.clone();

        info!(
            "Playlists directory from config: {}",
            playlists_dir.display()
        );

        let runtime = AsyncRuntime::new().map_err(|e| {
            error!("Failed to create async runtime: {}", e);
            Error::Configuration(format!("Failed to create async runtime: {e}"))
        })?;

        info!("Async runtime initialized successfully");

        let download_queue = DownloadQueueManager::with_config(queue_config);
        info!("Download queue manager initialized");

        Ok(Self {
            config_manager: Arc::new(RwLock::new(config_manager)),
            device_manager: Arc::new(RwLock::new(DeviceManager::new())),
            playlist_manager: Arc::new(RwLock::new(PlaylistManager::new(playlists_dir)?)),
            runtime: Arc::new(runtime),
            device_watcher_handle: Arc::new(RwLock::new(None)),
            mount_handler: Arc::new(PlatformMountHandler::new()),
            sync_tasks: Arc::new(RwLock::new(HashMap::new())),
            download_queue: Arc::new(download_queue),
        })
    }

    /// Reinitialize the playlist manager with a new directory.
    async fn reinitialize_playlist_manager(&self, playlists_dir: PathBuf) -> Result<()> {
        let new_manager = PlaylistManager::new(playlists_dir)?;
        let mut manager = self.playlist_manager.write().await;
        *manager = new_manager;
        Ok(())
    }

    /// Get a reference to the async runtime.
    pub fn runtime(&self) -> &AsyncRuntime {
        &self.runtime
    }

    /// Get a progress sender for reporting task progress.
    #[allow(dead_code)]
    pub fn progress_sender(&self) -> ProgressSender {
        self.runtime.progress_sender()
    }

    /// Spawn an async task on the runtime.
    #[allow(dead_code)]
    pub fn spawn_task<F, T>(
        &self,
        category: TaskCategory,
        description: Option<String>,
        future: F,
    ) -> TaskId
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.runtime.spawn(category, description, future)
    }

    /// Get the status of a task.
    pub async fn task_status(&self, task_id: TaskId) -> Option<TaskStatus> {
        self.runtime.task_status(task_id).await
    }

    /// Get a clone of the device manager Arc for use in device watching.
    pub fn device_manager_arc(&self) -> Arc<RwLock<DeviceManager>> {
        Arc::clone(&self.device_manager)
    }

    /// Get a clone of the device watcher handle Arc.
    pub fn device_watcher_handle_arc(&self) -> Arc<RwLock<Option<DeviceWatcherHandle>>> {
        Arc::clone(&self.device_watcher_handle)
    }

    /// Register a sync task with its cancellation token.
    pub async fn register_sync_task(
        &self,
        task_id: TaskId,
        info: SyncTaskInfo,
        cancel_token: Arc<AtomicBool>,
    ) {
        let mut tasks = self.sync_tasks.write().await;
        tasks.insert(task_id, (info, cancel_token));
    }

    /// Unregister a sync task.
    #[allow(dead_code)]
    pub async fn unregister_sync_task(&self, task_id: TaskId) {
        let mut tasks = self.sync_tasks.write().await;
        tasks.remove(&task_id);
    }

    /// Get info about a sync task.
    pub async fn get_sync_task_info(&self, task_id: TaskId) -> Option<SyncTaskInfo> {
        let tasks = self.sync_tasks.read().await;
        tasks.get(&task_id).map(|(info, _)| info.clone())
    }

    /// Cancel a sync task by task ID.
    pub async fn cancel_sync_task(&self, task_id: TaskId) -> bool {
        let tasks = self.sync_tasks.read().await;
        if let Some((_, cancel_token)) = tasks.get(&task_id) {
            cancel_token.store(true, Ordering::SeqCst);
            info!("Sync task {} cancellation requested", task_id);
            true
        } else {
            debug!("Sync task {} not found for cancellation", task_id);
            false
        }
    }

    /// Get all active sync tasks.
    pub async fn list_sync_tasks(&self) -> Vec<SyncTaskInfo> {
        let tasks = self.sync_tasks.read().await;
        tasks.values().map(|(info, _)| info.clone()).collect()
    }

    /// Get a clone of the playlist manager Arc for async operations.
    pub fn playlist_manager_arc(&self) -> Arc<RwLock<PlaylistManager>> {
        Arc::clone(&self.playlist_manager)
    }

    /// Get a clone of the download queue manager Arc.
    pub fn download_queue_arc(&self) -> Arc<DownloadQueueManager> {
        Arc::clone(&self.download_queue)
    }
}

/// Structured error response for Tauri IPC.
///
/// Includes both the error message and the error kind for frontend handling.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorResponse {
    /// Human-readable error message.
    pub message: String,
    /// Error category for programmatic handling.
    pub kind: String,
    /// Whether the error can be retried.
    pub retryable: bool,
    /// Suggested retry delay in seconds, if applicable.
    pub retry_delay_secs: Option<u64>,
}

impl From<&Error> for ErrorResponse {
    fn from(e: &Error) -> Self {
        Self {
            message: e.to_string(),
            kind: format!("{:?}", e.kind()),
            retryable: e.is_retryable(),
            retry_delay_secs: e.retry_delay_secs(),
        }
    }
}

/// Convert our error type to a string for Tauri.
///
/// The returned string is JSON-encoded `ErrorResponse` for structured error handling
/// in the frontend. Falls back to plain error message if serialization fails.
fn map_err(e: Error) -> String {
    let kind = e.kind();
    let is_retryable = e.is_retryable();

    error!(
        "Command error [kind={:?}, retryable={}]: {}",
        kind, is_retryable, e
    );

    // Try to return structured JSON for the frontend
    let response = ErrorResponse::from(&e);
    serde_json::to_string(&response).unwrap_or_else(|_| e.to_string())
}

/// Get the error kind from an error, useful for testing.
#[allow(dead_code)]
fn error_kind(e: &Error) -> ErrorKind {
    e.kind()
}

// =============================================================================
// Device API Commands
// =============================================================================

/// List all detected devices.
#[tauri::command]
pub async fn list_devices(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<DeviceInfo>, String> {
    info!("=== LIST_DEVICES command called ===");

    let mut manager = state.device_manager.write().await;
    manager.refresh();
    let devices = manager.list_devices().map_err(map_err)?;

    info!("=== LIST_DEVICES returning {} devices ===", devices.len());
    for (i, d) in devices.iter().enumerate() {
        info!(
            "  Device {}: name='{}' mount='{}'",
            i,
            d.name,
            d.mount_point.display()
        );
    }

    Ok(devices)
}

/// Get information about a specific device by mount point.
///
/// Returns detailed device information including name, capacity, available space,
/// file system type, and whether the device is removable.
#[tauri::command]
pub async fn get_device_info(
    state: State<'_, AppState>,
    mount_point: String,
) -> std::result::Result<DeviceInfo, String> {
    debug!("Getting device info for: {}", mount_point);

    let mut manager = state.device_manager.write().await;
    manager.refresh();

    let device =
        mp3youtube_core::device::get_device_by_mount_point(&*manager, &PathBuf::from(&mount_point))
            .map_err(map_err)?;

    info!(
        "Retrieved device info: {} at {} ({} bytes available)",
        device.name,
        device.mount_point.display(),
        device.available_bytes
    );
    Ok(device)
}

/// Check if a device is currently connected and available.
///
/// Returns `true` if the device at the specified mount point is connected,
/// mounted, and accessible; `false` otherwise.
#[tauri::command]
pub async fn check_device_available(
    state: State<'_, AppState>,
    mount_point: String,
) -> std::result::Result<bool, String> {
    debug!("Checking device availability: {}", mount_point);

    let mut manager = state.device_manager.write().await;
    manager.refresh();

    let path = PathBuf::from(&mount_point);
    let is_connected = manager.is_device_connected(&path);

    // Also verify the mount point directory exists and is accessible
    let is_accessible = path.exists() && path.is_dir();

    let available = is_connected && is_accessible;
    info!(
        "Device availability for {}: connected={}, accessible={}, available={}",
        mount_point, is_connected, is_accessible, available
    );

    Ok(available)
}

/// Verify that a device has sufficient space for a transfer.
///
/// Checks if the device at the specified mount point has at least `required_bytes`
/// of available space. Returns `Ok(true)` if space is sufficient, or an error
/// with details about available vs required space if insufficient.
#[tauri::command]
pub async fn verify_device_space(
    state: State<'_, AppState>,
    mount_point: String,
    required_bytes: u64,
) -> std::result::Result<bool, String> {
    debug!(
        "Verifying space for device: {} (required: {} bytes)",
        mount_point, required_bytes
    );

    let mut manager = state.device_manager.write().await;
    manager.refresh();

    let device =
        mp3youtube_core::device::get_device_by_mount_point(&*manager, &PathBuf::from(&mount_point))
            .map_err(map_err)?;

    // Check if space is sufficient - this returns an error if insufficient
    mp3youtube_core::device::check_device_space(&device, required_bytes).map_err(map_err)?;

    info!(
        "Device {} has sufficient space: {} bytes available, {} bytes required",
        device.name, device.available_bytes, required_bytes
    );
    Ok(true)
}

/// Warning level for capacity checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum CapacityWarningLevel {
    /// Sufficient space available.
    #[default]
    Ok,
    /// Space is limited (usage will be high after sync).
    Warning,
    /// Insufficient space for sync.
    Critical,
}

/// Result of checking device capacity for a sync operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CapacityCheckResult {
    /// Whether the playlist(s) can fit on the device.
    pub can_fit: bool,
    /// Total bytes required for the sync operation.
    pub required_bytes: u64,
    /// Available bytes on the device.
    pub available_bytes: u64,
    /// Total device capacity in bytes.
    pub total_bytes: u64,
    /// Device usage percentage after sync (0.0 - 100.0).
    pub usage_after_sync_percent: f64,
    /// Warning level based on available space.
    pub warning_level: CapacityWarningLevel,
    /// Human-readable message about the capacity status.
    pub message: String,
}

/// Check if playlists can fit on a device before syncing.
///
/// This pre-flight check calculates the total size of selected playlists
/// and compares it against the available space on the device. It provides
/// detailed information about:
/// - Whether the sync can proceed
/// - Total required space
/// - Available space
/// - Usage percentage after sync
/// - Warning level (Ok, Warning, Critical)
///
/// The warning levels are:
/// - `Ok`: Plenty of space available (usage < 85% after sync)
/// - `Warning`: Space is limited (usage 85-95% after sync)
/// - `Critical`: Cannot fit or would exceed 95% usage
#[tauri::command]
pub async fn check_sync_capacity(
    state: State<'_, AppState>,
    playlist_names: Vec<String>,
    device_mount_point: String,
) -> std::result::Result<CapacityCheckResult, String> {
    debug!(
        "Checking sync capacity for {} playlists to device: {}",
        playlist_names.len(),
        device_mount_point
    );

    // Get device info
    let mut manager = state.device_manager.write().await;
    manager.refresh();
    let device = mp3youtube_core::device::get_device_by_mount_point(
        &*manager,
        &PathBuf::from(&device_mount_point),
    )
    .map_err(map_err)?;
    drop(manager);

    // Calculate total required bytes from all playlists
    let playlist_manager = state.playlist_manager.read().await;
    let mut total_required: u64 = 0;

    for playlist_name in &playlist_names {
        match playlist_manager.get_folder_statistics(playlist_name) {
            Ok(stats) => {
                total_required += stats.audio_size_bytes;
                debug!(
                    "Playlist '{}' size: {} bytes",
                    playlist_name, stats.audio_size_bytes
                );
            }
            Err(e) => {
                error!(
                    "Failed to get stats for playlist '{}': {}",
                    playlist_name, e
                );
                return Err(map_err(e));
            }
        }
    }
    drop(playlist_manager);

    // Calculate capacity metrics
    let can_fit = device.available_bytes >= total_required;
    let used_after_sync = device.used_bytes() + total_required;
    let usage_after_sync_percent = if device.total_bytes > 0 {
        (used_after_sync as f64 / device.total_bytes as f64) * 100.0
    } else {
        100.0
    };

    // Determine warning level
    // Critical: cannot fit OR usage > 95%
    // Warning: usage > 85%
    // Ok: otherwise
    let warning_level = if !can_fit || usage_after_sync_percent > 95.0 {
        CapacityWarningLevel::Critical
    } else if usage_after_sync_percent > 85.0 {
        CapacityWarningLevel::Warning
    } else {
        CapacityWarningLevel::Ok
    };

    // Generate human-readable message
    let message = if !can_fit {
        let deficit = total_required - device.available_bytes;
        format!(
            "Insufficient space: need {} more on {}",
            format_bytes(deficit),
            device.name
        )
    } else if warning_level == CapacityWarningLevel::Warning {
        format!(
            "Limited space: {} will be {:.0}% full after sync",
            device.name, usage_after_sync_percent
        )
    } else {
        format!(
            "Ready to sync: {} available on {}",
            format_bytes(device.available_bytes - total_required),
            device.name
        )
    };

    let result = CapacityCheckResult {
        can_fit,
        required_bytes: total_required,
        available_bytes: device.available_bytes,
        total_bytes: device.total_bytes,
        usage_after_sync_percent,
        warning_level,
        message,
    };

    info!(
        "Capacity check result: can_fit={}, required={} bytes, available={} bytes, usage_after={:.1}%, level={:?}",
        result.can_fit,
        result.required_bytes,
        result.available_bytes,
        result.usage_after_sync_percent,
        result.warning_level
    );

    Ok(result)
}

/// Format bytes as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.2} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} bytes")
    }
}

// =============================================================================
// Device Watching Commands
// =============================================================================

/// Event names for device events emitted to the frontend.
pub mod device_events {
    /// Event emitted when a device is connected.
    pub const DEVICE_CONNECTED: &str = "device-connected";
    /// Event emitted when a device is disconnected.
    pub const DEVICE_DISCONNECTED: &str = "device-disconnected";
    /// Event emitted when the device list is refreshed.
    pub const DEVICES_REFRESHED: &str = "devices-refreshed";
}

/// Event names for sync events emitted to the frontend.
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

/// Start watching for USB device connections/disconnections.
///
/// This will start a background task that polls for device changes and emits
/// events to the frontend when devices are connected or disconnected.
///
/// Returns `true` if the watcher was started, `false` if it was already running.
#[tauri::command]
pub async fn start_device_watcher(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<bool, String> {
    info!("Starting device watcher");

    // Check if watcher is already running
    {
        let handle = state.device_watcher_handle_arc();
        let existing = handle.read().await;
        if existing.is_some() {
            debug!("Device watcher already running");
            return Ok(false);
        }
    }

    // Create and start the device watcher
    let device_manager = state.device_manager_arc();
    let watcher = DeviceWatcher::new(device_manager);
    let (mut event_rx, watcher_handle) = watcher.start();

    // Store the handle
    {
        let handle_arc = state.device_watcher_handle_arc();
        let mut handle = handle_arc.write().await;
        *handle = Some(watcher_handle);
    }

    // Spawn a task to forward events to the frontend
    let app_handle = app.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match &event {
                DeviceEvent::Connected(device) => {
                    info!("Emitting device-connected event: {}", device.name);
                    if let Err(e) = app_handle.emit(device_events::DEVICE_CONNECTED, device) {
                        error!("Failed to emit device-connected event: {}", e);
                    }
                }
                DeviceEvent::Disconnected(device) => {
                    info!("Emitting device-disconnected event: {}", device.name);
                    if let Err(e) = app_handle.emit(device_events::DEVICE_DISCONNECTED, device) {
                        error!("Failed to emit device-disconnected event: {}", e);
                    }
                }
                DeviceEvent::Refreshed(devices) => {
                    info!(
                        "Emitting devices-refreshed event: {} devices",
                        devices.len()
                    );
                    if let Err(e) = app_handle.emit(device_events::DEVICES_REFRESHED, devices) {
                        error!("Failed to emit devices-refreshed event: {}", e);
                    }
                }
            }
        }
        debug!("Device event forwarding task ended");
    });

    info!("Device watcher started successfully");
    Ok(true)
}

/// Stop watching for USB device connections/disconnections.
///
/// Returns `true` if the watcher was stopped, `false` if it wasn't running.
#[tauri::command]
pub async fn stop_device_watcher(state: State<'_, AppState>) -> std::result::Result<bool, String> {
    info!("Stopping device watcher");

    let handle_arc = state.device_watcher_handle_arc();
    let mut handle = handle_arc.write().await;

    if let Some(watcher_handle) = handle.take() {
        watcher_handle.stop().await;
        info!("Device watcher stopped successfully");
        Ok(true)
    } else {
        debug!("Device watcher was not running");
        Ok(false)
    }
}

/// Check if the device watcher is currently running.
#[tauri::command]
pub async fn is_device_watcher_running(
    state: State<'_, AppState>,
) -> std::result::Result<bool, String> {
    let handle_arc = state.device_watcher_handle_arc();
    let handle = handle_arc.read().await;
    Ok(handle.is_some())
}

// =============================================================================
// Device Mount/Unmount Commands
// =============================================================================

/// Get the mount status of a device.
///
/// Returns information about whether a device is mounted, its mount point,
/// accessibility, and read-only status.
#[tauri::command]
pub async fn get_mount_status(
    state: State<'_, AppState>,
    device_path: String,
) -> std::result::Result<MountStatus, String> {
    debug!("Getting mount status for: {}", device_path);

    let path = PathBuf::from(&device_path);
    let status = state
        .mount_handler
        .get_mount_status(&path)
        .map_err(map_err)?;

    info!(
        "Mount status for {}: mounted={}, accessible={}, read_only={}",
        device_path, status.is_mounted, status.is_accessible, status.is_read_only
    );
    Ok(status)
}

/// Mount a device.
///
/// Attempts to mount a device at the specified path. On most platforms,
/// USB devices are auto-mounted, but this can be used for explicit mounting.
/// If `mount_point` is provided, attempts to mount at that location.
#[tauri::command]
pub async fn mount_device(
    state: State<'_, AppState>,
    device_path: String,
    mount_point: Option<String>,
) -> std::result::Result<MountResult, String> {
    info!("Mounting device: {}", device_path);

    let path = PathBuf::from(&device_path);

    let result = if let Some(mp) = mount_point {
        let mount_pt = PathBuf::from(&mp);
        state
            .mount_handler
            .mount_device_at(&path, &mount_pt)
            .map_err(map_err)?
    } else {
        state
            .mount_handler
            .mount_device_auto(&path)
            .map_err(map_err)?
    };

    info!(
        "Device {} mounted at {:?}",
        result.device_name, result.mount_point
    );
    Ok(result)
}

/// Unmount a device.
///
/// Safely unmounts a device from the specified mount point.
/// If `force` is true, will attempt to force unmount even if the device is busy.
#[tauri::command]
pub async fn unmount_device(
    state: State<'_, AppState>,
    mount_point: String,
    force: bool,
) -> std::result::Result<UnmountResult, String> {
    info!("Unmounting device at: {} (force={})", mount_point, force);

    let path = PathBuf::from(&mount_point);
    let result = state
        .mount_handler
        .unmount_device(&path, force)
        .map_err(map_err)?;

    info!("Device unmounted from {:?}", result.mount_point);
    Ok(result)
}

/// Eject a device.
///
/// Safely ejects a device, unmounting it and preparing it for physical removal.
/// This is the recommended way to remove USB devices.
#[tauri::command]
pub async fn eject_device(
    state: State<'_, AppState>,
    mount_point: String,
) -> std::result::Result<UnmountResult, String> {
    info!("Ejecting device at: {}", mount_point);

    let path = PathBuf::from(&mount_point);
    let result = state.mount_handler.eject_device(&path).map_err(map_err)?;

    info!("Device ejected from {:?}", result.mount_point);
    Ok(result)
}

/// Check if a mount point is accessible.
///
/// Returns true if the mount point exists, is a directory, and is readable.
#[tauri::command]
pub async fn is_mount_point_accessible(
    state: State<'_, AppState>,
    mount_point: String,
) -> std::result::Result<bool, String> {
    debug!("Checking accessibility of mount point: {}", mount_point);

    let path = PathBuf::from(&mount_point);
    let accessible = state.mount_handler.is_mount_point_accessible(&path);

    info!("Mount point {} accessible: {}", mount_point, accessible);
    Ok(accessible)
}

/// Get the current platform identifier.
///
/// Returns the platform that the mount handler is running on (e.g., "macos", "linux", "windows").
#[tauri::command]
pub async fn get_mount_handler_platform(
    state: State<'_, AppState>,
) -> std::result::Result<String, String> {
    Ok(state.mount_handler.platform().to_string())
}

// =============================================================================
// Device Cleanup Commands
// =============================================================================

/// Preview what would be deleted from a device.
///
/// Performs a dry run of the cleanup operation and returns information about
/// what files would be deleted without actually deleting anything.
#[tauri::command]
pub async fn preview_device_cleanup(
    mount_point: String,
    skip_hidden: bool,
    skip_system_files: bool,
    protected_patterns: Vec<String>,
) -> std::result::Result<CleanupResult, String> {
    info!("Previewing cleanup for device: {}", mount_point);

    let handler = DeviceCleanupHandler::new();
    let mut options = CleanupOptions::default();
    options.skip_hidden = skip_hidden;
    options.skip_system_files = skip_system_files;
    options.protected_patterns = protected_patterns;
    options.dry_run = true;

    let path = PathBuf::from(&mount_point);
    let result = handler.preview_cleanup(&path, &options).map_err(map_err)?;

    info!(
        "Preview complete: {} files, {} directories would be deleted ({} bytes)",
        result.files_deleted, result.directories_deleted, result.bytes_freed
    );

    Ok(result)
}

/// Clean up (delete) all non-protected files from a device.
///
/// This safely removes all files from the device except hidden files,
/// system files, and any files matching protected patterns.
/// Use `preview_device_cleanup` first to see what would be deleted.
#[tauri::command]
pub async fn cleanup_device(
    mount_point: String,
    skip_hidden: bool,
    skip_system_files: bool,
    protected_patterns: Vec<String>,
    verify_deletions: bool,
) -> std::result::Result<CleanupResult, String> {
    info!("Starting cleanup for device: {}", mount_point);

    let handler = DeviceCleanupHandler::new();
    let mut options = CleanupOptions::default();
    options.skip_hidden = skip_hidden;
    options.skip_system_files = skip_system_files;
    options.protected_patterns = protected_patterns;
    options.verify_deletions = verify_deletions;
    options.dry_run = false;

    let path = PathBuf::from(&mount_point);
    let result = handler.cleanup_device(&path, &options).map_err(map_err)?;

    info!(
        "Cleanup complete: {} files, {} directories deleted ({} bytes freed, {} failed)",
        result.files_deleted, result.directories_deleted, result.bytes_freed, result.files_failed
    );

    Ok(result)
}

/// Clean up only audio files from a device.
///
/// This removes only audio files (mp3, m4a, wav, flac, ogg, aac) from the device,
/// leaving all other files intact. Useful for refreshing audio content.
#[tauri::command]
pub async fn cleanup_device_audio_only(
    mount_point: String,
    skip_hidden: bool,
    verify_deletions: bool,
) -> std::result::Result<CleanupResult, String> {
    info!("Starting audio-only cleanup for device: {}", mount_point);

    let handler = DeviceCleanupHandler::new();
    let mut options = CleanupOptions::default();
    options.skip_hidden = skip_hidden;
    options.skip_system_files = true;
    options.verify_deletions = verify_deletions;
    options.dry_run = false;

    let path = PathBuf::from(&mount_point);
    let result = handler
        .cleanup_audio_files_only(&path, &options)
        .map_err(map_err)?;

    info!(
        "Audio cleanup complete: {} files deleted ({} bytes freed)",
        result.files_deleted, result.bytes_freed
    );

    Ok(result)
}

/// Perform a verified cleanup with device connection check.
///
/// This method first verifies that the device is still connected,
/// then performs the cleanup. Useful for ensuring the device hasn't
/// been disconnected before cleanup.
#[tauri::command]
pub async fn cleanup_device_verified(
    state: State<'_, AppState>,
    mount_point: String,
    skip_hidden: bool,
    skip_system_files: bool,
    protected_patterns: Vec<String>,
    verify_deletions: bool,
) -> std::result::Result<CleanupResult, String> {
    info!("Starting verified cleanup for device: {}", mount_point);

    // Get device info first
    let mut manager = state.device_manager.write().await;
    manager.refresh();

    let path = PathBuf::from(&mount_point);
    let device =
        mp3youtube_core::device::get_device_by_mount_point(&*manager, &path).map_err(map_err)?;

    // Verify device is accessible
    mp3youtube_core::device::verify_device_accessible(&*manager, &device).map_err(map_err)?;

    // Now perform cleanup
    let handler = DeviceCleanupHandler::new();
    let mut options = CleanupOptions::default();
    options.skip_hidden = skip_hidden;
    options.skip_system_files = skip_system_files;
    options.protected_patterns = protected_patterns;
    options.verify_deletions = verify_deletions;
    options.dry_run = false;

    let result = handler
        .cleanup_device_verified(&*manager, &device, &options)
        .map_err(map_err)?;

    info!(
        "Verified cleanup complete: {} files, {} directories deleted ({} bytes freed)",
        result.files_deleted, result.directories_deleted, result.bytes_freed
    );

    Ok(result)
}

/// List all playlists.
#[tauri::command]
pub async fn list_playlists(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<PlaylistMetadata>, String> {
    debug!("Listing playlists");
    let manager = state.playlist_manager.read().await;
    manager.list_playlists().map_err(map_err)
}

/// Create a new playlist.
#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    source_url: Option<String>,
    thumbnail_url: Option<String>,
) -> std::result::Result<String, String> {
    info!("Creating playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    let path = manager
        .create_playlist(&name, source_url)
        .map_err(map_err)?;

    // If thumbnail_url is provided, update the metadata with it
    if let Some(thumb) = thumbnail_url {
        manager
            .update_playlist_metadata_full(&name, None, None, None, Some(Some(thumb)))
            .map_err(map_err)?;
    }

    Ok(path.display().to_string())
}

/// Delete a playlist.
#[tauri::command]
pub async fn delete_playlist(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    info!("Deleting playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.delete_playlist(&name).map_err(map_err)
}

/// Sync a playlist to a device.
///
/// This operation is performed asynchronously and returns a task ID
/// that can be used to track progress.
#[tauri::command]
pub async fn sync_playlist(
    state: State<'_, AppState>,
    playlist_name: String,
    device_mount_point: String,
) -> std::result::Result<(), String> {
    info!(
        "Syncing playlist '{}' to device at '{}'",
        playlist_name, device_mount_point
    );

    let mount_point = PathBuf::from(&device_mount_point);
    let manager = state.playlist_manager.read().await;
    manager
        .sync_to_device(&playlist_name, &mount_point)
        .map_err(map_err)
}

// =============================================================================
// File Transfer Commands
// =============================================================================

/// Sync a playlist to a device with progress tracking.
///
/// This enhanced sync operation provides:
/// - Chunked file transfers for better performance
/// - Progress callbacks via Tauri events
/// - Optional integrity verification
/// - Detailed transfer statistics
///
/// Progress events are emitted to "transfer-progress" with `TransferProgress` payloads.
#[tauri::command]
pub async fn sync_playlist_with_progress(
    app: AppHandle,
    state: State<'_, AppState>,
    playlist_name: String,
    device_mount_point: String,
    verify_integrity: bool,
    skip_existing: bool,
) -> std::result::Result<TransferResult, String> {
    info!(
        "Syncing playlist '{}' to device at '{}' with progress tracking (verify={}, skip_existing={})",
        playlist_name, device_mount_point, verify_integrity, skip_existing
    );

    let mount_point = PathBuf::from(&device_mount_point);

    // Configure transfer options
    let mut options = TransferOptions::default();
    options.verify_integrity = verify_integrity;
    options.skip_existing = skip_existing;

    // Create progress callback that emits events
    let app_handle = app.clone();
    let progress_callback = move |progress: &TransferProgress| {
        if let Err(e) = app_handle.emit("transfer-progress", progress) {
            error!("Failed to emit transfer-progress event: {}", e);
        }
    };

    let manager = state.playlist_manager.read().await;
    manager
        .sync_to_device_with_progress(
            &playlist_name,
            &mount_point,
            &options,
            Some(progress_callback),
        )
        .map_err(map_err)
}

/// Get default transfer options.
///
/// Returns the default configuration for file transfers.
#[tauri::command]
pub fn get_default_transfer_options() -> TransferOptions {
    TransferOptions::default()
}

/// Get fast transfer options (no verification).
///
/// Returns transfer options optimized for speed without integrity verification.
#[tauri::command]
pub fn get_fast_transfer_options() -> TransferOptions {
    TransferOptions::fast()
}

/// Get reliable transfer options (full verification).
///
/// Returns transfer options optimized for reliability with full integrity verification.
#[tauri::command]
pub fn get_reliable_transfer_options() -> TransferOptions {
    TransferOptions::reliable()
}

/// Transfer specific files to a device.
///
/// This allows transferring a subset of files from a playlist to a device
/// with progress tracking and verification.
#[tauri::command]
pub async fn transfer_files_to_device(
    app: AppHandle,
    _state: State<'_, AppState>,
    source_files: Vec<String>,
    device_mount_point: String,
    options: TransferOptions,
) -> std::result::Result<TransferResult, String> {
    info!(
        "Transferring {} files to device at '{}'",
        source_files.len(),
        device_mount_point
    );

    // Validate options
    options.validate().map_err(map_err)?;

    let mount_point = PathBuf::from(&device_mount_point);
    let source_paths: Vec<PathBuf> = source_files.iter().map(PathBuf::from).collect();

    // Create progress callback
    let app_handle = app;
    let progress_callback = move |progress: &TransferProgress| {
        if let Err(e) = app_handle.emit("transfer-progress", progress) {
            error!("Failed to emit transfer-progress event: {}", e);
        }
    };

    // Perform transfer
    let mut engine = mp3youtube_core::TransferEngine::new();
    engine
        .transfer_files(
            &source_paths,
            &mount_point,
            &options,
            Some(progress_callback),
        )
        .map_err(map_err)
}

/// Compute the checksum of a file.
///
/// Returns the SHA-256 checksum of the specified file as a hex string.
#[tauri::command]
pub async fn compute_file_checksum(file_path: String) -> std::result::Result<String, String> {
    debug!("Computing checksum for: {}", file_path);

    let path = PathBuf::from(&file_path);
    let engine = mp3youtube_core::TransferEngine::new();

    engine.compute_file_checksum(&path).map_err(map_err)
}

/// Verify integrity of a transferred file.
///
/// Compares the checksum of a source file with its destination copy.
/// Returns `true` if checksums match, `false` otherwise.
#[tauri::command]
pub async fn verify_file_integrity(
    source_path: String,
    destination_path: String,
) -> std::result::Result<bool, String> {
    debug!(
        "Verifying integrity: {} vs {}",
        source_path, destination_path
    );

    let source = PathBuf::from(&source_path);
    let dest = PathBuf::from(&destination_path);

    let engine = mp3youtube_core::TransferEngine::new();

    let source_checksum = engine.compute_file_checksum(&source).map_err(map_err)?;
    let dest_checksum = engine.compute_file_checksum(&dest).map_err(map_err)?;

    let matches = source_checksum == dest_checksum;
    info!(
        "Integrity check: {} (source={}, dest={})",
        if matches { "PASSED" } else { "FAILED" },
        &source_checksum[..8],
        &dest_checksum[..8]
    );

    Ok(matches)
}

// =============================================================================
// Integrity Verification Commands
// =============================================================================

/// Event names for integrity verification events.
pub mod integrity_events {
    /// Event emitted for verification progress updates.
    pub const VERIFICATION_PROGRESS: &str = "integrity-verification-progress";
    /// Event emitted when verification completes.
    pub const VERIFICATION_COMPLETED: &str = "integrity-verification-completed";
}

/// Create a checksum manifest for a directory.
///
/// Scans all audio files in the directory, computes their SHA-256 checksums,
/// and saves the manifest as `checksums.json` in the directory.
///
/// Returns the number of files included in the manifest.
#[tauri::command]
pub async fn create_checksum_manifest(directory: String) -> std::result::Result<usize, String> {
    info!("Creating checksum manifest for directory: {}", directory);

    let path = PathBuf::from(&directory);
    let verifier = IntegrityVerifier::new();

    let manifest = verifier
        .create_manifest_from_directory(&path, None::<fn(&VerificationProgress)>)
        .map_err(map_err)?;

    let file_count = manifest.len();
    manifest.save_to_directory(&path).map_err(map_err)?;

    info!("Created checksum manifest with {} files", file_count);
    Ok(file_count)
}

/// Load a checksum manifest from a directory.
///
/// Returns the manifest contents including file checksums and metadata.
#[tauri::command]
pub async fn load_checksum_manifest(
    directory: String,
) -> std::result::Result<ChecksumManifest, String> {
    debug!("Loading checksum manifest from: {}", directory);

    let path = PathBuf::from(&directory);
    ChecksumManifest::load_from_directory(&path).map_err(map_err)
}

/// Check if a checksum manifest exists in a directory.
#[tauri::command]
pub async fn has_checksum_manifest(directory: String) -> std::result::Result<bool, String> {
    let path = PathBuf::from(&directory).join(mp3youtube_core::integrity::DEFAULT_MANIFEST_FILE);
    Ok(path.exists())
}

/// Verify all files in a directory against a checksum manifest.
///
/// Compares each file's current checksum against the stored value in the manifest.
/// Returns detailed verification results including pass/fail status for each file.
///
/// Progress events are emitted to "integrity-verification-progress".
#[tauri::command]
pub async fn verify_directory_integrity(
    app: AppHandle,
    directory: String,
    check_extra_files: bool,
) -> std::result::Result<VerificationResult, String> {
    info!("Verifying integrity of directory: {}", directory);

    let path = PathBuf::from(&directory);

    // Load the manifest
    let manifest = ChecksumManifest::load_from_directory(&path).map_err(map_err)?;

    // Configure verification options
    let mut options = VerificationOptions::default();
    options.check_extra_files = check_extra_files;

    let verifier = IntegrityVerifier::with_options(options);

    // Set up progress callback
    let app_handle = app.clone();
    let progress_callback = move |progress: &VerificationProgress| {
        if let Err(e) = app_handle.emit(integrity_events::VERIFICATION_PROGRESS, progress) {
            error!("Failed to emit verification-progress event: {}", e);
        }
    };

    let result = verifier
        .verify_directory(&path, &manifest, Some(progress_callback))
        .map_err(map_err)?;

    // Emit completion event
    if let Err(e) = app.emit(integrity_events::VERIFICATION_COMPLETED, &result) {
        error!("Failed to emit verification-completed event: {}", e);
    }

    info!(
        "Verification complete: {} passed, {} failed, {} extra files",
        result.passed, result.failed, result.extra_files
    );

    Ok(result)
}

/// Verify a single file against an expected checksum.
///
/// Returns true if the file's checksum matches the expected value.
#[tauri::command]
pub async fn verify_file_checksum(
    file_path: String,
    expected_checksum: String,
    expected_size: u64,
) -> std::result::Result<bool, String> {
    debug!("Verifying file checksum: {}", file_path);

    let path = PathBuf::from(&file_path);
    let verifier = IntegrityVerifier::new();

    let expected = FileChecksum::new(
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string(),
        expected_checksum,
        expected_size,
    );

    let result = verifier.verify_file(&path, &expected).map_err(map_err)?;
    Ok(result.passed)
}

/// Add or update a file in a checksum manifest.
///
/// Computes the checksum for the specified file and adds/updates it in the manifest.
/// The manifest is saved to the directory containing the file.
#[tauri::command]
pub async fn update_manifest_file(
    file_path: String,
    manifest_dir: String,
) -> std::result::Result<(), String> {
    info!("Updating manifest for file: {}", file_path);

    let path = PathBuf::from(&file_path);
    let manifest_path = PathBuf::from(&manifest_dir);

    // Load or create manifest
    let mut manifest = match ChecksumManifest::load_from_directory(&manifest_path) {
        Ok(m) => m,
        Err(_) => ChecksumManifest::new(),
    };

    // Compute checksum for the file
    let verifier = IntegrityVerifier::new();
    let checksum = verifier.compute_checksum(&path).map_err(map_err)?;

    // Get file metadata
    let metadata = std::fs::metadata(&path).map_err(|e| {
        map_err(Error::FileSystem(
            mp3youtube_core::error::FileSystemError::ReadFailed {
                path: path.clone(),
                reason: e.to_string(),
            },
        ))
    })?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    // Add to manifest
    let file_checksum = FileChecksum::new(file_name, checksum, metadata.len());
    manifest.add_file(file_checksum);

    // Save manifest
    manifest
        .save_to_directory(&manifest_path)
        .map_err(map_err)?;

    info!("Manifest updated successfully");
    Ok(())
}

/// Remove a file from a checksum manifest.
#[tauri::command]
pub async fn remove_from_manifest(
    file_name: String,
    manifest_dir: String,
) -> std::result::Result<bool, String> {
    debug!("Removing file from manifest: {}", file_name);

    let manifest_path = PathBuf::from(&manifest_dir);

    // Load manifest
    let mut manifest = ChecksumManifest::load_from_directory(&manifest_path).map_err(map_err)?;

    // Remove file
    let removed = manifest.remove_file(&file_name).is_some();

    // Save manifest
    manifest
        .save_to_directory(&manifest_path)
        .map_err(map_err)?;

    info!("Removed file '{}' from manifest: {}", file_name, removed);
    Ok(removed)
}

/// Get verification options presets.
#[tauri::command]
pub fn get_default_verification_options() -> VerificationOptions {
    VerificationOptions::default()
}

/// Get strict verification options.
#[tauri::command]
pub const fn get_strict_verification_options() -> VerificationOptions {
    VerificationOptions::strict()
}

/// Get quick verification options.
#[tauri::command]
pub fn get_quick_verification_options() -> VerificationOptions {
    VerificationOptions::quick()
}

/// Get tracks for a playlist.
#[tauri::command]
pub async fn get_playlist_tracks(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<Vec<TrackInfo>, String> {
    debug!("Getting tracks for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.list_tracks(&name).map_err(map_err)
}

/// Get detailed metadata for a specific playlist.
///
/// Returns playlist metadata including name, source URL, creation time,
/// modification time, track count, and total size.
#[tauri::command]
pub async fn get_playlist_details(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<PlaylistMetadata, String> {
    debug!("Getting details for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    let playlist_path = manager.get_playlist_path(&name).map_err(map_err)?;
    let metadata = manager
        .get_playlist_metadata(&playlist_path)
        .map_err(map_err)?;
    info!(
        "Retrieved details for playlist '{}': {} tracks, {} bytes",
        name, metadata.track_count, metadata.total_bytes
    );
    Ok(metadata)
}

/// Validate a playlist folder structure.
///
/// Checks if the folder exists, has valid metadata, and contains audio files.
/// Returns a validation result with details about any issues found.
#[tauri::command]
pub async fn validate_playlist_folder(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<mp3youtube_core::playlist::FolderValidationResult, String> {
    debug!("Validating playlist folder: {}", name);
    let manager = state.playlist_manager.read().await;
    let result = manager.validate_folder(&name);
    info!(
        "Validation result for '{}': exists={}, has_metadata={}, metadata_valid={}, audio_files={}, issues={}",
        name,
        result.exists,
        result.has_metadata,
        result.metadata_valid,
        result.audio_file_count,
        result.issues.len()
    );
    Ok(result)
}

/// Get statistics about a playlist folder.
///
/// Returns information about file counts, sizes, and metadata status.
#[tauri::command]
pub async fn get_playlist_statistics(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<mp3youtube_core::playlist::FolderStatistics, String> {
    debug!("Getting statistics for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    let stats = manager.get_folder_statistics(&name).map_err(map_err)?;
    info!(
        "Statistics for '{}': {} audio files, {} other files, {} bytes total",
        name, stats.audio_files, stats.other_files, stats.total_size_bytes
    );
    Ok(stats)
}

/// Repair a playlist folder by fixing common issues.
///
/// Currently this creates missing metadata files and fixes corrupted metadata.
/// Returns a list of repairs that were made.
#[tauri::command]
pub async fn repair_playlist_folder(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<Vec<String>, String> {
    info!("Repairing playlist folder: {}", name);
    let manager = state.playlist_manager.read().await;
    let repairs = manager.repair_folder(&name).map_err(map_err)?;
    if repairs.is_empty() {
        info!("No repairs needed for playlist '{}'", name);
    } else {
        info!("Repaired playlist '{}': {:?}", name, repairs);
    }
    Ok(repairs)
}

/// Extract MP3 metadata (ID3 tags) from a single file.
///
/// Returns metadata including title, artist, album, duration, track number, etc.
/// If the file has no tags or is not a valid MP3, returns empty metadata.
#[tauri::command]
pub async fn extract_track_metadata(path: String) -> std::result::Result<Mp3Metadata, String> {
    debug!("Extracting metadata from: {}", path);
    let path_buf = PathBuf::from(&path);
    extract_metadata(&path_buf).map_err(map_err)
}

/// Get tracks for a playlist without metadata extraction.
///
/// This is faster than `get_playlist_tracks` when metadata is not needed.
#[tauri::command]
pub async fn get_playlist_tracks_fast(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<Vec<TrackInfo>, String> {
    debug!("Getting tracks (fast) for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager
        .list_tracks_with_options(&name, false)
        .map_err(map_err)
}

/// Import an existing folder as a playlist.
///
/// Creates metadata for a folder that already contains audio files.
/// The folder must be in the playlists directory.
#[tauri::command]
pub async fn import_playlist_folder(
    state: State<'_, AppState>,
    folder_name: String,
    source_url: Option<String>,
) -> std::result::Result<String, String> {
    info!("Importing folder as playlist: {}", folder_name);
    let manager = state.playlist_manager.read().await;
    let folder_path = manager.base_path().join(&folder_name);
    let name = manager
        .import_folder(&folder_path, source_url)
        .map_err(map_err)?;
    info!("Successfully imported folder '{}' as playlist", name);
    Ok(name)
}

/// Rename a playlist.
///
/// This renames the playlist folder and updates any metadata as needed.
#[tauri::command]
pub async fn rename_playlist(
    state: State<'_, AppState>,
    old_name: String,
    new_name: String,
) -> std::result::Result<(), String> {
    info!("Renaming playlist '{}' to '{}'", old_name, new_name);

    // Validate the new name
    mp3youtube_core::playlist::validate_playlist_name(&new_name).map_err(map_err)?;

    let manager = state.playlist_manager.read().await;
    let old_path = manager.get_playlist_path(&old_name).map_err(map_err)?;
    let new_path = manager.base_path().join(&new_name);

    // Check if new name already exists
    if new_path.exists() {
        return Err(map_err(Error::Playlist(
            mp3youtube_core::error::PlaylistError::AlreadyExists { name: new_name },
        )));
    }

    // Rename the folder
    std::fs::rename(&old_path, &new_path).map_err(|e| {
        map_err(Error::FileSystem(
            mp3youtube_core::error::FileSystemError::WriteFailed {
                path: new_path.clone(),
                reason: format!("Failed to rename playlist folder: {e}"),
            },
        ))
    })?;

    info!(
        "Successfully renamed playlist '{}' to '{}'",
        old_name, new_name
    );
    Ok(())
}

/// Check if a playlist exists.
#[tauri::command]
pub async fn playlist_exists(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<bool, String> {
    debug!("Checking if playlist exists: {}", name);
    let manager = state.playlist_manager.read().await;
    let exists = manager.get_playlist_path(&name).is_ok();
    Ok(exists)
}

/// Ensure a playlist folder has proper structure.
///
/// Creates the metadata file if it doesn't exist.
#[tauri::command]
pub async fn ensure_playlist_structure(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    debug!("Ensuring folder structure for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.ensure_folder_structure(&name).map_err(map_err)?;
    info!("Ensured folder structure for playlist '{}'", name);
    Ok(())
}

/// Get the saved metadata for a playlist.
///
/// Returns the raw metadata stored in playlist.json, including title,
/// description, source URL, timestamps, track count, and total size.
#[tauri::command]
pub async fn get_playlist_saved_metadata(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<SavedPlaylistMetadata, String> {
    debug!("Getting saved metadata for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.get_saved_metadata(&name).map_err(map_err)
}

/// Update playlist metadata.
///
/// Updates the playlist.json file with new metadata values.
/// Pass `null` for fields that should not be changed.
/// Pass an empty string to clear a field.
#[tauri::command]
pub async fn update_playlist_metadata(
    state: State<'_, AppState>,
    name: String,
    title: Option<String>,
    description: Option<String>,
    source_url: Option<Option<String>>,
    thumbnail_url: Option<Option<String>>,
) -> std::result::Result<SavedPlaylistMetadata, String> {
    info!("Updating metadata for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager
        .update_playlist_metadata_full(&name, title, description, source_url, thumbnail_url)
        .map_err(map_err)
}

/// Refresh the cached track count and total size for a playlist.
///
/// Scans the playlist folder and updates the `track_count` and `total_size_bytes`
/// fields in the metadata file.
#[tauri::command]
pub async fn refresh_playlist_stats(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<SavedPlaylistMetadata, String> {
    debug!("Refreshing stats for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.refresh_playlist_stats(&name).map_err(map_err)
}

/// Get the status of a running task.
#[tauri::command]
pub async fn get_task_status(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> std::result::Result<Option<String>, String> {
    let status = state.task_status(task_id).await;
    Ok(status.map(|s| format!("{s:?}")))
}

/// Cancel a running task.
///
/// Returns `true` if the task was successfully cancelled, `false` otherwise.
#[tauri::command]
pub async fn cancel_task(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> std::result::Result<bool, String> {
    info!("Cancelling task {}", task_id);
    let cancelled = state.runtime().cancel_task(task_id).await;
    if cancelled {
        info!("Task {} cancelled successfully", task_id);
    } else {
        debug!(
            "Task {} could not be cancelled (not found or already completed)",
            task_id
        );
    }
    Ok(cancelled)
}

/// Get all running tasks count by category.
#[tauri::command]
pub async fn get_running_tasks(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<(String, usize)>, String> {
    let counts = state.runtime().running_tasks_count().await;
    Ok(counts
        .into_iter()
        .map(|(cat, count)| (cat.to_string(), count))
        .collect())
}

// =============================================================================
// Configuration Commands
// =============================================================================

/// Get the current application configuration.
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> std::result::Result<AppConfig, String> {
    debug!("Getting config");
    let config_manager = state.config_manager.read().await;
    Ok(config_manager.config().clone())
}

/// Update the application configuration.
///
/// This will also reinitialize the playlist manager if the playlists directory changes.
#[tauri::command]
pub async fn update_config(
    state: State<'_, AppState>,
    config: AppConfig,
) -> std::result::Result<(), String> {
    info!("Updating config");
    debug!(
        "New playlists directory: {}",
        config.playlists_directory.display()
    );

    let new_playlists_dir = config.playlists_directory.clone();

    // Update config
    {
        let mut config_manager = state.config_manager.write().await;
        config_manager.update(config).map_err(map_err)?;
    }

    // Reinitialize playlist manager with new directory
    state
        .reinitialize_playlist_manager(new_playlists_dir)
        .await
        .map_err(map_err)?;

    info!("Config updated successfully");
    Ok(())
}

/// Get the current playlists storage directory.
#[tauri::command]
pub async fn get_storage_directory(
    state: State<'_, AppState>,
) -> std::result::Result<String, String> {
    debug!("Getting storage directory");
    let config_manager = state.config_manager.read().await;
    Ok(config_manager.playlists_directory().display().to_string())
}

/// Set the playlists storage directory.
///
/// This will validate the directory and reinitialize the playlist manager.
#[tauri::command]
pub async fn set_storage_directory(
    state: State<'_, AppState>,
    path: String,
) -> std::result::Result<(), String> {
    let new_path = PathBuf::from(&path);
    info!("Setting storage directory to: {}", new_path.display());

    // Update config
    {
        let mut config_manager = state.config_manager.write().await;
        config_manager
            .set_playlists_directory(new_path.clone())
            .map_err(map_err)?;
    }

    // Reinitialize playlist manager with new directory
    state
        .reinitialize_playlist_manager(new_path)
        .await
        .map_err(map_err)?;

    info!("Storage directory updated successfully");
    Ok(())
}

/// Get the default storage directory.
#[tauri::command]
pub fn get_default_storage_directory() -> String {
    mp3youtube_core::config::default_playlists_directory()
        .display()
        .to_string()
}

// =============================================================================
// Sync API Commands
// =============================================================================

/// Sync progress payload for events.
#[derive(Debug, Clone, serde::Serialize)]
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

impl SyncProgressPayload {
    fn from_transfer_progress(
        task_id: TaskId,
        playlist_name: &str,
        device_mount_point: &str,
        progress: &TransferProgress,
    ) -> Self {
        Self {
            task_id,
            status: progress.status.to_string(),
            playlist_name: playlist_name.to_string(),
            device_mount_point: device_mount_point.to_string(),
            current_file_index: progress.current_file_index,
            total_files: progress.total_files,
            current_file_name: progress.current_file_name.clone(),
            current_file_bytes: progress.current_file_bytes,
            current_file_total: progress.current_file_total,
            total_bytes_transferred: progress.total_bytes_transferred,
            total_bytes: progress.total_bytes,
            files_completed: progress.files_completed,
            files_skipped: progress.files_skipped,
            files_failed: progress.files_failed,
            transfer_speed_bps: progress.transfer_speed_bps,
            estimated_remaining_secs: progress.estimated_remaining_secs,
            elapsed_secs: progress.elapsed_secs,
            overall_progress_percent: progress.overall_progress_percent(),
        }
    }
}

/// Sync result payload for completion events.
#[derive(Debug, Clone, serde::Serialize)]
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

/// Start a sync operation to transfer a playlist to a device.
///
/// This spawns a background task that transfers files with progress tracking.
/// Progress events are emitted to "sync-progress" and completion events to
/// "sync-completed", "sync-failed", or "sync-cancelled".
///
/// Returns the task ID that can be used to track or cancel the sync.
#[tauri::command]
pub async fn start_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    playlist_name: String,
    device_mount_point: String,
    verify_integrity: bool,
    skip_existing: bool,
) -> std::result::Result<TaskId, String> {
    info!(
        "Starting sync: playlist '{}' -> device '{}' (verify={}, skip_existing={})",
        playlist_name, device_mount_point, verify_integrity, skip_existing
    );

    let mount_point = PathBuf::from(&device_mount_point);

    // Verify device is accessible
    if !mount_point.exists() || !mount_point.is_dir() {
        return Err(map_err(Error::device_not_found(&device_mount_point)));
    }

    // Configure transfer options
    let mut options = TransferOptions::default();
    options.verify_integrity = verify_integrity;
    options.skip_existing = skip_existing;

    // Create cancellation token
    let cancel_token = Arc::new(AtomicBool::new(false));
    let cancel_token_clone = Arc::clone(&cancel_token);

    // Generate task ID
    let task_id = state.runtime().spawn(
        TaskCategory::FileTransfer,
        Some(format!("Sync '{playlist_name}' to '{device_mount_point}'")),
        async {},
    );

    // Register the sync task
    let sync_info = SyncTaskInfo {
        task_id,
        playlist_name: playlist_name.clone(),
        device_mount_point: device_mount_point.clone(),
        verify_integrity,
        skip_existing,
    };
    state
        .register_sync_task(task_id, sync_info, cancel_token)
        .await;

    // Emit sync started event
    let started_payload = SyncTaskInfo {
        task_id,
        playlist_name: playlist_name.clone(),
        device_mount_point: device_mount_point.clone(),
        verify_integrity,
        skip_existing,
    };
    if let Err(e) = app.emit(sync_events::SYNC_STARTED, &started_payload) {
        error!("Failed to emit sync-started event: {}", e);
    }

    // Clone necessary data for the async task
    let playlist_name_clone = playlist_name.clone();
    let device_mount_point_clone = device_mount_point.clone();
    let app_handle = app.clone();
    let playlist_manager = state.playlist_manager_arc();
    let sync_tasks = Arc::clone(&state.sync_tasks);

    // Spawn the actual sync operation
    tokio::spawn(async move {
        let playlist_name_for_progress = playlist_name_clone.clone();
        let device_mount_point_for_progress = device_mount_point_clone.clone();
        let app_handle_for_progress = app_handle.clone();
        let task_id_for_progress = task_id;

        // Create progress callback that emits events
        let progress_callback = move |progress: &TransferProgress| {
            let payload = SyncProgressPayload::from_transfer_progress(
                task_id_for_progress,
                &playlist_name_for_progress,
                &device_mount_point_for_progress,
                progress,
            );
            if let Err(e) = app_handle_for_progress.emit(sync_events::SYNC_PROGRESS, &payload) {
                error!("Failed to emit sync-progress event: {}", e);
            }
        };

        // Perform the sync
        let manager = playlist_manager.read().await;
        let result = manager.sync_to_device_cancellable(
            &playlist_name_clone,
            &PathBuf::from(&device_mount_point_clone),
            &options,
            cancel_token_clone,
            Some(progress_callback),
        );

        // Unregister the sync task
        {
            let mut tasks = sync_tasks.write().await;
            tasks.remove(&task_id);
        }

        // Emit completion event
        match result {
            Ok(transfer_result) => {
                let payload = SyncResultPayload {
                    task_id,
                    success: transfer_result.success,
                    was_cancelled: transfer_result.was_cancelled,
                    playlist_name: playlist_name_clone.clone(),
                    device_mount_point: device_mount_point_clone.clone(),
                    total_files: transfer_result.total_files,
                    files_transferred: transfer_result.files_transferred,
                    files_skipped: transfer_result.files_skipped,
                    files_failed: transfer_result.files_failed,
                    bytes_transferred: transfer_result.bytes_transferred,
                    duration_secs: transfer_result.duration_secs,
                    error_message: None,
                };

                let event = if transfer_result.was_cancelled {
                    info!("Sync task {} was cancelled", task_id);
                    sync_events::SYNC_CANCELLED
                } else if transfer_result.success {
                    info!(
                        "Sync task {} completed: {} files transferred, {} skipped, {} failed",
                        task_id,
                        transfer_result.files_transferred,
                        transfer_result.files_skipped,
                        transfer_result.files_failed
                    );
                    sync_events::SYNC_COMPLETED
                } else {
                    error!(
                        "Sync task {} failed: {} files failed",
                        task_id, transfer_result.files_failed
                    );
                    sync_events::SYNC_FAILED
                };

                if let Err(e) = app_handle.emit(event, &payload) {
                    error!("Failed to emit {} event: {}", event, e);
                }
            }
            Err(e) => {
                error!("Sync task {} failed with error: {}", task_id, e);
                let payload = SyncResultPayload {
                    task_id,
                    success: false,
                    was_cancelled: false,
                    playlist_name: playlist_name_clone.clone(),
                    device_mount_point: device_mount_point_clone.clone(),
                    total_files: 0,
                    files_transferred: 0,
                    files_skipped: 0,
                    files_failed: 0,
                    bytes_transferred: 0,
                    duration_secs: 0.0,
                    error_message: Some(e.to_string()),
                };

                if let Err(e) = app_handle.emit(sync_events::SYNC_FAILED, &payload) {
                    error!("Failed to emit sync-failed event: {}", e);
                }
            }
        }
    });

    info!("Sync task {} spawned successfully", task_id);
    Ok(task_id)
}

/// Cancel a running sync operation.
///
/// Returns `true` if the cancellation was requested successfully, `false` if
/// the sync task was not found (may have already completed).
#[tauri::command]
pub async fn cancel_sync(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> std::result::Result<bool, String> {
    info!("Cancelling sync task {}", task_id);
    let cancelled = state.cancel_sync_task(task_id).await;
    if cancelled {
        info!("Sync task {} cancellation requested", task_id);
    } else {
        debug!("Sync task {} not found or already completed", task_id);
    }
    Ok(cancelled)
}

/// Get the status of a sync operation.
///
/// Returns information about the sync task, or None if the task was not found.
#[tauri::command]
pub async fn get_sync_status(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> std::result::Result<Option<SyncTaskInfo>, String> {
    debug!("Getting sync status for task {}", task_id);
    let info = state.get_sync_task_info(task_id).await;
    Ok(info)
}

/// Get all currently active sync operations.
///
/// Returns a list of all sync tasks that are currently running.
#[tauri::command]
pub async fn list_active_syncs(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<SyncTaskInfo>, String> {
    debug!("Listing active syncs");
    let syncs = state.list_sync_tasks().await;
    info!("Found {} active sync operations", syncs.len());
    Ok(syncs)
}

// =============================================================================
// Sync Orchestrator Commands
// =============================================================================

/// Event names for sync orchestrator events.
pub mod sync_orchestrator_events {
    /// Event emitted during sync progress.
    pub const SYNC_ORCHESTRATOR_PROGRESS: &str = "sync-orchestrator-progress";
    /// Event emitted when sync completes successfully.
    pub const SYNC_ORCHESTRATOR_COMPLETED: &str = "sync-orchestrator-completed";
    /// Event emitted when sync fails.
    pub const SYNC_ORCHESTRATOR_FAILED: &str = "sync-orchestrator-failed";
    /// Event emitted when sync is cancelled.
    pub const SYNC_ORCHESTRATOR_CANCELLED: &str = "sync-orchestrator-cancelled";
}

/// Start a multi-playlist sync operation using the sync orchestrator.
///
/// This is an enhanced sync that:
/// - Supports syncing multiple playlists at once
/// - Performs device cleanup before transfer
/// - Verifies device connection between phases
/// - Provides detailed progress tracking for each phase
///
/// Progress events are emitted to "sync-orchestrator-progress".
/// Completion events are emitted to "sync-orchestrator-completed",
/// "sync-orchestrator-failed", or "sync-orchestrator-cancelled".
///
/// Returns the task ID that can be used to track or cancel the sync.
#[tauri::command]
pub async fn start_orchestrated_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    playlists: Vec<String>,
    device_mount_point: String,
    cleanup_enabled: bool,
    verify_integrity: bool,
    skip_existing: bool,
) -> std::result::Result<TaskId, String> {
    info!(
        "Starting orchestrated sync: {} playlist(s) -> device '{}' (cleanup={}, verify={}, skip_existing={})",
        playlists.len(),
        device_mount_point,
        cleanup_enabled,
        verify_integrity,
        skip_existing
    );

    if playlists.is_empty() {
        return Err(map_err(Error::Configuration(
            "No playlists specified for sync".to_string(),
        )));
    }

    let mount_point = PathBuf::from(&device_mount_point);

    // Verify device is accessible
    if !mount_point.exists() || !mount_point.is_dir() {
        return Err(map_err(Error::device_not_found(&device_mount_point)));
    }

    // Configure sync options
    let mut sync_options = SyncOptions::default();
    sync_options.cleanup_enabled = cleanup_enabled;
    sync_options.transfer_options.verify_integrity = verify_integrity;
    sync_options.transfer_options.skip_existing = skip_existing;

    // Create cancellation token
    let cancel_token = Arc::new(AtomicBool::new(false));
    let cancel_token_clone = Arc::clone(&cancel_token);

    // Generate task ID
    let task_id = state.runtime().spawn(
        TaskCategory::FileTransfer,
        Some(format!(
            "Orchestrated sync: {} playlist(s) to '{}'",
            playlists.len(),
            device_mount_point
        )),
        async {},
    );

    // Register the sync task (using first playlist name for compatibility)
    let sync_info = SyncTaskInfo {
        task_id,
        playlist_name: playlists.first().cloned().unwrap_or_default(),
        device_mount_point: device_mount_point.clone(),
        verify_integrity,
        skip_existing,
    };
    state
        .register_sync_task(task_id, sync_info.clone(), Arc::clone(&cancel_token))
        .await;

    // Emit sync started event
    if let Err(e) = app.emit(sync_events::SYNC_STARTED, &sync_info) {
        error!("Failed to emit sync-started event: {}", e);
    }

    // Clone necessary data for the async task
    let playlists_clone = playlists.clone();
    let device_mount_point_clone = device_mount_point.clone();
    let app_handle = app.clone();
    let playlist_manager = state.playlist_manager_arc();
    let device_manager = state.device_manager_arc();
    let sync_tasks = Arc::clone(&state.sync_tasks);

    // Spawn the actual sync operation
    tokio::spawn(async move {
        let orchestrator = SyncOrchestrator::with_cancellation(cancel_token_clone);
        let request = SyncRequest::new(
            playlists_clone.clone(),
            PathBuf::from(&device_mount_point_clone),
        );

        // Set up progress callback
        let app_handle_for_progress = app_handle.clone();
        let progress_callback = move |progress: &SyncProgress| {
            if let Err(e) = app_handle_for_progress.emit(
                sync_orchestrator_events::SYNC_ORCHESTRATOR_PROGRESS,
                progress,
            ) {
                error!("Failed to emit sync-orchestrator-progress event: {}", e);
            }
        };

        // Get manager locks
        let playlist_mgr = playlist_manager.read().await;
        let device_mgr = device_manager.read().await;

        // Perform the orchestrated sync
        let result = orchestrator.sync(
            &playlist_mgr,
            &*device_mgr,
            request,
            &sync_options,
            Some(progress_callback),
        );

        // Release locks
        drop(playlist_mgr);
        drop(device_mgr);

        // Unregister the sync task
        {
            let mut tasks = sync_tasks.write().await;
            tasks.remove(&task_id);
        }

        // Emit completion event
        match result {
            Ok(sync_result) => {
                let event = if sync_result.was_cancelled {
                    info!("Orchestrated sync task {} was cancelled", task_id);
                    sync_orchestrator_events::SYNC_ORCHESTRATOR_CANCELLED
                } else if sync_result.success {
                    info!(
                        "Orchestrated sync task {} completed: {} files transferred, {} skipped, {} failed",
                        task_id,
                        sync_result.total_files_transferred,
                        sync_result.total_files_skipped,
                        sync_result.total_files_failed
                    );
                    sync_orchestrator_events::SYNC_ORCHESTRATOR_COMPLETED
                } else {
                    error!(
                        "Orchestrated sync task {} failed: {} files failed",
                        task_id, sync_result.total_files_failed
                    );
                    sync_orchestrator_events::SYNC_ORCHESTRATOR_FAILED
                };

                if let Err(e) = app_handle.emit(event, &sync_result) {
                    error!("Failed to emit {} event: {}", event, e);
                }
            }
            Err(e) => {
                error!(
                    "Orchestrated sync task {} failed with error: {}",
                    task_id, e
                );
                let error_result = CoreSyncResult {
                    success: false,
                    was_cancelled: false,
                    final_phase: mp3youtube_core::sync::SyncPhase::Failed,
                    cleanup_result: None,
                    transfer_results: vec![],
                    total_files_transferred: 0,
                    total_files_skipped: 0,
                    total_files_failed: 0,
                    total_bytes_transferred: 0,
                    duration_secs: 0.0,
                    average_speed_bps: 0.0,
                    error_message: Some(e.to_string()),
                };

                if let Err(emit_err) = app_handle.emit(
                    sync_orchestrator_events::SYNC_ORCHESTRATOR_FAILED,
                    &error_result,
                ) {
                    error!(
                        "Failed to emit sync-orchestrator-failed event: {}",
                        emit_err
                    );
                }
            }
        }
    });

    info!("Orchestrated sync task {} spawned successfully", task_id);
    Ok(task_id)
}

// =============================================================================
// YouTube URL Validation Commands
// =============================================================================

/// Validate a `YouTube` URL and extract playlist information.
///
/// This command validates whether a given URL is a valid `YouTube` playlist URL
/// and extracts the playlist ID if valid. It supports multiple URL formats:
///
/// - Standard playlist URLs: `https://www.youtube.com/playlist?list=PLxxxxxxxx`
/// - Watch URLs with playlist: `https://www.youtube.com/watch?v=xxx&list=PLxxxxxxxx`
/// - Short URLs with playlist: `https://youtu.be/xxx?list=PLxxxxxxxx`
///
/// Returns a `YouTubeUrlValidation` object containing:
/// - `is_valid`: Whether the URL is valid
/// - `playlist_id`: The extracted playlist ID (if valid)
/// - `normalized_url`: A normalized/canonical version of the URL
/// - `error_message`: Error details if validation failed
/// - `url_type`: The type of `YouTube` URL detected
#[tauri::command]
pub fn validate_youtube_playlist_url(url: String) -> YouTubeUrlValidation {
    debug!("Validating YouTube URL: {}", url);
    let result = validate_youtube_url(&url);

    if result.is_valid {
        info!(
            "URL validation succeeded: playlist_id={:?}, type={:?}",
            result.playlist_id, result.url_type
        );
    } else {
        info!("URL validation failed: {:?}", result.error_message);
    }

    result
}

/// Check if a URL is a valid `YouTube` playlist URL.
///
/// This is a simpler version that just returns true/false.
#[tauri::command]
pub fn is_valid_youtube_playlist_url(url: String) -> bool {
    let result = validate_youtube_url(&url);
    result.is_valid
}

/// Extract the playlist ID from a `YouTube` URL.
///
/// Returns the playlist ID if the URL is valid, or an error message if not.
#[tauri::command]
pub fn extract_youtube_playlist_id(url: String) -> std::result::Result<String, String> {
    debug!("Extracting playlist ID from URL: {}", url);
    let result = validate_youtube_url(&url);

    if result.is_valid {
        // SAFETY: is_valid guarantees playlist_id is Some
        #[allow(clippy::unwrap_used)]
        let playlist_id = result.playlist_id.unwrap();
        info!("Extracted playlist ID: {}", playlist_id);
        Ok(playlist_id)
    } else {
        let error_message = result
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string());
        info!("Failed to extract playlist ID: {}", error_message);
        Err(error_message)
    }
}

/// Get default sync options for the orchestrator.
///
/// Returns the default configuration for sync operations.
#[tauri::command]
pub fn get_default_sync_options() -> SyncOptions {
    SyncOptions::default()
}

/// Get fast sync options for the orchestrator.
///
/// Returns sync options optimized for speed.
#[tauri::command]
pub fn get_fast_sync_options() -> SyncOptions {
    SyncOptions::fast()
}

/// Get reliable sync options for the orchestrator.
///
/// Returns sync options optimized for reliability.
#[tauri::command]
pub fn get_reliable_sync_options() -> SyncOptions {
    SyncOptions::reliable()
}

// =============================================================================
// YouTube Download Commands
// =============================================================================

/// Event names for `YouTube` download events emitted to the frontend.
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

/// Serializable download progress for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadProgressPayload {
    /// Task ID for this download operation.
    pub task_id: TaskId,
    /// Current video index (1-based).
    pub current_index: usize,
    /// Total number of videos.
    pub total_videos: usize,
    /// Current video title.
    pub current_title: String,
    /// Download progress for current video (0.0 - 1.0).
    pub current_progress: f64,
    /// Overall progress (0.0 - 1.0).
    pub overall_progress: f64,
    /// Status message.
    pub status: String,
    /// Bytes downloaded for the current file.
    pub current_bytes: u64,
    /// Total bytes for the current file (if known).
    pub current_total_bytes: Option<u64>,
    /// Total bytes downloaded across all files.
    pub total_bytes_downloaded: u64,
    /// Download speed in bytes per second.
    pub download_speed_bps: f64,
    /// Formatted download speed (e.g., "1.5 MB/s").
    pub formatted_speed: String,
    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,
    /// Formatted estimated time remaining (e.g., "2:30").
    pub formatted_eta: Option<String>,
    /// Elapsed time in seconds since download started.
    pub elapsed_secs: f64,
    /// Formatted elapsed time (e.g., "1:15").
    pub formatted_elapsed: String,
    /// Number of videos completed successfully.
    pub videos_completed: usize,
    /// Number of videos skipped (already exist).
    pub videos_skipped: usize,
    /// Number of videos that failed.
    pub videos_failed: usize,
}

impl DownloadProgressPayload {
    fn from_progress(task_id: TaskId, progress: &DownloadProgress) -> Self {
        Self {
            task_id,
            current_index: progress.current_index,
            total_videos: progress.total_videos,
            current_title: progress.current_title.clone(),
            current_progress: progress.current_progress,
            overall_progress: progress.overall_progress,
            status: match &progress.status {
                DownloadStatus::Starting => "starting".to_string(),
                DownloadStatus::Downloading => "downloading".to_string(),
                DownloadStatus::Converting => "converting".to_string(),
                DownloadStatus::Completed => "completed".to_string(),
                DownloadStatus::Failed(msg) => format!("failed: {msg}"),
                DownloadStatus::Skipped => "skipped".to_string(),
            },
            current_bytes: progress.current_bytes,
            current_total_bytes: progress.current_total_bytes,
            total_bytes_downloaded: progress.total_bytes_downloaded,
            download_speed_bps: progress.download_speed_bps,
            formatted_speed: progress.formatted_speed(),
            estimated_remaining_secs: progress.estimated_remaining_secs,
            formatted_eta: progress.formatted_eta(),
            elapsed_secs: progress.elapsed_secs,
            formatted_elapsed: progress.formatted_elapsed(),
            videos_completed: progress.videos_completed,
            videos_skipped: progress.videos_skipped,
            videos_failed: progress.videos_failed,
        }
    }
}

/// Category of YouTube-related errors for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YouTubeErrorCategory {
    /// Network connection issues (no internet, DNS failure, timeout).
    Network,
    /// `YouTube` service issues (rate limiting, service unavailable).
    YouTubeService,
    /// Invalid or malformed URL.
    InvalidUrl,
    /// Playlist not found or is private.
    PlaylistNotFound,
    /// Video is unavailable (private, deleted, region-locked).
    VideoUnavailable,
    /// Age-restricted content requiring authentication.
    AgeRestricted,
    /// Geographic restriction on content.
    GeoRestricted,
    /// Failed to extract or download audio stream.
    AudioExtraction,
    /// File system error (disk full, permission denied).
    FileSystem,
    /// Operation was cancelled by user.
    Cancelled,
    /// Unknown or unclassified error.
    Unknown,
}

impl YouTubeErrorCategory {
    /// Get a user-friendly title for this error category.
    #[must_use]
    pub const fn title(&self) -> &'static str {
        match self {
            Self::Network => "Network Error",
            Self::YouTubeService => "YouTube Service Error",
            Self::InvalidUrl => "Invalid URL",
            Self::PlaylistNotFound => "Playlist Not Found",
            Self::VideoUnavailable => "Video Unavailable",
            Self::AgeRestricted => "Age-Restricted Content",
            Self::GeoRestricted => "Geographic Restriction",
            Self::AudioExtraction => "Audio Extraction Failed",
            Self::FileSystem => "File System Error",
            Self::Cancelled => "Cancelled",
            Self::Unknown => "Error",
        }
    }

    /// Get a user-friendly description for this error category.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Network => {
                "Could not connect to YouTube. Please check your internet connection and try again."
            }
            Self::YouTubeService => {
                "YouTube is temporarily unavailable or has rate-limited requests. Please wait a moment and try again."
            }
            Self::InvalidUrl => "The provided URL is not a valid YouTube playlist URL.",
            Self::PlaylistNotFound => {
                "The playlist could not be found. It may be private, deleted, or the URL is incorrect."
            }
            Self::VideoUnavailable => {
                "One or more videos in the playlist are unavailable (private, deleted, or restricted)."
            }
            Self::AgeRestricted => {
                "This content is age-restricted and requires authentication to access."
            }
            Self::GeoRestricted => "This content is not available in your geographic region.",
            Self::AudioExtraction => {
                "Failed to extract audio from the video. The format may not be supported."
            }
            Self::FileSystem => "Could not save the file. Please check disk space and permissions.",
            Self::Cancelled => "The download was cancelled.",
            Self::Unknown => "An unexpected error occurred.",
        }
    }
}

/// Classify an error into a category for user display.
fn classify_error(error: &Error) -> YouTubeErrorCategory {
    match error {
        Error::Download(download_err) => {
            use mp3youtube_core::error::DownloadError;
            match download_err {
                DownloadError::InvalidUrl { .. } => YouTubeErrorCategory::InvalidUrl,
                DownloadError::NotAPlaylist { .. } => YouTubeErrorCategory::InvalidUrl,
                DownloadError::Network { .. } => YouTubeErrorCategory::Network,
                DownloadError::Timeout { .. } => YouTubeErrorCategory::Network,
                DownloadError::RateLimited { .. } => YouTubeErrorCategory::YouTubeService,
                DownloadError::VideoUnavailable { reason, .. } => {
                    let reason_lower = reason.to_lowercase();
                    if reason_lower.contains("age") || reason_lower.contains("sign in") {
                        YouTubeErrorCategory::AgeRestricted
                    } else if reason_lower.contains("country")
                        || reason_lower.contains("region")
                        || reason_lower.contains("geo")
                    {
                        YouTubeErrorCategory::GeoRestricted
                    } else {
                        YouTubeErrorCategory::VideoUnavailable
                    }
                }
                DownloadError::AudioExtractionFailed { .. } => {
                    YouTubeErrorCategory::AudioExtraction
                }
                DownloadError::ConversionFailed { .. } => YouTubeErrorCategory::AudioExtraction,
                DownloadError::PlaylistParseFailed { reason, .. } => {
                    let reason_lower = reason.to_lowercase();
                    if reason_lower.contains("not found")
                        || reason_lower.contains("404")
                        || reason_lower.contains("private")
                    {
                        YouTubeErrorCategory::PlaylistNotFound
                    } else if reason_lower.contains("network")
                        || reason_lower.contains("connection")
                        || reason_lower.contains("timeout")
                    {
                        YouTubeErrorCategory::Network
                    } else {
                        YouTubeErrorCategory::YouTubeService
                    }
                }
                DownloadError::Cancelled => YouTubeErrorCategory::Cancelled,
            }
        }
        Error::FileSystem(_) => YouTubeErrorCategory::FileSystem,
        _ => YouTubeErrorCategory::Unknown,
    }
}

/// Classify an error message string into a category (for cases where we only have the string).
#[allow(dead_code)]
fn classify_error_message(message: &str) -> YouTubeErrorCategory {
    let msg_lower = message.to_lowercase();

    if msg_lower.contains("network")
        || msg_lower.contains("connection")
        || msg_lower.contains("timeout")
        || msg_lower.contains("dns")
        || msg_lower.contains("unreachable")
    {
        YouTubeErrorCategory::Network
    } else if msg_lower.contains("rate limit")
        || msg_lower.contains("429")
        || msg_lower.contains("too many requests")
    {
        YouTubeErrorCategory::YouTubeService
    } else if msg_lower.contains("invalid url") || msg_lower.contains("malformed") {
        YouTubeErrorCategory::InvalidUrl
    } else if msg_lower.contains("playlist")
        && (msg_lower.contains("not found")
            || msg_lower.contains("404")
            || msg_lower.contains("private"))
    {
        YouTubeErrorCategory::PlaylistNotFound
    } else if msg_lower.contains("video unavailable")
        || msg_lower.contains("private video")
        || msg_lower.contains("deleted")
    {
        YouTubeErrorCategory::VideoUnavailable
    } else if msg_lower.contains("age")
        || msg_lower.contains("sign in")
        || msg_lower.contains("login")
    {
        YouTubeErrorCategory::AgeRestricted
    } else if msg_lower.contains("country")
        || msg_lower.contains("region")
        || msg_lower.contains("geo")
    {
        YouTubeErrorCategory::GeoRestricted
    } else if msg_lower.contains("extract")
        || msg_lower.contains("audio")
        || msg_lower.contains("stream")
        || msg_lower.contains("format")
    {
        YouTubeErrorCategory::AudioExtraction
    } else if msg_lower.contains("permission")
        || msg_lower.contains("disk")
        || msg_lower.contains("space")
        || msg_lower.contains("write")
    {
        YouTubeErrorCategory::FileSystem
    } else if msg_lower.contains("cancel") {
        YouTubeErrorCategory::Cancelled
    } else {
        YouTubeErrorCategory::Unknown
    }
}

/// Download result payload for completion events.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadResultPayload {
    /// Task ID for this download operation.
    pub task_id: TaskId,
    /// Whether the overall download was successful.
    pub success: bool,
    /// Number of videos successfully downloaded.
    pub successful_count: usize,
    /// Number of videos that failed.
    pub failed_count: usize,
    /// Number of videos skipped (already exist).
    pub skipped_count: usize,
    /// Total number of videos in the playlist.
    pub total_count: usize,
    /// Individual video results.
    pub results: Vec<VideoDownloadResult>,
    /// Error message if the overall operation failed.
    pub error_message: Option<String>,
    /// Category of error for UI display (if failed).
    pub error_category: Option<YouTubeErrorCategory>,
    /// User-friendly error title.
    pub error_title: Option<String>,
    /// User-friendly error description with suggested action.
    pub error_description: Option<String>,
}

/// Result of downloading a single video.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VideoDownloadResult {
    /// Video ID.
    pub video_id: String,
    /// Video title.
    pub title: String,
    /// Whether the download was successful.
    pub success: bool,
    /// Output file path (if successful).
    pub output_path: Option<String>,
    /// Error message (if failed).
    pub error: Option<String>,
}

/// Check if yt-dlp is available on the system.
///
/// Check if the downloader is available.
/// This always returns success since we use a pure Rust implementation.
#[tauri::command]
pub fn check_yt_dlp_available() -> std::result::Result<String, String> {
    info!("Checking downloader availability (pure Rust - always available)");
    Ok("rusty_ytdl (pure Rust)".to_string())
}

/// Fetch playlist information from a `YouTube` URL.
///
/// This retrieves metadata about a `YouTube` playlist including:
/// - Playlist title
/// - Number of videos
/// - Video titles and durations
///
/// This does not download any audio, just retrieves the playlist information.
#[tauri::command]
pub async fn fetch_youtube_playlist_info(url: String) -> std::result::Result<PlaylistInfo, String> {
    info!("Fetching playlist info for URL: {}", url);

    // Run in a blocking task since rusty_ytdl blocking API is synchronous
    let result = tokio::task::spawn_blocking(move || {
        let downloader = RustyYtdlDownloader::new();
        downloader.parse_playlist_url(&url)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    result.map_err(map_err)
}

/// Download a `YouTube` playlist as MP3 files.
///
/// Downloads all videos from a `YouTube` playlist, extracts audio,
/// and converts to MP3 format with 192kbps quality.
///
/// Progress events are emitted to "youtube-download-progress".
/// Completion events are emitted to "youtube-download-completed",
/// "youtube-download-failed", or "youtube-download-cancelled".
///
/// Parameters:
/// - `url`: `YouTube` playlist URL
/// - `output_dir`: Directory to save MP3 files to
/// - `audio_quality`: Audio quality/bitrate (default: "192" for 192kbps)
/// - `embed_thumbnail`: Whether to embed thumbnail in MP3 (default: true)
///
/// Returns the task ID that can be used to track the download.
#[tauri::command]
pub async fn download_youtube_playlist(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    output_dir: String,
    audio_quality: Option<String>,
    embed_thumbnail: Option<bool>,
) -> std::result::Result<TaskId, String> {
    info!(
        "Starting YouTube playlist download: {} -> {}",
        url, output_dir
    );

    // Validate the URL first
    let validation = validate_youtube_url(&url);
    if !validation.is_valid {
        return Err(validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string()));
    }

    let output_path = PathBuf::from(&output_dir);

    // Create output directory if it doesn't exist
    if !output_path.exists() {
        std::fs::create_dir_all(&output_path)
            .map_err(|e| format!("Failed to create output directory: {e}"))?;
    }

    // Configure the downloader
    let config = RustyYtdlConfig::default();
    // Note: audio_quality and embed_thumbnail are not used in pure Rust implementation
    // as it downloads the best available audio stream directly
    let _ = audio_quality;
    let _ = embed_thumbnail;
    let _ = config; // Config is used when creating the downloader

    // Generate task ID
    let task_id = state.runtime().spawn(
        TaskCategory::Download,
        Some(format!("Download playlist: {url}")),
        async {},
    );

    // Clone for the async task
    let url_clone = url;
    let app_handle = app;

    // Spawn the download task in a separate OS thread to avoid nested runtime issues
    // rusty_ytdl internally creates its own tokio runtime, which conflicts with Tauri's runtime
    std::thread::spawn(move || {
        let downloader = RustyYtdlDownloader::with_config(config);

        // Emit download started event
        if let Err(e) = app_handle.emit(youtube_events::DOWNLOAD_STARTED, &task_id) {
            error!("Failed to emit download-started event: {}", e);
        }

        // First, fetch playlist info
        let playlist_info = match downloader.parse_playlist_url(&url_clone) {
            Ok(info) => info,
            Err(e) => {
                error!("Failed to parse playlist: {}", e);
                let category = classify_error(&e);
                let payload = DownloadResultPayload {
                    task_id,
                    success: false,
                    successful_count: 0,
                    failed_count: 0,
                    skipped_count: 0,
                    total_count: 0,
                    results: vec![],
                    error_message: Some(e.to_string()),
                    error_category: Some(category),
                    error_title: Some(category.title().to_string()),
                    error_description: Some(category.description().to_string()),
                };
                if let Err(emit_err) = app_handle.emit(youtube_events::DOWNLOAD_FAILED, &payload) {
                    error!("Failed to emit download-failed event: {}", emit_err);
                }
                return;
            }
        };

        info!(
            "Playlist '{}' has {} videos",
            playlist_info.title, playlist_info.video_count
        );

        // Set up progress callback
        let app_handle_for_progress = app_handle.clone();
        let progress_callback = move |progress: DownloadProgress| {
            let payload = DownloadProgressPayload::from_progress(task_id, &progress);
            if let Err(e) =
                app_handle_for_progress.emit(youtube_events::DOWNLOAD_PROGRESS, &payload)
            {
                error!("Failed to emit download-progress event: {}", e);
            }
        };

        // Download the playlist
        let results = match downloader.download_playlist(
            &playlist_info,
            &output_path,
            Some(Box::new(progress_callback)),
        ) {
            Ok(results) => results,
            Err(e) => {
                error!("Download failed: {}", e);
                let category = classify_error(&e);

                // Check if it was cancelled
                let event = if matches!(
                    e,
                    Error::Download(mp3youtube_core::error::DownloadError::Cancelled)
                ) {
                    youtube_events::DOWNLOAD_CANCELLED
                } else {
                    youtube_events::DOWNLOAD_FAILED
                };

                let payload = DownloadResultPayload {
                    task_id,
                    success: false,
                    successful_count: 0,
                    failed_count: 0,
                    skipped_count: 0,
                    total_count: playlist_info.video_count,
                    results: vec![],
                    error_message: Some(e.to_string()),
                    error_category: Some(category),
                    error_title: Some(category.title().to_string()),
                    error_description: Some(category.description().to_string()),
                };
                if let Err(emit_err) = app_handle.emit(event, &payload) {
                    error!("Failed to emit {} event: {}", event, emit_err);
                }
                return;
            }
        };

        // Process results
        let successful_count = results.iter().filter(|r| r.success).count();
        let failed_count = results
            .iter()
            .filter(|r| !r.success && r.error.is_some())
            .count();
        let skipped_count = results.len() - successful_count - failed_count;

        let video_results: Vec<VideoDownloadResult> = results
            .iter()
            .map(|r| VideoDownloadResult {
                video_id: r.video.id.clone(),
                title: r.video.title.clone(),
                success: r.success,
                output_path: r.output_path.as_ref().map(|p| p.display().to_string()),
                error: r.error.clone(),
            })
            .collect();

        let payload = DownloadResultPayload {
            task_id,
            success: failed_count == 0,
            successful_count,
            failed_count,
            skipped_count,
            total_count: results.len(),
            results: video_results,
            error_message: None,
            error_category: None,
            error_title: None,
            error_description: None,
        };

        if failed_count == 0 {
            info!(
                "Download completed: {} successful, {} skipped",
                successful_count, skipped_count
            );
        } else {
            info!(
                "Download completed with errors: {} successful, {} failed, {} skipped",
                successful_count, failed_count, skipped_count
            );
        }

        if let Err(e) = app_handle.emit(youtube_events::DOWNLOAD_COMPLETED, &payload) {
            error!("Failed to emit download completed event: {}", e);
        }
    });

    info!("Download task {} spawned successfully", task_id);
    Ok(task_id)
}

/// Download a `YouTube` playlist directly to a local playlist folder.
///
/// This is a convenience command that:
/// 1. Creates the playlist folder if it doesn't exist
/// 2. Downloads all videos as MP3 to that folder
/// 3. Updates the playlist metadata (including thumbnail)
///
/// Parameters:
/// - `url`: `YouTube` playlist URL
/// - `playlist_name`: Name of the local playlist to download to
///
/// Returns the task ID that can be used to track the download.
/// The download runs in the background and emits progress events.
#[tauri::command]
pub async fn download_youtube_to_playlist(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    playlist_name: String,
) -> std::result::Result<TaskId, String> {
    info!(
        "Downloading YouTube playlist to local playlist: {} -> {}",
        url, playlist_name
    );

    // Validate the URL first
    let validation = validate_youtube_url(&url);
    if !validation.is_valid {
        return Err(validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string()));
    }

    // Get the playlist directory path
    let playlist_manager = state.playlist_manager.read().await;
    let playlist_path = playlist_manager.base_path().join(&playlist_name);
    let _base_path = playlist_manager.base_path().to_path_buf();
    drop(playlist_manager);

    // Create output directory if it doesn't exist
    if !playlist_path.exists() {
        std::fs::create_dir_all(&playlist_path)
            .map_err(|e| format!("Failed to create playlist directory: {e}"))?;
    }

    // Generate task ID without spawning (avoids nested runtime issues)
    let task_id = state.runtime().generate_task_id();

    // Clone values for the background task
    let url_clone = url.clone();
    let playlist_name_clone = playlist_name.clone();
    let app_handle = app.clone();
    let output_path = playlist_path;

    // Spawn the download as a completely detached background task
    // This avoids the nested runtime issue
    std::thread::spawn(move || {
        let config = RustyYtdlConfig::default();
        let downloader = RustyYtdlDownloader::with_config(config);

        // Emit download started event
        if let Err(e) = app_handle.emit(youtube_events::DOWNLOAD_STARTED, &task_id) {
            error!("Failed to emit download-started event: {}", e);
        }

        // First, fetch playlist info
        let playlist_info = match downloader.parse_playlist_url(&url_clone) {
            Ok(info) => info,
            Err(e) => {
                error!("Failed to parse playlist: {}", e);
                let category = classify_error(&e);
                let payload = DownloadResultPayload {
                    task_id,
                    success: false,
                    successful_count: 0,
                    failed_count: 0,
                    skipped_count: 0,
                    total_count: 0,
                    results: vec![],
                    error_message: Some(e.to_string()),
                    error_category: Some(category),
                    error_title: Some(category.title().to_string()),
                    error_description: Some(category.description().to_string()),
                };
                if let Err(emit_err) = app_handle.emit(youtube_events::DOWNLOAD_FAILED, &payload) {
                    error!("Failed to emit download-failed event: {}", emit_err);
                }
                return;
            }
        };

        info!(
            "Playlist '{}' has {} videos",
            playlist_info.title, playlist_info.video_count
        );

        // Update playlist metadata with source_url and thumbnail URL
        let playlist_json_path = output_path.join("playlist.json");
        if playlist_json_path.exists()
            && let Ok(content) = std::fs::read_to_string(&playlist_json_path)
            && let Ok(mut metadata) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(obj) = metadata.as_object_mut()
        {
            // Always set the source_url to the YouTube playlist URL
            obj.insert("source_url".to_string(), serde_json::json!(&url_clone));

            // Set thumbnail if available
            if let Some(thumb) = &playlist_info.thumbnail_url {
                obj.insert("thumbnail_url".to_string(), serde_json::json!(thumb));
            }

            if let Ok(updated) = serde_json::to_string_pretty(&metadata) {
                let _ = std::fs::write(&playlist_json_path, updated);
            }
        }

        // Set up progress callback
        let app_handle_for_progress = app_handle.clone();
        let progress_callback = move |progress: DownloadProgress| {
            let payload = DownloadProgressPayload::from_progress(task_id, &progress);
            if let Err(e) =
                app_handle_for_progress.emit(youtube_events::DOWNLOAD_PROGRESS, &payload)
            {
                error!("Failed to emit download-progress event: {}", e);
            }
        };

        // Download the playlist
        let results = match downloader.download_playlist(
            &playlist_info,
            &output_path,
            Some(Box::new(progress_callback)),
        ) {
            Ok(results) => results,
            Err(e) => {
                error!("Download failed: {}", e);
                let category = classify_error(&e);

                // Check if it was cancelled
                let event = if matches!(
                    e,
                    Error::Download(mp3youtube_core::error::DownloadError::Cancelled)
                ) {
                    youtube_events::DOWNLOAD_CANCELLED
                } else {
                    youtube_events::DOWNLOAD_FAILED
                };

                let payload = DownloadResultPayload {
                    task_id,
                    success: false,
                    successful_count: 0,
                    failed_count: 0,
                    skipped_count: 0,
                    total_count: playlist_info.video_count,
                    results: vec![],
                    error_message: Some(e.to_string()),
                    error_category: Some(category),
                    error_title: Some(category.title().to_string()),
                    error_description: Some(category.description().to_string()),
                };
                if let Err(emit_err) = app_handle.emit(event, &payload) {
                    error!("Failed to emit {} event: {}", event, emit_err);
                }
                return;
            }
        };

        // Process results
        let successful_count = results.iter().filter(|r| r.success).count();
        let failed_count = results
            .iter()
            .filter(|r| !r.success && r.error.is_some())
            .count();
        let skipped_count = results.len() - successful_count - failed_count;

        let video_results: Vec<VideoDownloadResult> = results
            .iter()
            .map(|r| VideoDownloadResult {
                video_id: r.video.id.clone(),
                title: r.video.title.clone(),
                success: r.success,
                output_path: r.output_path.as_ref().map(|p| p.display().to_string()),
                error: r.error.clone(),
            })
            .collect();

        let payload = DownloadResultPayload {
            task_id,
            success: failed_count == 0,
            successful_count,
            failed_count,
            skipped_count,
            total_count: results.len(),
            results: video_results,
            error_message: None,
            error_category: None,
            error_title: None,
            error_description: None,
        };

        if failed_count == 0 {
            info!(
                "Download to playlist '{}' completed: {} successful, {} skipped",
                playlist_name_clone, successful_count, skipped_count
            );
        } else {
            info!(
                "Download to playlist '{}' completed with errors: {} successful, {} failed, {} skipped",
                playlist_name_clone, successful_count, failed_count, skipped_count
            );
        }

        // Update playlist.json with actual track count and total size
        let playlist_json_path = output_path.join("playlist.json");
        if let Ok(content) = std::fs::read_to_string(&playlist_json_path)
            && let Ok(mut metadata) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(obj) = metadata.as_object_mut()
        {
            // Count audio files and total size
            let mut track_count = 0usize;
            let mut total_size: u64 = 0;
            if let Ok(entries) = std::fs::read_dir(&output_path) {
                for entry in entries.filter_map(std::result::Result::ok) {
                    let path = entry.path();
                    if path.is_file()
                        && let Some(ext) = path.extension().and_then(|e| e.to_str())
                    {
                        let ext_lower = ext.to_lowercase();
                        if matches!(
                            ext_lower.as_str(),
                            "mp3"
                                | "m4a"
                                | "mp4"
                                | "wav"
                                | "flac"
                                | "ogg"
                                | "aac"
                                | "webm"
                                | "opus"
                        ) {
                            track_count += 1;
                            if let Ok(meta) = std::fs::metadata(&path) {
                                total_size += meta.len();
                            }
                        }
                    }
                }
            }

            obj.insert("track_count".to_string(), serde_json::json!(track_count));
            obj.insert(
                "total_size_bytes".to_string(),
                serde_json::json!(total_size),
            );
            obj.insert(
                "modified_at".to_string(),
                serde_json::json!(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs())
                ),
            );

            if let Ok(updated) = serde_json::to_string_pretty(&metadata) {
                let _ = std::fs::write(&playlist_json_path, updated);
            }
            info!(
                "Updated playlist.json: {} tracks, {} bytes",
                track_count, total_size
            );
        }

        if let Err(e) = app_handle.emit(youtube_events::DOWNLOAD_COMPLETED, &payload) {
            error!("Failed to emit download-completed event: {}", e);
        }
    });

    info!(
        "Download task {} spawned for playlist '{}'",
        task_id, playlist_name
    );
    Ok(task_id)
}

/// Perform a synchronous sync operation (blocking).
///
/// This is useful for simple syncs where you don't need progress tracking.
/// For progress tracking, use `start_orchestrated_sync` instead.
#[tauri::command]
pub async fn sync_playlists_to_device(
    app: AppHandle,
    state: State<'_, AppState>,
    playlists: Vec<String>,
    device_mount_point: String,
    options: SyncOptions,
) -> std::result::Result<CoreSyncResult, String> {
    info!(
        "Syncing {} playlist(s) to device '{}' (synchronous)",
        playlists.len(),
        device_mount_point
    );

    if playlists.is_empty() {
        return Err(map_err(Error::Configuration(
            "No playlists specified for sync".to_string(),
        )));
    }

    let mount_point = PathBuf::from(&device_mount_point);

    // Verify device is accessible
    if !mount_point.exists() || !mount_point.is_dir() {
        return Err(map_err(Error::device_not_found(&device_mount_point)));
    }

    let orchestrator = SyncOrchestrator::new();
    let request = SyncRequest::new(playlists, mount_point);

    // Set up progress callback
    let app_handle = app.clone();
    let progress_callback = move |progress: &SyncProgress| {
        if let Err(e) = app_handle.emit(
            sync_orchestrator_events::SYNC_ORCHESTRATOR_PROGRESS,
            progress,
        ) {
            error!("Failed to emit sync-orchestrator-progress event: {}", e);
        }
    };

    let playlist_mgr = state.playlist_manager.read().await;
    let device_mgr = state.device_manager.read().await;

    orchestrator
        .sync(
            &playlist_mgr,
            &*device_mgr,
            request,
            &options,
            Some(progress_callback),
        )
        .map_err(map_err)
}

// =============================================================================
// Cache Commands
// =============================================================================

use mp3youtube_core::cache::{
    CacheCleanupStats, CacheConfig, CacheManager, CacheStats, default_cache_directory,
};

/// Get cache statistics.
///
/// Returns information about the current state of the cache including:
/// - Total number of entries
/// - Size breakdown by type
/// - Cache usage percentage
#[tauri::command]
pub async fn get_cache_stats(
    state: State<'_, AppState>,
) -> std::result::Result<CacheStats, String> {
    debug!("Getting cache statistics");

    let config_manager = state.config_manager.read().await;
    let cache_config = config_manager.config().cache.clone();
    drop(config_manager);

    let cache = CacheManager::new(cache_config).map_err(map_err)?;
    Ok(cache.stats())
}

/// Get the cache configuration.
#[tauri::command]
pub async fn get_cache_config(
    state: State<'_, AppState>,
) -> std::result::Result<CacheConfig, String> {
    debug!("Getting cache configuration");

    let config_manager = state.config_manager.read().await;
    Ok(config_manager.config().cache.clone())
}

/// Update the cache configuration.
#[tauri::command]
pub async fn update_cache_config(
    state: State<'_, AppState>,
    config: CacheConfig,
) -> std::result::Result<(), String> {
    info!("Updating cache configuration");

    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.cache = config;
    config_manager.update(app_config).map_err(map_err)
}

/// Clean up the cache.
///
/// Removes expired entries and enforces size limits.
/// Returns statistics about what was cleaned up.
#[tauri::command]
pub async fn cleanup_cache(
    state: State<'_, AppState>,
) -> std::result::Result<CacheCleanupStats, String> {
    info!("Running cache cleanup");

    let config_manager = state.config_manager.read().await;
    let cache_config = config_manager.config().cache.clone();
    drop(config_manager);

    let mut cache = CacheManager::new(cache_config).map_err(map_err)?;
    cache.cleanup().map_err(map_err)
}

/// Clear all cached data.
///
/// Removes all entries from the cache. Use with caution.
#[tauri::command]
pub async fn clear_cache(
    state: State<'_, AppState>,
) -> std::result::Result<CacheCleanupStats, String> {
    info!("Clearing all cache data");

    let config_manager = state.config_manager.read().await;
    let cache_config = config_manager.config().cache.clone();
    drop(config_manager);

    let mut cache = CacheManager::new(cache_config).map_err(map_err)?;
    cache.clear().map_err(map_err)
}

/// Clean up temporary files.
///
/// Removes all files from the cache temp directory.
#[tauri::command]
pub async fn cleanup_cache_temp(
    state: State<'_, AppState>,
) -> std::result::Result<CacheCleanupStats, String> {
    info!("Cleaning up cache temp files");

    let config_manager = state.config_manager.read().await;
    let cache_config = config_manager.config().cache.clone();
    drop(config_manager);

    let mut cache = CacheManager::new(cache_config).map_err(map_err)?;
    cache.cleanup_temp().map_err(map_err)
}

/// Get the default cache directory path.
#[tauri::command]
pub fn get_default_cache_directory() -> String {
    default_cache_directory().display().to_string()
}

/// Check if caching is enabled.
#[tauri::command]
pub async fn is_cache_enabled(state: State<'_, AppState>) -> std::result::Result<bool, String> {
    let config_manager = state.config_manager.read().await;
    Ok(config_manager.config().cache.enabled)
}

/// Enable or disable caching.
#[tauri::command]
pub async fn set_cache_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> std::result::Result<(), String> {
    info!("Setting cache enabled: {}", enabled);

    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.cache.enabled = enabled;
    config_manager.update(app_config).map_err(map_err)
}

/// Set the maximum cache size in bytes.
#[tauri::command]
pub async fn set_cache_max_size(
    state: State<'_, AppState>,
    max_size_bytes: u64,
) -> std::result::Result<(), String> {
    info!("Setting cache max size: {} bytes", max_size_bytes);

    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.cache.max_size_bytes = max_size_bytes;
    config_manager.update(app_config).map_err(map_err)
}

// =============================================================================
// Download Queue Commands
// =============================================================================

/// Event names for download queue events emitted to the frontend.
pub mod queue_events {
    /// Event emitted when a queue item is added.
    pub const QUEUE_ITEM_ADDED: &str = "queue-item-added";
    /// Event emitted when a queue item starts downloading.
    pub const QUEUE_ITEM_STARTED: &str = "queue-item-started";
    /// Event emitted when a queue item's progress updates.
    pub const QUEUE_ITEM_PROGRESS: &str = "queue-item-progress";
    /// Event emitted when a queue item completes.
    pub const QUEUE_ITEM_COMPLETED: &str = "queue-item-completed";
    /// Event emitted when a queue item fails.
    pub const QUEUE_ITEM_FAILED: &str = "queue-item-failed";
    /// Event emitted when a queue item is cancelled.
    pub const QUEUE_ITEM_CANCELLED: &str = "queue-item-cancelled";
    /// Event emitted when a queue item is removed.
    pub const QUEUE_ITEM_REMOVED: &str = "queue-item-removed";
    /// Event emitted when the queue is paused.
    pub const QUEUE_PAUSED: &str = "queue-paused";
    /// Event emitted when the queue is resumed.
    pub const QUEUE_RESUMED: &str = "queue-resumed";
    /// Event emitted when queue configuration changes.
    pub const QUEUE_CONFIG_UPDATED: &str = "queue-config-updated";
}

/// Serializable request for adding a download to the queue.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AddToQueueRequest {
    /// `YouTube` playlist URL.
    pub url: String,
    /// Output directory for downloaded files.
    pub output_dir: String,
    /// Optional playlist name for display purposes.
    pub playlist_name: Option<String>,
    /// Audio quality setting (e.g., "192", "320").
    pub audio_quality: Option<String>,
    /// Whether to embed thumbnails in MP3 files.
    pub embed_thumbnail: Option<bool>,
    /// Priority level for this download.
    pub priority: Option<String>,
}

impl AddToQueueRequest {
    fn into_download_request(self) -> DownloadRequest {
        let mut request = DownloadRequest::new(self.url, PathBuf::from(self.output_dir));

        if let Some(name) = self.playlist_name {
            request = request.with_playlist_name(name);
        }
        if let Some(quality) = self.audio_quality {
            request = request.with_audio_quality(quality);
        }
        if let Some(embed) = self.embed_thumbnail {
            request = request.with_embed_thumbnail(embed);
        }
        if let Some(priority) = self.priority {
            let priority = match priority.to_lowercase().as_str() {
                "high" => DownloadPriority::High,
                "low" => DownloadPriority::Low,
                _ => DownloadPriority::Normal,
            };
            request = request.with_priority(priority);
        }

        request
    }
}

/// Add a download request to the queue.
///
/// Returns the queue item ID for tracking.
#[tauri::command]
pub async fn queue_add_download(
    app: AppHandle,
    state: State<'_, AppState>,
    request: AddToQueueRequest,
) -> std::result::Result<QueueItemId, String> {
    info!("Adding download to queue: {}", request.url);

    // Validate the URL first
    let validation = validate_youtube_url(&request.url);
    if !validation.is_valid {
        return Err(validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string()));
    }

    let download_request = request.into_download_request();
    let queue = state.download_queue_arc();
    let item_id = queue.add(download_request).await;

    // Emit event for frontend
    if let Some(item) = queue.get_item(item_id).await
        && let Err(e) = app.emit(queue_events::QUEUE_ITEM_ADDED, &item)
    {
        error!("Failed to emit queue-item-added event: {}", e);
    }

    // Try to start the next download if auto-start is enabled
    process_queue(app.clone(), state.clone()).await;

    Ok(item_id)
}

/// Add a download request to a specific local playlist.
///
/// This creates the playlist folder if needed and queues the download.
#[tauri::command]
pub async fn queue_add_to_playlist(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    playlist_name: String,
    priority: Option<String>,
) -> std::result::Result<QueueItemId, String> {
    info!(
        "Adding download to queue for playlist '{}': {}",
        playlist_name, url
    );

    // Validate the URL first
    let validation = validate_youtube_url(&url);
    if !validation.is_valid {
        return Err(validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string()));
    }

    // Get the playlist directory
    let playlist_manager = state.playlist_manager.read().await;
    let playlist_path = playlist_manager.base_path().join(&playlist_name);

    // Create the playlist if it doesn't exist
    if !playlist_path.exists() {
        playlist_manager
            .create_playlist(&playlist_name, Some(url.clone()))
            .map_err(map_err)?;
    }

    drop(playlist_manager);

    let request = AddToQueueRequest {
        url,
        output_dir: playlist_path.display().to_string(),
        playlist_name: Some(playlist_name),
        audio_quality: None,
        embed_thumbnail: None,
        priority,
    };

    queue_add_download(app, state, request).await
}

/// Add multiple download requests to the queue at once.
///
/// Returns a vector of queue item IDs.
#[tauri::command]
pub async fn queue_add_batch(
    app: AppHandle,
    state: State<'_, AppState>,
    requests: Vec<AddToQueueRequest>,
) -> std::result::Result<Vec<QueueItemId>, String> {
    info!("Adding {} downloads to queue (batch)", requests.len());

    // Validate all URLs first
    for request in &requests {
        let validation = validate_youtube_url(&request.url);
        if !validation.is_valid {
            return Err(format!(
                "Invalid URL '{}': {}",
                request.url,
                validation
                    .error_message
                    .unwrap_or_else(|| "Invalid".to_string())
            ));
        }
    }

    let download_requests: Vec<DownloadRequest> = requests
        .into_iter()
        .map(AddToQueueRequest::into_download_request)
        .collect();

    let queue = state.download_queue_arc();
    let item_ids = queue.add_batch(download_requests).await;

    // Emit events for each added item
    for &item_id in &item_ids {
        if let Some(item) = queue.get_item(item_id).await
            && let Err(e) = app.emit(queue_events::QUEUE_ITEM_ADDED, &item)
        {
            error!("Failed to emit queue-item-added event: {}", e);
        }
    }

    // Try to start downloads
    process_queue(app.clone(), state.clone()).await;

    Ok(item_ids)
}

/// Remove an item from the queue.
///
/// Only pending or finished items can be removed.
#[tauri::command]
pub async fn queue_remove_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<bool, String> {
    info!("Removing item {} from queue", item_id);

    let queue = state.download_queue_arc();
    let removed = queue.remove(item_id).await;

    if removed && let Err(e) = app.emit(queue_events::QUEUE_ITEM_REMOVED, &item_id) {
        error!("Failed to emit queue-item-removed event: {}", e);
    }

    Ok(removed)
}

/// Cancel a downloading or pending item.
#[tauri::command]
pub async fn queue_cancel_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<bool, String> {
    info!("Cancelling queue item {}", item_id);

    let queue = state.download_queue_arc();
    let cancelled = queue.cancel(item_id).await;

    if cancelled {
        if let Err(e) = app.emit(queue_events::QUEUE_ITEM_CANCELLED, &item_id) {
            error!("Failed to emit queue-item-cancelled event: {}", e);
        }

        // Process queue to start next item
        process_queue(app.clone(), state.clone()).await;
    }

    Ok(cancelled)
}

/// Update the priority of a queue item.
#[tauri::command]
pub async fn queue_set_priority(
    state: State<'_, AppState>,
    item_id: QueueItemId,
    priority: String,
) -> std::result::Result<bool, String> {
    info!("Setting priority of item {} to {}", item_id, priority);

    let priority = match priority.to_lowercase().as_str() {
        "high" => DownloadPriority::High,
        "low" => DownloadPriority::Low,
        _ => DownloadPriority::Normal,
    };

    let queue = state.download_queue_arc();
    Ok(queue.set_priority(item_id, priority).await)
}

/// Move an item to the front of the queue (high priority).
#[tauri::command]
pub async fn queue_move_to_front(
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<bool, String> {
    info!("Moving item {} to front of queue", item_id);

    let queue = state.download_queue_arc();
    Ok(queue.move_to_front(item_id).await)
}

/// Retry a failed queue item.
#[tauri::command]
pub async fn queue_retry_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<bool, String> {
    info!("Retrying queue item {}", item_id);

    let queue = state.download_queue_arc();
    let retried = queue.retry(item_id).await;

    if retried {
        // Process queue to potentially start this item
        process_queue(app.clone(), state.clone()).await;
    }

    Ok(retried)
}

/// Get a specific queue item.
#[tauri::command]
pub async fn queue_get_item(
    state: State<'_, AppState>,
    item_id: QueueItemId,
) -> std::result::Result<Option<QueueItem>, String> {
    let queue = state.download_queue_arc();
    Ok(queue.get_item(item_id).await)
}

/// Get all items in the queue.
#[tauri::command]
pub async fn queue_get_all_items(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<QueueItem>, String> {
    let queue = state.download_queue_arc();
    Ok(queue.get_all_items().await)
}

/// Get all pending items in the queue, sorted by priority.
#[tauri::command]
pub async fn queue_get_pending_items(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<QueueItem>, String> {
    let queue = state.download_queue_arc();
    Ok(queue.get_pending_items().await)
}

/// Get all currently downloading items.
#[tauri::command]
pub async fn queue_get_downloading_items(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<QueueItem>, String> {
    let queue = state.download_queue_arc();
    Ok(queue.get_downloading_items().await)
}

/// Get queue statistics.
#[tauri::command]
pub async fn queue_get_stats(
    state: State<'_, AppState>,
) -> std::result::Result<QueueStats, String> {
    let queue = state.download_queue_arc();
    Ok(queue.stats().await)
}

/// Pause the queue (stop starting new downloads).
#[tauri::command]
pub async fn queue_pause(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    info!("Pausing download queue");

    let queue = state.download_queue_arc();
    queue.pause().await;

    if let Err(e) = app.emit(queue_events::QUEUE_PAUSED, &()) {
        error!("Failed to emit queue-paused event: {}", e);
    }

    Ok(())
}

/// Resume the queue (allow starting new downloads).
#[tauri::command]
pub async fn queue_resume(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    info!("Resuming download queue");

    let queue = state.download_queue_arc();
    queue.resume().await;

    if let Err(e) = app.emit(queue_events::QUEUE_RESUMED, &()) {
        error!("Failed to emit queue-resumed event: {}", e);
    }

    // Process queue to start downloads
    process_queue(app.clone(), state.clone()).await;

    Ok(())
}

/// Check if the queue is paused.
#[tauri::command]
pub async fn queue_is_paused(state: State<'_, AppState>) -> std::result::Result<bool, String> {
    let queue = state.download_queue_arc();
    Ok(queue.is_paused().await)
}

/// Clear all finished items from the queue.
#[tauri::command]
pub async fn queue_clear_finished(
    state: State<'_, AppState>,
) -> std::result::Result<usize, String> {
    info!("Clearing finished items from queue");

    let queue = state.download_queue_arc();
    Ok(queue.clear_finished().await)
}

/// Clear all non-downloading items from the queue.
#[tauri::command]
pub async fn queue_clear_all(state: State<'_, AppState>) -> std::result::Result<usize, String> {
    info!("Clearing all items from queue");

    let queue = state.download_queue_arc();
    Ok(queue.clear_all().await)
}

/// Get the queue configuration.
#[tauri::command]
pub async fn queue_get_config(
    state: State<'_, AppState>,
) -> std::result::Result<QueueConfig, String> {
    let queue = state.download_queue_arc();
    Ok(queue.config().await)
}

/// Update the queue configuration.
#[tauri::command]
pub async fn queue_set_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: QueueConfig,
) -> std::result::Result<(), String> {
    info!("Updating queue configuration");

    let queue = state.download_queue_arc();
    queue.set_config(config.clone()).await;

    // Also persist to app config
    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.queue = config.clone();
    config_manager.update(app_config).map_err(map_err)?;

    if let Err(e) = app.emit(queue_events::QUEUE_CONFIG_UPDATED, &config) {
        error!("Failed to emit queue-config-updated event: {}", e);
    }

    // Process queue in case more downloads can start
    drop(config_manager);
    process_queue(app.clone(), state.clone()).await;

    Ok(())
}

/// Set the maximum number of concurrent downloads.
#[tauri::command]
pub async fn queue_set_max_concurrent(
    app: AppHandle,
    state: State<'_, AppState>,
    max_concurrent: usize,
) -> std::result::Result<(), String> {
    info!("Setting max concurrent downloads to {}", max_concurrent);

    let queue = state.download_queue_arc();
    queue.set_max_concurrent(max_concurrent).await;

    // Also persist to app config
    let new_config = queue.config().await;
    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.queue = new_config;
    config_manager.update(app_config).map_err(map_err)?;

    // Process queue in case more downloads can start
    drop(config_manager);
    process_queue(app.clone(), state.clone()).await;

    Ok(())
}

/// Internal function to process the queue and start downloads.
async fn process_queue(app: AppHandle, state: State<'_, AppState>) {
    let queue = state.download_queue_arc();

    // Keep starting downloads while we can
    while queue.can_start_download().await {
        if let Some(item) = queue.start_next().await {
            // Spawn the download task
            let app_clone = app.clone();
            let queue_clone = Arc::clone(&queue);
            let item_id = item.id;

            // Get config for audio quality
            let config_manager = state.config_manager.read().await;
            let download_quality = config_manager.config().download_quality;
            drop(config_manager);

            let audio_quality =
                item.request
                    .audio_quality
                    .clone()
                    .unwrap_or_else(|| match download_quality {
                        mp3youtube_core::config::DownloadQuality::Low => "128".to_string(),
                        mp3youtube_core::config::DownloadQuality::Medium => "192".to_string(),
                        mp3youtube_core::config::DownloadQuality::High => "320".to_string(),
                    });

            let embed_thumbnail = item.request.embed_thumbnail.unwrap_or(true);
            let url = item.request.url.clone();
            let output_dir = item.request.output_dir.clone();

            // Spawn the download as a task
            let task_id = state.runtime().spawn(
                TaskCategory::Download,
                Some(format!("Queue download: {}", item.display_name())),
                async move {
                    // Mark as started with task ID
                    queue_clone.mark_started(item_id, 0).await;

                    if let Err(e) = app_clone.emit(queue_events::QUEUE_ITEM_STARTED, &serde_json::json!({
                        "item_id": item_id,
                        "task_id": 0
                    })) {
                        error!("Failed to emit queue-item-started event: {}", e);
                    }

                    // Configure the downloader (pure Rust implementation)
                    let config = RustyYtdlConfig::default();
                    // Note: audio_quality and embed_thumbnail not used in pure Rust impl
                    let _ = audio_quality;
                    let _ = embed_thumbnail;

                    let downloader = RustyYtdlDownloader::with_config(config);

                    // First, fetch playlist info
                    let playlist_info = match downloader.parse_playlist_url(&url) {
                        Ok(info) => info,
                        Err(e) => {
                            error!("Failed to parse playlist for queue item {}: {}", item_id, e);
                            queue_clone.mark_failed(item_id, e.to_string()).await;
                            if let Err(emit_err) = app_clone.emit(queue_events::QUEUE_ITEM_FAILED, &serde_json::json!({
                                "item_id": item_id,
                                "error": e.to_string()
                            })) {
                                error!("Failed to emit queue-item-failed event: {}", emit_err);
                            }
                            return;
                        }
                    };

                    // Update total videos count
                    queue_clone.update_progress(
                        item_id,
                        0.0,
                        None,
                        Some(playlist_info.video_count),
                        Some(0),
                    ).await;

                    // Set up progress callback
                    let app_for_progress = app_clone.clone();
                    let queue_for_progress = Arc::clone(&queue_clone);
                    let progress_callback = move |progress: DownloadProgress| {
                        let queue_inner = Arc::clone(&queue_for_progress);
                        let app_inner = app_for_progress.clone();

                        // Update queue item progress
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                queue_inner.update_progress(
                                    item_id,
                                    progress.overall_progress,
                                    Some(progress.current_title.clone()),
                                    Some(progress.total_videos),
                                    Some(progress.videos_completed + progress.videos_skipped),
                                ).await;
                            });
                        });

                        // Emit progress event
                        if let Err(e) = app_inner.emit(queue_events::QUEUE_ITEM_PROGRESS, &serde_json::json!({
                            "item_id": item_id,
                            "progress": progress.overall_progress,
                            "current_video": progress.current_title,
                            "total_videos": progress.total_videos,
                            "videos_completed": progress.videos_completed + progress.videos_skipped,
                        })) {
                            error!("Failed to emit queue-item-progress event: {}", e);
                        }
                    };

                    // Create output directory if needed
                    if let Err(e) = std::fs::create_dir_all(&output_dir) {
                        error!("Failed to create output directory for queue item {}: {}", item_id, e);
                        queue_clone.mark_failed(item_id, format!("Failed to create output directory: {e}")).await;
                        if let Err(emit_err) = app_clone.emit(queue_events::QUEUE_ITEM_FAILED, &serde_json::json!({
                            "item_id": item_id,
                            "error": format!("Failed to create output directory: {}", e)
                        })) {
                            error!("Failed to emit queue-item-failed event: {}", emit_err);
                        }
                        return;
                    }

                    // Download the playlist
                    match downloader.download_playlist(
                        &playlist_info,
                        &output_dir,
                        Some(Box::new(progress_callback)),
                    ) {
                        Ok(_results) => {
                            info!("Queue item {} completed successfully", item_id);
                            queue_clone.mark_completed(item_id).await;
                            if let Err(e) = app_clone.emit(queue_events::QUEUE_ITEM_COMPLETED, &item_id) {
                                error!("Failed to emit queue-item-completed event: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Queue item {} failed: {}", item_id, e);
                            queue_clone.mark_failed(item_id, e.to_string()).await;
                            if let Err(emit_err) = app_clone.emit(queue_events::QUEUE_ITEM_FAILED, &serde_json::json!({
                                "item_id": item_id,
                                "error": e.to_string()
                            })) {
                                error!("Failed to emit queue-item-failed event: {}", emit_err);
                            }
                        }
                    }
                },
            );

            // Update task ID
            queue.mark_started(item_id, task_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> AppState {
        AppState::new().expect("Failed to create state")
    }

    #[test]
    fn test_app_state_creation() {
        let result = AppState::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_app_state_has_runtime() {
        let state = create_test_state();
        // Verify runtime is accessible
        let _runtime = state.runtime();
    }

    #[test]
    fn test_app_state_progress_sender() {
        let state = create_test_state();
        let _sender = state.progress_sender();
    }

    #[test]
    fn test_spawn_task() {
        let state = create_test_state();
        let task_id = state.spawn_task(TaskCategory::Background, Some("test".to_string()), async {
            42
        });
        assert_eq!(task_id, 0);
    }

    #[test]
    fn test_map_err() {
        let error = Error::playlist_not_found("test");
        let result = map_err(error);
        // Result should be JSON
        assert!(result.contains("message"));
        assert!(result.contains("test"));
        assert!(result.contains("kind"));
        assert!(result.contains("Playlist"));
    }

    #[test]
    fn test_error_response_structure() {
        let error = Error::device_not_found("test-device");
        let response = ErrorResponse::from(&error);

        assert!(response.message.contains("test-device"));
        assert_eq!(response.kind, "Device");
        assert!(!response.retryable);
        assert!(response.retry_delay_secs.is_none());
    }

    #[test]
    fn test_retryable_error() {
        let error = Error::network_error("connection reset");
        let response = ErrorResponse::from(&error);

        assert!(response.retryable);
    }

    #[test]
    fn test_get_default_storage_directory() {
        let dir = get_default_storage_directory();
        assert!(!dir.is_empty());
        assert!(dir.contains("mp3youtube"));
    }
}
