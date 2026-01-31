//! Async runtime configuration for the `Youtun4` application.
//!
//! This module provides utilities for managing the Tokio async runtime,
//! including task spawning, thread pool configuration, and task lifecycle management.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::runtime::{Builder, Runtime};
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Unique identifier for a spawned task.
pub type TaskId = u64;

/// Task status for tracking async operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed with an error.
    Failed(String),
    /// Task was cancelled.
    Cancelled,
}

/// Progress update for long-running tasks.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Task identifier.
    pub task_id: TaskId,
    /// Current progress (0-100).
    pub progress: u8,
    /// Optional status message.
    pub message: Option<String>,
}

/// Task category for organizing different types of concurrent operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskCategory {
    /// Download operations (`YouTube` downloads).
    Download,
    /// File transfer operations (syncing to devices).
    FileTransfer,
    /// Device monitoring operations.
    DeviceMonitor,
    /// General background tasks.
    Background,
}

impl std::fmt::Display for TaskCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Download => write!(f, "download"),
            Self::FileTransfer => write!(f, "file_transfer"),
            Self::DeviceMonitor => write!(f, "device_monitor"),
            Self::Background => write!(f, "background"),
        }
    }
}

/// Information about a tracked task.
#[derive(Debug)]
pub struct TaskInfo {
    /// Task category.
    pub category: TaskCategory,
    /// Current status.
    pub status: TaskStatus,
    /// Optional description.
    pub description: Option<String>,
}

/// Configuration for the async runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Number of worker threads for the async runtime.
    /// Defaults to the number of CPU cores.
    pub worker_threads: Option<usize>,
    /// Maximum number of blocking threads.
    /// Defaults to 512.
    pub max_blocking_threads: usize,
    /// Thread keep-alive duration in seconds.
    pub thread_keep_alive_secs: u64,
    /// Name prefix for worker threads.
    pub thread_name_prefix: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            worker_threads: None, // Use Tokio's default (num CPUs)
            max_blocking_threads: 512,
            thread_keep_alive_secs: 10,
            thread_name_prefix: "youtun4-worker".to_string(),
        }
    }
}

/// Manages the Tokio async runtime and task lifecycle.
pub struct AsyncRuntime {
    /// The Tokio runtime instance.
    runtime: Runtime,
    /// Counter for generating unique task IDs.
    task_counter: AtomicU64,
    /// Tracked tasks with their handles and info.
    tasks: Arc<RwLock<HashMap<TaskId, TaskInfo>>>,
    /// Channel for sending progress updates.
    progress_tx: mpsc::UnboundedSender<ProgressUpdate>,
    /// Channel for receiving progress updates.
    progress_rx: Arc<RwLock<mpsc::UnboundedReceiver<ProgressUpdate>>>,
    /// Cancellation senders for tasks that support cancellation.
    cancel_senders: Arc<RwLock<HashMap<TaskId, oneshot::Sender<()>>>>,
}

impl AsyncRuntime {
    /// Create a new async runtime with default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime cannot be created.
    pub fn new() -> std::io::Result<Self> {
        Self::with_config(RuntimeConfig::default())
    }

