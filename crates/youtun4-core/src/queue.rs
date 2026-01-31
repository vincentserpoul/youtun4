//! Download queue manager for handling multiple playlist download requests.
//!
//! This module provides a queue system for managing concurrent downloads with:
//! - Configurable concurrent download limits
//! - Priority-based ordering
//! - Queue item lifecycle management (pending, downloading, completed, failed, cancelled)
//! - Event emission for queue state changes

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

/// Unique identifier for a queue item.
pub type QueueItemId = u64;

/// Default maximum number of concurrent downloads.
pub const DEFAULT_MAX_CONCURRENT_DOWNLOADS: usize = 2;

/// Minimum allowed concurrent downloads.
pub const MIN_CONCURRENT_DOWNLOADS: usize = 1;

/// Maximum allowed concurrent downloads.
pub const MAX_CONCURRENT_DOWNLOADS: usize = 4;

/// Priority level for download queue items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DownloadPriority {
    /// Low priority - processed after normal and high priority items.
    Low = 0,
    /// Normal priority (default).
    #[default]
    Normal = 1,
    /// High priority - processed before normal and low priority items.
    High = 2,
}

impl std::fmt::Display for DownloadPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Normal => write!(f, "Normal"),
            Self::High => write!(f, "High"),
        }
    }
}

/// Status of a queue item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueItemStatus {
    /// Item is waiting to be processed.
    Pending,
    /// Item is currently being downloaded.
    Downloading,
    /// Download completed successfully.
    Completed,
    /// Download failed with an error.
    Failed(String),
    /// Download was cancelled.
    Cancelled,
}

impl std::fmt::Display for QueueItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Downloading => write!(f, "Downloading"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed(msg) => write!(f, "Failed: {msg}"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Configuration for the download queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueConfig {
    /// Maximum number of concurrent downloads.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_downloads: usize,
    /// Whether to auto-start downloads when items are added.
    #[serde(default = "default_true")]
    pub auto_start: bool,
    /// Whether to retry failed downloads automatically.
    #[serde(default)]
    pub auto_retry: bool,
    /// Maximum number of retries for failed downloads.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

const fn default_max_concurrent() -> usize {
    DEFAULT_MAX_CONCURRENT_DOWNLOADS
}

const fn default_true() -> bool {
    true
}

const fn default_max_retries() -> u32 {
    3
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: DEFAULT_MAX_CONCURRENT_DOWNLOADS,
            auto_start: true,
            auto_retry: false,
            max_retries: 3,
        }
    }
}

impl QueueConfig {
    /// Validate and clamp the `max_concurrent_downloads` value.
    pub fn validate(&mut self) {
        self.max_concurrent_downloads = self
            .max_concurrent_downloads
            .clamp(MIN_CONCURRENT_DOWNLOADS, MAX_CONCURRENT_DOWNLOADS);
    }
}

/// A download request to be queued.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    /// `YouTube` playlist URL.
    pub url: String,
    /// Output directory for downloaded files.
    pub output_dir: PathBuf,
    /// Optional playlist name for display purposes.
    pub playlist_name: Option<String>,
    /// Audio quality setting (e.g., "192", "320").
    pub audio_quality: Option<String>,
    /// Whether to embed thumbnails in MP3 files.
    pub embed_thumbnail: Option<bool>,
    /// Priority level for this download.
    #[serde(default)]
    pub priority: DownloadPriority,
}

impl DownloadRequest {
    /// Create a new download request.
    pub fn new(url: impl Into<String>, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            output_dir: output_dir.into(),
            playlist_name: None,
            audio_quality: None,
            embed_thumbnail: None,
            priority: DownloadPriority::default(),
        }
    }

    /// Set the playlist name.
    #[must_use]
    pub fn with_playlist_name(mut self, name: impl Into<String>) -> Self {
        self.playlist_name = Some(name.into());
        self
    }

    /// Set the audio quality.
    #[must_use]
    pub fn with_audio_quality(mut self, quality: impl Into<String>) -> Self {
        self.audio_quality = Some(quality.into());
        self
    }

    /// Set whether to embed thumbnails.
    #[must_use]
    pub const fn with_embed_thumbnail(mut self, embed: bool) -> Self {
        self.embed_thumbnail = Some(embed);
        self
    }

    /// Set the priority level.
    #[must_use]
    pub const fn with_priority(mut self, priority: DownloadPriority) -> Self {
        self.priority = priority;
        self
    }
}

