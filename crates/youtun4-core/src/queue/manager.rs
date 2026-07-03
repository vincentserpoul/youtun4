use std::sync::Arc;

use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use super::state::QueueState;
use super::types::{
    DownloadPriority, DownloadRequest, MAX_CONCURRENT_DOWNLOADS, MIN_CONCURRENT_DOWNLOADS,
    QueueConfig, QueueEvent, QueueItem, QueueItemId, QueueItemStatus, QueueStats,
};
use crate::time::unix_timestamp_millis;

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
            // Don't allow removing items that are currently downloading
            if let Some(item) = state.items.get(pos)
                && matches!(item.status, QueueItemStatus::Downloading)
            {
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

            let now = unix_timestamp_millis();

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
            let now = unix_timestamp_millis();

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
            let now = unix_timestamp_millis();

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
            let now = unix_timestamp_millis();

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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;
    use crate::queue::DEFAULT_MAX_CONCURRENT_DOWNLOADS;

    // ========== DownloadQueueManager Basic Tests ==========

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
    async fn test_queue_get_nonexistent_item() {
        let queue = DownloadQueueManager::new();
        let item = queue.get_item(999).await;
        assert!(item.is_none());
    }

    #[tokio::test]
    async fn test_queue_add_batch() {
        let queue = DownloadQueueManager::new();

        let requests = vec![
            DownloadRequest::new("url1", "/tmp/1"),
            DownloadRequest::new("url2", "/tmp/2"),
            DownloadRequest::new("url3", "/tmp/3"),
        ];

        let ids = queue.add_batch(requests).await;
        assert_eq!(ids.len(), 3);
        assert_eq!(ids, vec![0, 1, 2]);

        let stats = queue.stats().await;
        assert_eq!(stats.total_items, 3);
    }

    #[tokio::test]
    async fn test_queue_get_all_items() {
        let queue = DownloadQueueManager::new();

        queue.add(DownloadRequest::new("url1", "/tmp/1")).await;
        queue.add(DownloadRequest::new("url2", "/tmp/2")).await;

        let items = queue.get_all_items().await;
        assert_eq!(items.len(), 2);
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

    // ========== Priority Tests ==========

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
    async fn test_queue_set_priority() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;

        // Change priority
        let result = queue.set_priority(id, DownloadPriority::High).await;
        assert!(result);

        let item = queue.get_item(id).await.unwrap();
        assert_eq!(item.request.priority, DownloadPriority::High);
    }

    #[tokio::test]
    async fn test_queue_set_priority_nonexistent() {
        let queue = DownloadQueueManager::new();
        let result = queue.set_priority(999, DownloadPriority::High).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_queue_set_priority_finished_item() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.cancel(id).await;

        // Cannot change priority of finished item
        let result = queue.set_priority(id, DownloadPriority::High).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_queue_move_to_front() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        let result = queue.move_to_front(id).await;
        assert!(result);

        let item = queue.get_item(id).await.unwrap();
        assert_eq!(item.request.priority, DownloadPriority::High);
    }

    #[tokio::test]
    async fn test_queue_get_pending_items_sorted() {
        let queue = DownloadQueueManager::new();

        queue
            .add(DownloadRequest::new("low", "/tmp").with_priority(DownloadPriority::Low))
            .await;
        queue
            .add(DownloadRequest::new("high", "/tmp").with_priority(DownloadPriority::High))
            .await;
        queue.add(DownloadRequest::new("normal", "/tmp")).await;

        let pending = queue.get_pending_items().await;
        assert_eq!(pending.len(), 3);
        assert_eq!(pending[0].request.priority, DownloadPriority::High);
        assert_eq!(pending[1].request.priority, DownloadPriority::Normal);
        assert_eq!(pending[2].request.priority, DownloadPriority::Low);
    }

    // ========== Concurrent Limit Tests ==========

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
    async fn test_queue_can_start_download() {
        let queue = DownloadQueueManager::new();

        // No items - cannot start
        assert!(!queue.can_start_download().await);

        queue.add(DownloadRequest::new("url", "/tmp")).await;

        // Has pending items - can start
        assert!(queue.can_start_download().await);
    }

    #[tokio::test]
    async fn test_queue_can_start_download_paused() {
        let queue = DownloadQueueManager::new();
        queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.pause().await;

        // Paused - cannot start
        assert!(!queue.can_start_download().await);
    }

    #[tokio::test]
    async fn test_queue_set_max_concurrent() {
        let queue = DownloadQueueManager::new();

        queue.set_max_concurrent(3).await;
        let config = queue.config().await;
        assert_eq!(config.max_concurrent_downloads, 3);
    }

    #[tokio::test]
    async fn test_queue_set_max_concurrent_clamped() {
        let queue = DownloadQueueManager::new();

        // Test clamping to min
        queue.set_max_concurrent(0).await;
        let config = queue.config().await;
        assert_eq!(config.max_concurrent_downloads, MIN_CONCURRENT_DOWNLOADS);

        // Test clamping to max
        queue.set_max_concurrent(100).await;
        let config = queue.config().await;
        assert_eq!(config.max_concurrent_downloads, MAX_CONCURRENT_DOWNLOADS);
    }

    #[tokio::test]
    async fn test_queue_get_downloading_items() {
        let queue = DownloadQueueManager::new();

        queue.add(DownloadRequest::new("url1", "/tmp/1")).await;
        queue.add(DownloadRequest::new("url2", "/tmp/2")).await;

        queue.start_next().await;

        let downloading = queue.get_downloading_items().await;
        assert_eq!(downloading.len(), 1);
    }

    // ========== Pause/Resume Tests ==========

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
    async fn test_queue_pause_idempotent() {
        let queue = DownloadQueueManager::new();

        queue.pause().await;
        queue.pause().await; // Should not error
        assert!(queue.is_paused().await);
    }

    #[tokio::test]
    async fn test_queue_resume_idempotent() {
        let queue = DownloadQueueManager::new();

        queue.resume().await; // Should not error even if not paused
        assert!(!queue.is_paused().await);
    }

    // ========== Cancel Tests ==========

    #[tokio::test]
    async fn test_queue_cancel() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url1", "/tmp/1")).await;

        assert!(queue.cancel(id).await);

        let item = queue.get_item(id).await.unwrap();
        assert!(matches!(item.status, QueueItemStatus::Cancelled));
        assert!(item.finished_at.is_some());
    }

    #[tokio::test]
    async fn test_queue_cancel_nonexistent() {
        let queue = DownloadQueueManager::new();
        let result = queue.cancel(999).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_queue_cancel_already_finished() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.start_next().await;
        queue.mark_completed(id).await;

        // Cannot cancel completed item
        let result = queue.cancel(id).await;
        assert!(!result);
    }

    // ========== Remove Tests ==========

    #[tokio::test]
    async fn test_queue_remove_pending() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        let result = queue.remove(id).await;
        assert!(result);

        let item = queue.get_item(id).await;
        assert!(item.is_none());
    }

    #[tokio::test]
    async fn test_queue_remove_nonexistent() {
        let queue = DownloadQueueManager::new();
        let result = queue.remove(999).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_queue_remove_downloading() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.start_next().await;

        // Cannot remove downloading item
        let result = queue.remove(id).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_queue_remove_finished() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.cancel(id).await;

        // Can remove finished (cancelled) item
        let result = queue.remove(id).await;
        assert!(result);
    }

    // ========== Retry Tests ==========

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
        assert!(item.task_id.is_none());
        assert!(item.started_at.is_none());
        assert!(item.finished_at.is_none());
        assert!((item.progress - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_queue_retry_nonexistent() {
        let queue = DownloadQueueManager::new();
        let result = queue.retry(999).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_queue_retry_max_exceeded() {
        let config = QueueConfig {
            max_retries: 2,
            ..Default::default()
        };
        let queue = DownloadQueueManager::with_config(config);

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;

        // Fail and retry twice
        for _ in 0..2 {
            queue.start_next().await;
            queue.mark_failed(id, "error".to_string()).await;
            queue.retry(id).await;
        }

        // Third failure
        queue.start_next().await;
        queue.mark_failed(id, "error".to_string()).await;

        // Should not be able to retry anymore
        let result = queue.retry(id).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_queue_retry_not_failed() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;

        // Cannot retry pending item
        let result = queue.retry(id).await;
        assert!(!result);
    }

    // ========== Progress and Status Updates ==========

    #[tokio::test]
    async fn test_queue_mark_started() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.start_next().await;
        queue.mark_started(id, 42).await;

        let item = queue.get_item(id).await.unwrap();
        assert_eq!(item.task_id, Some(42));
    }

    #[tokio::test]
    async fn test_queue_update_progress() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.start_next().await;

        queue
            .update_progress(id, 0.5, Some("video_1.mp3".to_string()), Some(10), Some(5))
            .await;

        let item = queue.get_item(id).await.unwrap();
        assert!((item.progress - 0.5).abs() < f64::EPSILON);
        assert_eq!(item.current_video, Some("video_1.mp3".to_string()));
        assert_eq!(item.total_videos, Some(10));
        assert_eq!(item.videos_completed, Some(5));
    }

    #[tokio::test]
    async fn test_queue_mark_completed() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.start_next().await;
        queue.mark_completed(id).await;

        let item = queue.get_item(id).await.unwrap();
        assert!(matches!(item.status, QueueItemStatus::Completed));
        assert!((item.progress - 1.0).abs() < f64::EPSILON);
        assert!(item.finished_at.is_some());
    }

    #[tokio::test]
    async fn test_queue_mark_failed() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        queue.start_next().await;
        queue.mark_failed(id, "network error".to_string()).await;

        let item = queue.get_item(id).await.unwrap();
        assert!(matches!(item.status, QueueItemStatus::Failed(ref msg) if msg == "network error"));
        assert!(item.finished_at.is_some());
    }

    // ========== Clear Tests ==========

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

    #[tokio::test]
    async fn test_queue_clear_finished_none() {
        let queue = DownloadQueueManager::new();

        queue.add(DownloadRequest::new("url", "/tmp")).await;

        let removed = queue.clear_finished().await;
        assert_eq!(removed, 0);
    }

    #[tokio::test]
    async fn test_queue_clear_all() {
        let queue = DownloadQueueManager::new();

        queue.add(DownloadRequest::new("url1", "/tmp/1")).await;
        queue.add(DownloadRequest::new("url2", "/tmp/2")).await;
        let id3 = queue.add(DownloadRequest::new("url3", "/tmp/3")).await;

        // Start one download
        queue.start_next().await;

        // Cancel another
        queue.cancel(id3).await;

        // Clear all - should keep the downloading one
        let removed = queue.clear_all().await;
        assert_eq!(removed, 2);

        let stats = queue.stats().await;
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.downloading_count, 1);
    }

    // ========== Config Tests ==========

    #[tokio::test]
    async fn test_queue_get_config() {
        let config = QueueConfig {
            max_concurrent_downloads: 3,
            auto_start: false,
            auto_retry: true,
            max_retries: 5,
        };
        let queue = DownloadQueueManager::with_config(config.clone());

        let retrieved = queue.config().await;
        assert_eq!(retrieved, config);
    }

    #[tokio::test]
    async fn test_queue_set_config() {
        let queue = DownloadQueueManager::new();

        let new_config = QueueConfig {
            max_concurrent_downloads: 3,
            auto_start: false,
            auto_retry: true,
            max_retries: 10,
        };
        queue.set_config(new_config.clone()).await;

        let retrieved = queue.config().await;
        assert_eq!(retrieved, new_config);
    }

    // ========== Event Tests ==========

    #[tokio::test]
    async fn test_queue_try_recv_event() {
        let queue = DownloadQueueManager::new();

        queue.add(DownloadRequest::new("url", "/tmp")).await;

        let event = queue.try_recv_event().await;
        assert!(event.is_some());
        assert!(matches!(event.unwrap(), QueueEvent::ItemAdded(_)));
    }

    #[tokio::test]
    async fn test_queue_try_recv_event_none() {
        let queue = DownloadQueueManager::new();

        let event = queue.try_recv_event().await;
        assert!(event.is_none());
    }

    #[tokio::test]
    async fn test_queue_event_sender() {
        let queue = DownloadQueueManager::new();

        let sender = queue.event_sender();
        let _ = sender.send(QueueEvent::QueuePaused);

        let event = queue.try_recv_event().await;
        assert!(event.is_some());
        assert!(matches!(event.unwrap(), QueueEvent::QueuePaused));
    }

    // ========== Default and Debug Tests ==========

    #[tokio::test]
    async fn test_queue_default() {
        let queue = DownloadQueueManager::default();
        let config = queue.config().await;
        assert_eq!(
            config.max_concurrent_downloads,
            DEFAULT_MAX_CONCURRENT_DOWNLOADS
        );
    }

    #[test]
    fn test_queue_debug() {
        let queue = DownloadQueueManager::new();
        let debug_str = format!("{queue:?}");
        assert!(debug_str.contains("DownloadQueueManager"));
    }

    // ========== QueueEvent Serde Tests ==========

    #[test]
    fn test_queue_event_serde() {
        let events = vec![
            QueueEvent::QueuePaused,
            QueueEvent::QueueResumed,
            QueueEvent::QueueCleared,
            QueueEvent::ItemCompleted { item_id: 1 },
            QueueEvent::ItemCancelled { item_id: 2 },
            QueueEvent::ItemRemoved { item_id: 3 },
            QueueEvent::ItemFailed {
                item_id: 4,
                error: "test".to_string(),
            },
            QueueEvent::ItemStarted {
                item_id: 5,
                task_id: 100,
            },
            QueueEvent::ItemPriorityChanged {
                item_id: 6,
                priority: DownloadPriority::High,
            },
            QueueEvent::ItemProgress {
                item_id: 7,
                progress: 0.5,
                current_video: Some("video".to_string()),
                total_videos: Some(10),
                videos_completed: Some(5),
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: QueueEvent = serde_json::from_str(&json).unwrap();
            // Just verify it round-trips without error
            let _ = serde_json::to_string(&deserialized).unwrap();
        }
    }

    // ========== Edge Cases ==========

    #[tokio::test]
    async fn test_queue_start_next_empty() {
        let queue = DownloadQueueManager::new();
        let result = queue.start_next().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_queue_fifo_within_same_priority() {
        let queue = DownloadQueueManager::new();

        // Add items with same priority
        let id1 = queue.add(DownloadRequest::new("first", "/tmp/1")).await;
        let id2 = queue.add(DownloadRequest::new("second", "/tmp/2")).await;
        let id3 = queue.add(DownloadRequest::new("third", "/tmp/3")).await;

        // Should be processed in FIFO order
        let next = queue.start_next().await.unwrap();
        assert_eq!(next.id, id1);

        let next = queue.start_next().await.unwrap();
        assert_eq!(next.id, id2);

        // Complete first two so we can start third
        queue.mark_completed(id1).await;
        queue.mark_completed(id2).await;

        let next = queue.start_next().await.unwrap();
        assert_eq!(next.id, id3);
    }

    #[tokio::test]
    async fn test_queue_item_timestamps() {
        let queue = DownloadQueueManager::new();

        let id = queue.add(DownloadRequest::new("url", "/tmp")).await;
        let item = queue.get_item(id).await.unwrap();

        // Should have added_at set
        assert!(item.added_at > 0);
        assert!(item.started_at.is_none());
        assert!(item.finished_at.is_none());

        // Start download
        queue.start_next().await;
        let item = queue.get_item(id).await.unwrap();
        assert!(item.started_at.is_some());
        assert!(item.started_at.unwrap() >= item.added_at);

        // Complete download
        queue.mark_completed(id).await;
        let item = queue.get_item(id).await.unwrap();
        assert!(item.finished_at.is_some());
        assert!(item.finished_at.unwrap() >= item.started_at.unwrap());
    }
}