    /// Create a new async runtime with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime cannot be created.
    pub fn with_config(config: RuntimeConfig) -> std::io::Result<Self> {
        info!("Initializing async runtime with config: {:?}", config);

        let mut builder = Builder::new_multi_thread();

        if let Some(threads) = config.worker_threads {
            builder.worker_threads(threads);
        }

        builder
            .max_blocking_threads(config.max_blocking_threads)
            .thread_keep_alive(std::time::Duration::from_secs(
                config.thread_keep_alive_secs,
            ))
            .thread_name(config.thread_name_prefix)
            .enable_all();

        let runtime = builder.build()?;
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();

        info!("Async runtime initialized successfully");

        Ok(Self {
            runtime,
            task_counter: AtomicU64::new(0),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            progress_tx,
            progress_rx: Arc::new(RwLock::new(progress_rx)),
            cancel_senders: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Generate a new unique task ID.
    fn next_task_id(&self) -> TaskId {
        self.task_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Get a new unique task ID without spawning a task.
    /// Use this when you need a task ID but will manage the task lifecycle yourself
    /// (e.g., when spawning in a separate OS thread to avoid nested runtime issues).
    pub fn generate_task_id(&self) -> TaskId {
        self.next_task_id()
    }

    /// Spawn a new async task.
    ///
    /// Returns a task ID that can be used to track or cancel the task.
    pub fn spawn<F, T>(
        &self,
        category: TaskCategory,
        description: Option<String>,
        future: F,
    ) -> TaskId
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let task_id = self.next_task_id();
        let tasks = Arc::clone(&self.tasks);

        debug!(
            "Spawning task {} ({}) - {:?}",
            task_id, category, description
        );

        // Register the task
        {
            let tasks_clone = Arc::clone(&tasks);
            self.runtime.block_on(async {
                let mut tasks_guard = tasks_clone.write().await;
                tasks_guard.insert(
                    task_id,
                    TaskInfo {
                        category,
                        status: TaskStatus::Running,
                        description: description.clone(),
                    },
                );
            });
        }

        // Spawn the actual task
        let _handle: JoinHandle<()> = self.runtime.spawn(async move {
            let result = future.await;
            // Drop the result as we only track completion status
            drop(result);

            // Update task status on completion
            let mut tasks_guard = tasks.write().await;
            if let Some(info) = tasks_guard.get_mut(&task_id) {
                info.status = TaskStatus::Completed;
            }
            debug!("Task {} completed", task_id);
        });

        task_id
    }

    /// Spawn a cancellable async task.
    ///
    /// Returns a task ID that can be used to cancel the task via `cancel_task`.
    pub fn spawn_cancellable<F, T>(
        &self,
        category: TaskCategory,
        description: Option<String>,
        future_factory: impl FnOnce(oneshot::Receiver<()>) -> F + Send + 'static,
    ) -> TaskId
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let task_id = self.next_task_id();
        let tasks = Arc::clone(&self.tasks);
        let cancel_senders = Arc::clone(&self.cancel_senders);

        debug!(
            "Spawning cancellable task {} ({}) - {:?}",
            task_id, category, description
        );

        // Create cancellation channel
        let (cancel_tx, cancel_rx) = oneshot::channel();

        // Register the task and cancellation sender
        {
            let tasks_clone = Arc::clone(&tasks);
            let cancel_senders_clone = Arc::clone(&cancel_senders);
            self.runtime.block_on(async {
                let mut tasks_guard = tasks_clone.write().await;
                tasks_guard.insert(
                    task_id,
                    TaskInfo {
                        category,
                        status: TaskStatus::Running,
                        description: description.clone(),
                    },
                );
                let mut cancel_guard = cancel_senders_clone.write().await;
                cancel_guard.insert(task_id, cancel_tx);
            });
        }

        // Spawn the actual task
        let _handle: JoinHandle<()> = self.runtime.spawn(async move {
            let future = future_factory(cancel_rx);
            let result = future.await;
            drop(result);

            // Update task status on completion
            let mut tasks_guard = tasks.write().await;
            if let Some(info) = tasks_guard.get_mut(&task_id)
                && info.status == TaskStatus::Running
            {
                info.status = TaskStatus::Completed;
            }
            debug!("Cancellable task {} completed", task_id);
        });

        task_id
    }

    /// Cancel a running task.
    ///
    /// Returns `true` if the cancellation signal was sent successfully.
    pub async fn cancel_task(&self, task_id: TaskId) -> bool {
        let mut cancel_guard = self.cancel_senders.write().await;
        if let Some(sender) = cancel_guard.remove(&task_id)
            && sender.send(()).is_ok()
        {
            // Update task status
            let mut tasks_guard = self.tasks.write().await;
            if let Some(info) = tasks_guard.get_mut(&task_id) {
                info.status = TaskStatus::Cancelled;
            }
            info!("Task {} cancelled", task_id);
            return true;
        }
        warn!(
            "Failed to cancel task {} - not found or already completed",
            task_id
        );
        false
    }

    /// Get the status of a task.
    pub async fn task_status(&self, task_id: TaskId) -> Option<TaskStatus> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard.get(&task_id).map(|info| info.status.clone())
    }

    /// Get information about a task.
    pub async fn task_info(&self, task_id: TaskId) -> Option<TaskInfo> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard.get(&task_id).map(|info| TaskInfo {
            category: info.category,
            status: info.status.clone(),
            description: info.description.clone(),
        })
    }