/// A queued download item with tracking information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Unique identifier for this queue item.
    pub id: QueueItemId,
    /// The download request details.
    pub request: DownloadRequest,
    /// Current status of the item.
    pub status: QueueItemStatus,
    /// Associated task ID (when downloading).
    pub task_id: Option<u64>,
    /// Number of retry attempts.
    pub retry_count: u32,
    /// Timestamp when the item was added (Unix millis).
    pub added_at: u64,
    /// Timestamp when download started (Unix millis).
    pub started_at: Option<u64>,
    /// Timestamp when download completed/failed (Unix millis).
    pub finished_at: Option<u64>,
    /// Download progress (0.0 - 1.0).
    pub progress: f64,
    /// Current video being downloaded (for display).
    pub current_video: Option<String>,
    /// Total videos in playlist.
    pub total_videos: Option<usize>,
    /// Videos completed so far.
    pub videos_completed: Option<usize>,
}

impl QueueItem {
    /// Create a new queue item from a download request.
    fn new(id: QueueItemId, request: DownloadRequest) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            id,
            request,
            status: QueueItemStatus::Pending,
            task_id: None,
            retry_count: 0,
            added_at: now,
            started_at: None,
            finished_at: None,
            progress: 0.0,
            current_video: None,
            total_videos: None,
            videos_completed: None,
        }
    }

    /// Get the display name for this item.
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.request
            .playlist_name
            .as_deref()
            .unwrap_or(&self.request.url)
    }

    /// Check if the item is in a terminal state (completed, failed, or cancelled).
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        matches!(
            self.status,
            QueueItemStatus::Completed | QueueItemStatus::Failed(_) | QueueItemStatus::Cancelled
        )
    }

    /// Check if the item can be retried.
    #[must_use]
    pub const fn can_retry(&self, max_retries: u32) -> bool {
        matches!(self.status, QueueItemStatus::Failed(_)) && self.retry_count < max_retries
    }
}

/// Event types emitted by the queue manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum QueueEvent {
    /// An item was added to the queue.
    ItemAdded(QueueItem),
    /// An item started downloading.
    ItemStarted {
        /// The queue item ID.
        item_id: QueueItemId,
        /// The associated task ID.
        task_id: u64,
    },
    /// An item's progress was updated.
    ItemProgress {
        /// The queue item ID.
        item_id: QueueItemId,
        /// Overall progress (0.0 - 1.0).
        progress: f64,
        /// Current video being downloaded.
        current_video: Option<String>,
        /// Total videos in playlist.
        total_videos: Option<usize>,
        /// Videos completed so far.
        videos_completed: Option<usize>,
    },
    /// An item completed successfully.
    ItemCompleted {
        /// The queue item ID.
        item_id: QueueItemId,
    },
    /// An item failed.
    ItemFailed {
        /// The queue item ID.
        item_id: QueueItemId,
        /// Error message.
        error: String,
    },
    /// An item was cancelled.
    ItemCancelled {
        /// The queue item ID.
        item_id: QueueItemId,
    },
    /// An item was removed from the queue.
    ItemRemoved {
        /// The queue item ID.
        item_id: QueueItemId,
    },
    /// An item's priority was changed.
    ItemPriorityChanged {
        /// The queue item ID.
        item_id: QueueItemId,
        /// The new priority.
        priority: DownloadPriority,
    },
    /// The queue was cleared.
    QueueCleared,
    /// Queue processing was paused.
    QueuePaused,
    /// Queue processing was resumed.
    QueueResumed,
    /// Queue configuration was updated.
    ConfigUpdated(QueueConfig),
}

