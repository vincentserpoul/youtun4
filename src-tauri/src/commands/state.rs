//! Application state managed by Tauri.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::RwLock;
use tracing::{debug, error, info};
use youtun4_core::{
    Error, Result,
    config::ConfigManager,
    device::{DeviceManager, DeviceWatcherHandle, PlatformMountHandler},
    error::DeviceError,
    playlist::PlaylistManager,
    queue::DownloadQueueManager,
};

use crate::runtime::{AsyncRuntime, TaskId, TaskStatus};

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

/// A shared cancellation flag that can be set from any thread.
type CancelFlag = Arc<AtomicBool>;

/// Type alias for sync task storage to reduce complexity.
type SyncTaskMap = HashMap<TaskId, (SyncTaskInfo, CancelFlag)>;

/// Type alias for download task storage (`task_id` -> cancel flag).
type DownloadTaskMap = HashMap<TaskId, CancelFlag>;

/// Application state managed by Tauri.
pub struct AppState {
    /// Configuration manager (async-safe).
    pub(crate) config_manager: Arc<RwLock<ConfigManager>>,
    /// Device manager for detecting USB devices (async-safe).
    pub(crate) device_manager: Arc<RwLock<DeviceManager>>,
    /// Playlist manager for local playlist operations (async-safe).
    pub(crate) playlist_manager: Arc<RwLock<PlaylistManager>>,
    /// Async runtime for spawning and managing tasks.
    pub(crate) runtime: Arc<AsyncRuntime>,
    /// Handle for the device watcher (if running).
    pub(crate) device_watcher_handle: Arc<RwLock<Option<DeviceWatcherHandle>>>,
    /// Mount handler for device mount/unmount operations.
    pub(crate) mount_handler: Arc<PlatformMountHandler>,
    /// Active sync tasks with their cancellation tokens.
    pub(crate) sync_tasks: Arc<RwLock<SyncTaskMap>>,
    /// Active download tasks with their cancellation flags.
    pub(crate) download_tasks: Arc<RwLock<DownloadTaskMap>>,
    /// Download queue manager for handling multiple playlist downloads.
    pub(crate) download_queue: Arc<DownloadQueueManager>,
}

/// Insert a sync task only if no other sync is active (single critical section).
fn try_insert_sync_task(
    tasks: &mut SyncTaskMap,
    task_id: TaskId,
    info: SyncTaskInfo,
    cancel_token: CancelFlag,
) -> Result<()> {
    if !tasks.is_empty() {
        return Err(Error::Device(DeviceError::DeviceBusy {
            mount_point: PathBuf::from(&info.device_mount_point),
            reason: "a sync is already in progress".to_string(),
        }));
    }
    tasks.insert(task_id, (info, cancel_token));
    Ok(())
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
            download_tasks: Arc::new(RwLock::new(HashMap::new())),
            download_queue: Arc::new(download_queue),
        })
    }

    /// Reinitialize the playlist manager with a new directory.
    pub async fn reinitialize_playlist_manager(&self, playlists_dir: PathBuf) -> Result<()> {
        let new_manager = PlaylistManager::new(playlists_dir)?;
        let mut manager = self.playlist_manager.write().await;
        *manager = new_manager;
        Ok(())
    }

    /// Get a reference to the async runtime.
    pub fn runtime(&self) -> &AsyncRuntime {
        &self.runtime
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

    /// Atomically register a sync task, failing if another sync is already active.
    ///
    /// # Errors
    ///
    /// Returns `DeviceError::DeviceBusy` if a sync is already in progress.
    pub async fn try_register_sync_task(
        &self,
        task_id: TaskId,
        info: SyncTaskInfo,
        cancel_token: CancelFlag,
    ) -> Result<()> {
        let mut tasks = self.sync_tasks.write().await;
        try_insert_sync_task(&mut tasks, task_id, info, cancel_token)
    }

    /// Remove a sync task registration (on completion, failure, or cancellation).
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

    /// Register a download task with its cancellation flag.
    pub async fn register_download_task(&self, task_id: TaskId, cancel_flag: CancelFlag) {
        let mut tasks = self.download_tasks.write().await;
        tasks.insert(task_id, cancel_flag);
    }

    /// Unregister a download task (called when download completes or fails).
    #[allow(dead_code, reason = "public API reserved for future use")]
    pub async fn unregister_download_task(&self, task_id: TaskId) {
        let mut tasks = self.download_tasks.write().await;
        tasks.remove(&task_id);
    }

    /// Cancel a download task by task ID.
    pub async fn cancel_download_task(&self, task_id: TaskId) -> bool {
        let tasks = self.download_tasks.read().await;
        if let Some(cancel_flag) = tasks.get(&task_id) {
            cancel_flag.store(true, Ordering::SeqCst);
            info!("Download task {} cancellation requested", task_id);
            true
        } else {
            debug!("Download task {} not found for cancellation", task_id);
            false
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::{SyncTaskInfo, SyncTaskMap, TaskId, try_insert_sync_task};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    /// Build a sample `SyncTaskInfo` for a given task ID.
    fn sample_info(task_id: TaskId) -> SyncTaskInfo {
        SyncTaskInfo {
            task_id,
            playlist_name: "My Playlist".to_string(),
            device_mount_point: "/media/device".to_string(),
            verify_integrity: false,
            skip_existing: false,
        }
    }

    #[test]
    fn insert_into_empty_map_succeeds() {
        let mut tasks: SyncTaskMap = SyncTaskMap::new();
        let result = try_insert_sync_task(
            &mut tasks,
            1,
            sample_info(1),
            Arc::new(AtomicBool::new(false)),
        );

        result.expect("insert into empty map should succeed");
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn second_insert_while_busy_is_rejected() {
        let mut tasks: SyncTaskMap = SyncTaskMap::new();
        try_insert_sync_task(
            &mut tasks,
            1,
            sample_info(1),
            Arc::new(AtomicBool::new(false)),
        )
        .expect("first insert should succeed");

        let result = try_insert_sync_task(
            &mut tasks,
            2,
            sample_info(2),
            Arc::new(AtomicBool::new(false)),
        );

        let err = result.expect_err("second insert should be rejected");
        assert!(
            err.to_string()
                .contains("is busy: a sync is already in progress")
        );
        assert_eq!(tasks.len(), 1);
        assert!(tasks.contains_key(&1));
    }

    #[test]
    fn insert_after_removal_succeeds() {
        let mut tasks: SyncTaskMap = SyncTaskMap::new();
        try_insert_sync_task(
            &mut tasks,
            1,
            sample_info(1),
            Arc::new(AtomicBool::new(false)),
        )
        .expect("first insert should succeed");

        tasks.remove(&1);

        let result = try_insert_sync_task(
            &mut tasks,
            2,
            sample_info(2),
            Arc::new(AtomicBool::new(false)),
        );

        result.expect("insert after removal should succeed");
        assert_eq!(tasks.len(), 1);
        assert!(tasks.contains_key(&2));
    }
}
