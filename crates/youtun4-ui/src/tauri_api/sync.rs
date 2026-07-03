//! Sync API and Sync Orchestrator API.

use crate::types::TaskId;

use super::{invoke, listen_to_event};

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
    #[serde(rename_all = "camelCase")]
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
    #[serde(rename_all = "camelCase")]
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
    #[serde(rename_all = "camelCase")]
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
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
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
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(info) = serde_wasm_bindgen::from_value::<SyncTaskInfo>(payload)
        {
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
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(progress) = serde_wasm_bindgen::from_value::<SyncProgressPayload>(payload)
        {
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
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<SyncResultPayload>(payload)
        {
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
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<SyncResultPayload>(payload)
        {
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
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<SyncResultPayload>(payload)
        {
            handler(result);
        }
    })
    .await
}

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
    #[allow(
        clippy::float_arithmetic,
        reason = "float arithmetic needed for speed unit conversion"
    )]
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
    #[allow(
        clippy::cast_possible_truncation,
        reason = "truncation is acceptable for time display"
    )]
    #[allow(
        clippy::cast_sign_loss,
        reason = "elapsed time values are always non-negative"
    )]
    #[allow(
        clippy::float_arithmetic,
        reason = "float arithmetic needed for time conversion"
    )]
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
    #[serde(rename_all = "camelCase")]
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