/// Statistics about the queue.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueueStats {
    /// Total number of items in queue.
    pub total_items: usize,
    /// Number of pending items.
    pub pending_count: usize,
    /// Number of currently downloading items.
    pub downloading_count: usize,
    /// Number of completed items.
    pub completed_count: usize,
    /// Number of failed items.
    pub failed_count: usize,
    /// Number of cancelled items.
    pub cancelled_count: usize,
}

/// Internal state for the queue manager.
struct QueueState {
    /// The queue of download items (ordered by priority and add time).
    items: VecDeque<QueueItem>,
    /// Counter for generating unique item IDs.
    next_id: QueueItemId,
    /// Current configuration.
    config: QueueConfig,
    /// Whether the queue is paused.
    paused: bool,
}

impl QueueState {
    const fn new(config: QueueConfig) -> Self {
        Self {
            items: VecDeque::new(),
            next_id: 0,
            config,
            paused: false,
        }
    }

    /// Generate the next unique item ID.
    const fn next_item_id(&mut self) -> QueueItemId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Get the number of currently downloading items.
    fn active_download_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item.status, QueueItemStatus::Downloading))
            .count()
    }

    /// Get the next pending item that should be started, respecting priority.
    /// Higher priority items are processed first, and within the same priority,
    /// older items (smaller `added_at`) are processed first (FIFO).
    fn next_pending_item(&self) -> Option<QueueItemId> {
        self.items
            .iter()
            .filter(|item| matches!(item.status, QueueItemStatus::Pending))
            // Sort by: highest priority first, then oldest first (smallest added_at)
            // Use min_by_key with negated priority to get highest priority first
            .min_by_key(|item| (std::cmp::Reverse(item.request.priority), item.added_at))
            .map(|item| item.id)
    }

    /// Find an item by ID.
    fn find_item(&self, id: QueueItemId) -> Option<&QueueItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// Find an item by ID (mutable).
    fn find_item_mut(&mut self, id: QueueItemId) -> Option<&mut QueueItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    /// Calculate queue statistics.
    fn stats(&self) -> QueueStats {
        let mut pending_count = 0;
        let mut downloading_count = 0;
        let mut completed_count = 0;
        let mut failed_count = 0;
        let mut cancelled_count = 0;

        for item in &self.items {
            match &item.status {
                QueueItemStatus::Pending => pending_count += 1,
                QueueItemStatus::Downloading => downloading_count += 1,
                QueueItemStatus::Completed => completed_count += 1,
                QueueItemStatus::Failed(_) => failed_count += 1,
                QueueItemStatus::Cancelled => cancelled_count += 1,
            }
        }

        QueueStats {
            total_items: self.items.len(),
            pending_count,
            downloading_count,
            completed_count,
            failed_count,
            cancelled_count,
        }
    }
}

/// Manages a queue of download requests with concurrent processing support.
pub struct DownloadQueueManager {
    /// Internal state protected by async `RwLock`.
    state: Arc<RwLock<QueueState>>,
    /// Channel for sending queue events.
    event_tx: mpsc::UnboundedSender<QueueEvent>,
    /// Channel for receiving queue events.
    event_rx: Arc<RwLock<mpsc::UnboundedReceiver<QueueEvent>>>,
}