    /// List all tasks of a specific category.
    pub async fn list_tasks(&self, category: Option<TaskCategory>) -> Vec<(TaskId, TaskInfo)> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard
            .iter()
            .filter(|(_, info)| category.is_none_or(|c| c == info.category))
            .map(|(id, info)| {
                (
                    *id,
                    TaskInfo {
                        category: info.category,
                        status: info.status.clone(),
                        description: info.description.clone(),
                    },
                )
            })
            .collect()
    }

    /// Count running tasks by category.
    pub async fn running_tasks_count(&self) -> HashMap<TaskCategory, usize> {
        let tasks_guard = self.tasks.read().await;
        let mut counts = HashMap::new();
        for info in tasks_guard.values() {
            if info.status == TaskStatus::Running {
                *counts.entry(info.category).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Send a progress update for a task.
    pub fn send_progress(&self, task_id: TaskId, progress: u8, message: Option<String>) {
        let progress = progress.min(100); // Clamp to 100
        if let Err(e) = self.progress_tx.send(ProgressUpdate {
            task_id,
            progress,
            message,
        }) {
            error!("Failed to send progress update: {}", e);
        }
    }

    /// Create a progress sender that can be passed to async tasks.
    pub fn progress_sender(&self) -> ProgressSender {
        ProgressSender {
            tx: self.progress_tx.clone(),
        }
    }

    /// Try to receive a progress update without blocking.
    pub async fn try_recv_progress(&self) -> Option<ProgressUpdate> {
        let mut rx_guard = self.progress_rx.write().await;
        rx_guard.try_recv().ok()
    }

    /// Block on a future within this runtime.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    /// Spawn a blocking operation on the blocking thread pool.
    pub fn spawn_blocking<F, T>(&self, f: F) -> JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.runtime.spawn_blocking(f)
    }

    /// Clean up completed tasks older than the specified number of entries.
    ///
    /// Keeps the most recent `keep_count` completed/failed/cancelled tasks.
    pub async fn cleanup_completed_tasks(&self, keep_count: usize) {
        let mut tasks_guard = self.tasks.write().await;

        // Collect completed task IDs
        let mut completed: Vec<TaskId> = tasks_guard
            .iter()
            .filter(|(_, info)| info.status != TaskStatus::Running)
            .map(|(id, _)| *id)
            .collect();

        // Sort by ID (older first) and remove excess
        completed.sort_unstable();
        let to_remove = completed.len().saturating_sub(keep_count);
        for &task_id in completed.iter().take(to_remove) {
            tasks_guard.remove(&task_id);
        }

        if to_remove > 0 {
            debug!("Cleaned up {} completed tasks", to_remove);
        }
    }
}

impl std::fmt::Debug for AsyncRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncRuntime")
            .field("task_counter", &self.task_counter)
            .finish_non_exhaustive()
    }
}

/// A cloneable handle for sending progress updates.
#[derive(Clone)]
pub struct ProgressSender {
    tx: mpsc::UnboundedSender<ProgressUpdate>,
}

impl ProgressSender {
    /// Send a progress update.
    pub fn send(&self, task_id: TaskId, progress: u8, message: Option<String>) {
        let progress = progress.min(100);
        if let Err(e) = self.tx.send(ProgressUpdate {
            task_id,
            progress,
            message,
        }) {
            error!("Failed to send progress update: {}", e);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_runtime_creation() {
        let runtime = AsyncRuntime::new();
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_runtime_with_config() {
        let config = RuntimeConfig {
            worker_threads: Some(2),
            max_blocking_threads: 10,
            thread_keep_alive_secs: 5,
            thread_name_prefix: "test-worker".to_string(),
        };
        let runtime = AsyncRuntime::with_config(config);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_spawn_task() {
        let runtime = AsyncRuntime::new().expect("Failed to create runtime");
        let task_id = runtime.spawn(TaskCategory::Background, Some("test".to_string()), async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        });
        assert_eq!(task_id, 0);
    }

    #[test]
    fn test_task_status() {
        let runtime = AsyncRuntime::new().expect("Failed to create runtime");
        let task_id = runtime.spawn(TaskCategory::Download, None, async {
            tokio::time::sleep(Duration::from_millis(100)).await;
        });

        // Task should be running initially
        let status = runtime.block_on(runtime.task_status(task_id));
        assert_eq!(status, Some(TaskStatus::Running));

        // Wait for completion
        std::thread::sleep(Duration::from_millis(200));
        let status = runtime.block_on(runtime.task_status(task_id));
        assert_eq!(status, Some(TaskStatus::Completed));
    }

    #[test]
    fn test_progress_sender() {
        let runtime = AsyncRuntime::new().expect("Failed to create runtime");
        runtime.send_progress(0, 50, Some("halfway".to_string()));

        let progress = runtime.block_on(runtime.try_recv_progress());
        assert!(progress.is_some());
        let update = progress.expect("expected progress");
        assert_eq!(update.task_id, 0);
        assert_eq!(update.progress, 50);
        assert_eq!(update.message, Some("halfway".to_string()));
    }

    #[test]
    fn test_cancellable_task() {
        let runtime = AsyncRuntime::new().expect("Failed to create runtime");
        let task_id = runtime.spawn_cancellable(
            TaskCategory::Download,
            Some("cancellable test".to_string()),
            |cancel_rx| async move {
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(60)) => {
                        "completed"
                    }
                    _ = cancel_rx => {
                        "cancelled"
                    }
                }
            },
        );

        // Cancel the task
        let cancelled = runtime.block_on(runtime.cancel_task(task_id));
        assert!(cancelled);

        // Check status
        let status = runtime.block_on(runtime.task_status(task_id));
        assert_eq!(status, Some(TaskStatus::Cancelled));
    }
}
