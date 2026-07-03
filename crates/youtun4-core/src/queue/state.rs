use std::collections::VecDeque;

use super::types::{QueueConfig, QueueItem, QueueItemId, QueueItemStatus, QueueStats};

/// Internal state for the queue manager.
pub(super) struct QueueState {
    /// The queue of download items (ordered by priority and add time).
    pub(super) items: VecDeque<QueueItem>,
    /// Counter for generating unique item IDs.
    pub(super) next_id: QueueItemId,
    /// Current configuration.
    pub(super) config: QueueConfig,
    /// Whether the queue is paused.
    pub(super) paused: bool,
}

impl QueueState {
    pub(super) const fn new(config: QueueConfig) -> Self {
        Self {
            items: VecDeque::new(),
            next_id: 0,
            config,
            paused: false,
        }
    }

    /// Generate the next unique item ID.
    pub(super) const fn next_item_id(&mut self) -> QueueItemId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Get the number of currently downloading items.
    pub(super) fn active_download_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item.status, QueueItemStatus::Downloading))
            .count()
    }

    /// Get the next pending item that should be started, respecting priority.
    /// Higher priority items are processed first, and within the same priority,
    /// older items (smaller `added_at`) are processed first (FIFO).
    pub(super) fn next_pending_item(&self) -> Option<QueueItemId> {
        self.items
            .iter()
            .filter(|item| matches!(item.status, QueueItemStatus::Pending))
            // Sort by: highest priority first, then oldest first (smallest added_at)
            // Use min_by_key with negated priority to get highest priority first
            .min_by_key(|item| (std::cmp::Reverse(item.request.priority), item.added_at))
            .map(|item| item.id)
    }

    /// Find an item by ID.
    pub(super) fn find_item(&self, id: QueueItemId) -> Option<&QueueItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// Find an item by ID (mutable).
    pub(super) fn find_item_mut(&mut self, id: QueueItemId) -> Option<&mut QueueItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    /// Calculate queue statistics.
    pub(super) fn stats(&self) -> QueueStats {
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