impl DownloadQueueManager {
    /// Create a new download queue manager with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(QueueConfig::default())
    }

    /// Create a new download queue manager with custom configuration.
    #[must_use]
    pub fn with_config(mut config: QueueConfig) -> Self {
        config.validate();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            state: Arc::new(RwLock::new(QueueState::new(config))),
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
        }
    }

    /// Add a download request to the queue.
    ///
    /// Returns the queue item ID for the added request.
    pub async fn add(&self, request: DownloadRequest) -> QueueItemId {
        let mut state = self.state.write().await;
        let id = state.next_item_id();
        let item = QueueItem::new(id, request);

        info!(
            "Adding download to queue: id={}, url={}",
            id, item.request.url
        );

        // Send event before modifying state
        let _ = self.event_tx.send(QueueEvent::ItemAdded(item.clone()));

        state.items.push_back(item);
        id
    }

    /// Add multiple download requests to the queue.
    ///
    /// Returns a vector of queue item IDs.
    pub async fn add_batch(&self, requests: Vec<DownloadRequest>) -> Vec<QueueItemId> {
        let mut state = self.state.write().await;
        let mut ids = Vec::with_capacity(requests.len());

        for request in requests {
            let id = state.next_item_id();
            let item = QueueItem::new(id, request);

            info!(
                "Adding download to queue (batch): id={}, url={}",
                id, item.request.url
            );
            let _ = self.event_tx.send(QueueEvent::ItemAdded(item.clone()));

            state.items.push_back(item);
            ids.push(id);
        }

        ids
    }

    /// Remove an item from the queue.
    ///
    /// Only pending or finished items can be removed.
    /// Returns true if the item was removed.
    pub async fn remove(&self, id: QueueItemId) -> bool {
        let mut state = self.state.write().await;

        if let Some(pos) = state.items.iter().position(|item| item.id == id) {
            let item = &state.items[pos];

            // Don't allow removing items that are currently downloading
            if matches!(item.status, QueueItemStatus::Downloading) {
                warn!("Cannot remove item {} - currently downloading", id);
                return false;
            }

            state.items.remove(pos);
            let _ = self.event_tx.send(QueueEvent::ItemRemoved { item_id: id });
            info!("Removed item {} from queue", id);
            true
        } else {
            warn!("Cannot remove item {} - not found", id);
            false
        }
    }

    /// Cancel a downloading or pending item.
    ///
    /// Returns true if the item was cancelled.
    pub async fn cancel(&self, id: QueueItemId) -> bool {
        let mut state = self.state.write().await;

        if let Some(item) = state.find_item_mut(id) {
            if item.is_finished() {
                warn!("Cannot cancel item {} - already finished", id);
                return false;
            }

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            item.status = QueueItemStatus::Cancelled;
            item.finished_at = Some(now);

            let _ = self
                .event_tx
                .send(QueueEvent::ItemCancelled { item_id: id });
            info!("Cancelled item {}", id);
            true
        } else {
            warn!("Cannot cancel item {} - not found", id);
            false
        }
    }

    /// Update the priority of a queue item.
    ///
    /// Returns true if the priority was updated.
    pub async fn set_priority(&self, id: QueueItemId, priority: DownloadPriority) -> bool {
        let mut state = self.state.write().await;

        if let Some(item) = state.find_item_mut(id) {
            if item.is_finished() {
                warn!("Cannot change priority of item {} - already finished", id);
                return false;
            }

            item.request.priority = priority;
            let _ = self.event_tx.send(QueueEvent::ItemPriorityChanged {
                item_id: id,
                priority,
            });
            info!("Updated priority of item {} to {:?}", id, priority);
            true
        } else {
            warn!("Cannot update priority of item {} - not found", id);
            false
        }
    }

    /// Move an item to the front of the queue (highest priority for pending items).
    pub async fn move_to_front(&self, id: QueueItemId) -> bool {
        self.set_priority(id, DownloadPriority::High).await
    }

    /// Get a specific queue item by ID.
    pub async fn get_item(&self, id: QueueItemId) -> Option<QueueItem> {
        let state = self.state.read().await;
        state.find_item(id).cloned()
    }

    /// Get all items in the queue.
    pub async fn get_all_items(&self) -> Vec<QueueItem> {
        let state = self.state.read().await;
        state.items.iter().cloned().collect()
    }

    /// Get all pending items in the queue, sorted by priority.
    pub async fn get_pending_items(&self) -> Vec<QueueItem> {
        let state = self.state.read().await;
        let mut items: Vec<_> = state
            .items
            .iter()
            .filter(|item| matches!(item.status, QueueItemStatus::Pending))
            .cloned()
            .collect();
        items.sort_by_key(|item| (std::cmp::Reverse(item.request.priority), item.added_at));
        items
    }

    /// Get all currently downloading items.
    pub async fn get_downloading_items(&self) -> Vec<QueueItem> {
        let state = self.state.read().await;
        state
            .items
            .iter()
            .filter(|item| matches!(item.status, QueueItemStatus::Downloading))
            .cloned()
            .collect()
    }

    /// Get queue statistics.
    pub async fn stats(&self) -> QueueStats {
        let state = self.state.read().await;
        state.stats()
    }

    /// Check if the queue can start a new download.
    pub async fn can_start_download(&self) -> bool {
        let state = self.state.read().await;
        !state.paused
            && state.active_download_count() < state.config.max_concurrent_downloads
            && state.next_pending_item().is_some()
    }

    /// Get the next item to download (if any).
    ///
    /// This marks the item as downloading and returns it.
    /// Returns None if no items are ready or the queue is at capacity.
    pub async fn start_next(&self) -> Option<QueueItem> {
        let mut state = self.state.write().await;

        if state.paused {
            debug!("Queue is paused, not starting next download");
            return None;
        }

        if state.active_download_count() >= state.config.max_concurrent_downloads {
            debug!(
                "At max concurrent downloads ({}/{})",
                state.active_download_count(),
                state.config.max_concurrent_downloads
            );
            return None;
        }

        let next_id = state.next_pending_item()?;

        // Find and update the item
        if let Some(item) = state.find_item_mut(next_id) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            item.status = QueueItemStatus::Downloading;
            item.started_at = Some(now);

            info!(
                "Starting download for item {}: {}",
                item.id, item.request.url
            );
            Some(item.clone())
        } else {
            None
        }
    }

    /// Mark an item as started with a task ID.
    pub async fn mark_started(&self, id: QueueItemId, task_id: u64) {
        let mut state = self.state.write().await;

        if let Some(item) = state.find_item_mut(id) {
            item.task_id = Some(task_id);
            let _ = self.event_tx.send(QueueEvent::ItemStarted {
                item_id: id,
                task_id,
            });
        }
    }

    /// Update the progress of a downloading item.
    pub async fn update_progress(
        &self,
        id: QueueItemId,
        progress: f64,
        current_video: Option<String>,
        total_videos: Option<usize>,
        videos_completed: Option<usize>,
    ) {
        let mut state = self.state.write().await;

        if let Some(item) = state.find_item_mut(id) {
            item.progress = progress;
            if let Some(ref video) = current_video {
                item.current_video = Some(video.clone());
            }
            if let Some(total) = total_videos {
                item.total_videos = Some(total);
            }
            if let Some(completed) = videos_completed {
                item.videos_completed = Some(completed);
            }

            let _ = self.event_tx.send(QueueEvent::ItemProgress {
                item_id: id,
                progress,
                current_video,
                total_videos,
                videos_completed,
            });
        }
    }

    /// Mark an item as completed.
    pub async fn mark_completed(&self, id: QueueItemId) {
        let mut state = self.state.write().await;

        if let Some(item) = state.find_item_mut(id) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            item.status = QueueItemStatus::Completed;
            item.finished_at = Some(now);
            item.progress = 1.0;

            let _ = self
                .event_tx
                .send(QueueEvent::ItemCompleted { item_id: id });
            info!("Item {} completed", id);
        }
    }

    /// Mark an item as failed.
    pub async fn mark_failed(&self, id: QueueItemId, error: String) {
        let mut state = self.state.write().await;

        if let Some(item) = state.find_item_mut(id) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            item.status = QueueItemStatus::Failed(error.clone());
            item.finished_at = Some(now);

            let _ = self.event_tx.send(QueueEvent::ItemFailed {
                item_id: id,
                error: error.clone(),
            });
            error!("Item {} failed: {}", id, error);
        }
    }

    /// Retry a failed item.
    ///
    /// Returns true if the item was reset for retry.
    pub async fn retry(&self, id: QueueItemId) -> bool {
        let mut state = self.state.write().await;
        let max_retries = state.config.max_retries;

        if let Some(item) = state.find_item_mut(id) {
            if !item.can_retry(max_retries) {
                warn!(
                    "Cannot retry item {} - not failed or max retries exceeded",
                    id
                );
                return false;
            }

            item.status = QueueItemStatus::Pending;
            item.retry_count += 1;
            item.task_id = None;
            item.started_at = None;
            item.finished_at = None;
            item.progress = 0.0;
            item.current_video = None;
            item.videos_completed = None;

            info!("Retrying item {} (attempt {})", id, item.retry_count);
            true
        } else {
            warn!("Cannot retry item {} - not found", id);
            false
        }
    }

    /// Pause queue processing.
    pub async fn pause(&self) {
        let mut state = self.state.write().await;
        if !state.paused {
            state.paused = true;
            let _ = self.event_tx.send(QueueEvent::QueuePaused);
            info!("Queue paused");
        }
    }

    /// Resume queue processing.
    pub async fn resume(&self) {
        let mut state = self.state.write().await;
        if state.paused {
            state.paused = false;
            let _ = self.event_tx.send(QueueEvent::QueueResumed);
            info!("Queue resumed");
        }
    }

    /// Check if the queue is paused.
    pub async fn is_paused(&self) -> bool {
        let state = self.state.read().await;
        state.paused
    }

    /// Clear all finished (completed, failed, cancelled) items from the queue.
    pub async fn clear_finished(&self) -> usize {
        let mut state = self.state.write().await;
        let before = state.items.len();
        state.items.retain(|item| !item.is_finished());
        let removed = before - state.items.len();
        if removed > 0 {
            info!("Cleared {} finished items from queue", removed);
        }
        removed
    }

    /// Clear all items from the queue (except currently downloading).
    pub async fn clear_all(&self) -> usize {
        let mut state = self.state.write().await;
        let before = state.items.len();
        state
            .items
            .retain(|item| matches!(item.status, QueueItemStatus::Downloading));
        let removed = before - state.items.len();
        let _ = self.event_tx.send(QueueEvent::QueueCleared);
        if removed > 0 {
            info!("Cleared {} items from queue", removed);
        }
        removed
    }

    /// Get the current configuration.
    pub async fn config(&self) -> QueueConfig {
        let state = self.state.read().await;
        state.config.clone()
    }

    /// Update the queue configuration.
    pub async fn set_config(&self, mut config: QueueConfig) {
        config.validate();
        let mut state = self.state.write().await;
        state.config = config.clone();
        let _ = self.event_tx.send(QueueEvent::ConfigUpdated(config));
        info!("Queue configuration updated");
    }

    /// Update just the max concurrent downloads setting.
    pub async fn set_max_concurrent(&self, max: usize) {
        let mut state = self.state.write().await;
        state.config.max_concurrent_downloads =
            max.clamp(MIN_CONCURRENT_DOWNLOADS, MAX_CONCURRENT_DOWNLOADS);
        let config = state.config.clone();
        let _ = self.event_tx.send(QueueEvent::ConfigUpdated(config));
        info!(
            "Max concurrent downloads set to {}",
            state.config.max_concurrent_downloads
        );
    }

    /// Try to receive a queue event without blocking.
    pub async fn try_recv_event(&self) -> Option<QueueEvent> {
        let mut rx = self.event_rx.write().await;
        rx.try_recv().ok()
    }

    /// Get a clone of the event sender for external use.
    #[must_use]
    pub fn event_sender(&self) -> mpsc::UnboundedSender<QueueEvent> {
        self.event_tx.clone()
    }
}

impl Default for DownloadQueueManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DownloadQueueManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadQueueManager")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue_add_and_get() {
        let queue = DownloadQueueManager::new();

        let request = DownloadRequest::new(
            "https://www.youtube.com/playlist?list=PLtest",
            "/tmp/downloads",
        );

        let id = queue.add(request.clone()).await;
        assert_eq!(id, 0);

        let item = queue.get_item(id).await;
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.id, 0);
        assert_eq!(
            item.request.url,
            "https://www.youtube.com/playlist?list=PLtest"
        );
        assert!(matches!(item.status, QueueItemStatus::Pending));
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = DownloadQueueManager::new();

        queue.add(DownloadRequest::new("url1", "/tmp/1")).await;
        queue.add(DownloadRequest::new("url2", "/tmp/2")).await;
        queue.add(DownloadRequest::new("url3", "/tmp/3")).await;

        let stats = queue.stats().await;
        assert_eq!(stats.total_items, 3);
        assert_eq!(stats.pending_count, 3);
        assert_eq!(stats.downloading_count, 0);
    }

    #[tokio::test]
    async fn test_queue_priority() {
        let queue = DownloadQueueManager::new();

        // Add items with different priorities
        let _low_id = queue
            .add(DownloadRequest::new("low", "/tmp/low").with_priority(DownloadPriority::Low))
            .await;
        let normal_id = queue
            .add(DownloadRequest::new("normal", "/tmp/normal"))
            .await;
        let high_id = queue
            .add(DownloadRequest::new("high", "/tmp/high").with_priority(DownloadPriority::High))
            .await;

        // High priority should be picked first
        let next = queue.start_next().await;
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, high_id);

        // Then normal
        let next = queue.start_next().await;
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, normal_id);
    }

    #[tokio::test]
    async fn test_queue_concurrent_limit() {
        let config = QueueConfig {
            max_concurrent_downloads: 2,
            ..Default::default()
        };
        let queue = DownloadQueueManager::with_config(config);

        queue.add(DownloadRequest::new("url1", "/tmp/1")).await;
        queue.add(DownloadRequest::new("url2", "/tmp/2")).await;
        queue.add(DownloadRequest::new("url3", "/tmp/3")).await;

        // Start two downloads (the limit)
        assert!(queue.start_next().await.is_some());
        assert!(queue.start_next().await.is_some());

        // Third should fail due to limit
        assert!(queue.start_next().await.is_none());
    }

    #[tokio::test]
    async fn test_queue_pause_resume() {
        let queue = DownloadQueueManager::new();

        queue.add(DownloadRequest::new("url1", "/tmp/1")).await;

        // Pause the queue
        queue.pause().await;
        assert!(queue.is_paused().await);

        // Should not start when paused
        assert!(queue.start_next().await.is_none());

        // Resume and it should work
        queue.resume().await;
        assert!(!queue.is_paused().await);
        assert!(queue.start_next().await.is_some());
    }

    #[tokio::test]
    async fn test_queue_cancel() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url1", "/tmp/1")).await;

        assert!(queue.cancel(id).await);

        let item = queue.get_item(id).await.unwrap();
        assert!(matches!(item.status, QueueItemStatus::Cancelled));
    }

    #[tokio::test]
    async fn test_queue_retry() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url1", "/tmp/1")).await;

        // Start and then fail the item
        queue.start_next().await;
        queue.mark_failed(id, "Test error".to_string()).await;

        // Retry should work
        assert!(queue.retry(id).await);

        let item = queue.get_item(id).await.unwrap();
        assert!(matches!(item.status, QueueItemStatus::Pending));
        assert_eq!(item.retry_count, 1);
    }

    #[tokio::test]
    async fn test_queue_clear_finished() {
        // Use only 1 concurrent download to control timing
        let config = QueueConfig {
            max_concurrent_downloads: 1,
            ..Default::default()
        };
        let queue = DownloadQueueManager::with_config(config);

        let id1 = queue.add(DownloadRequest::new("url1", "/tmp/1")).await;
        let id2 = queue.add(DownloadRequest::new("url2", "/tmp/2")).await;
        queue.add(DownloadRequest::new("url3", "/tmp/3")).await;

        // Start and complete the first item
        let started1 = queue.start_next().await;
        assert!(started1.is_some());
        assert_eq!(started1.unwrap().id, id1);
        queue.mark_completed(id1).await;

        // Start and fail the second item
        let started2 = queue.start_next().await;
        assert!(started2.is_some());
        assert_eq!(started2.unwrap().id, id2);
        queue.mark_failed(id2, "error".to_string()).await;

        let stats = queue.stats().await;
        assert_eq!(stats.total_items, 3);
        assert_eq!(stats.completed_count, 1);
        assert_eq!(stats.failed_count, 1);
        assert_eq!(stats.pending_count, 1);

        // Clear finished
        let removed = queue.clear_finished().await;
        assert_eq!(removed, 2);

        let stats = queue.stats().await;
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.pending_count, 1);
    }
}
