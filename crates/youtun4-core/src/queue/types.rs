use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
    pub(super) fn new(id: QueueItemId, request: DownloadRequest) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "duration in ms won't exceed u64"
                )]
                let ms = d.as_millis() as u64;
                ms
            });

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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;

    // ========== DownloadPriority Tests ==========

    #[test]
    fn test_download_priority_default() {
        let priority = DownloadPriority::default();
        assert_eq!(priority, DownloadPriority::Normal);
    }

    #[test]
    fn test_download_priority_ordering() {
        assert!(DownloadPriority::Low < DownloadPriority::Normal);
        assert!(DownloadPriority::Normal < DownloadPriority::High);
        assert!(DownloadPriority::Low < DownloadPriority::High);
    }

    #[test]
    fn test_download_priority_display() {
        assert_eq!(format!("{}", DownloadPriority::Low), "Low");
        assert_eq!(format!("{}", DownloadPriority::Normal), "Normal");
        assert_eq!(format!("{}", DownloadPriority::High), "High");
    }

    #[test]
    fn test_download_priority_serde() {
        let high = DownloadPriority::High;
        let json = serde_json::to_string(&high).unwrap();
        assert_eq!(json, "\"high\"");

        let deserialized: DownloadPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, DownloadPriority::High);
    }

    // ========== QueueItemStatus Tests ==========

    #[test]
    fn test_queue_item_status_display() {
        assert_eq!(format!("{}", QueueItemStatus::Pending), "Pending");
        assert_eq!(format!("{}", QueueItemStatus::Downloading), "Downloading");
        assert_eq!(format!("{}", QueueItemStatus::Completed), "Completed");
        assert_eq!(
            format!("{}", QueueItemStatus::Failed("network error".to_string())),
            "Failed: network error"
        );
        assert_eq!(format!("{}", QueueItemStatus::Cancelled), "Cancelled");
    }

    #[test]
    fn test_queue_item_status_serde() {
        let pending = QueueItemStatus::Pending;
        let json = serde_json::to_string(&pending).unwrap();
        assert_eq!(json, "\"pending\"");

        let failed = QueueItemStatus::Failed("test".to_string());
        let json = serde_json::to_string(&failed).unwrap();
        assert!(json.contains("failed"));
    }

    // ========== QueueConfig Tests ==========

    #[test]
    fn test_queue_config_default() {
        let config = QueueConfig::default();
        assert_eq!(
            config.max_concurrent_downloads,
            DEFAULT_MAX_CONCURRENT_DOWNLOADS
        );
        assert!(config.auto_start);
        assert!(!config.auto_retry);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_queue_config_validate_clamps_min() {
        let mut config = QueueConfig {
            max_concurrent_downloads: 0,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.max_concurrent_downloads, MIN_CONCURRENT_DOWNLOADS);
    }

    #[test]
    fn test_queue_config_validate_clamps_max() {
        let mut config = QueueConfig {
            max_concurrent_downloads: 100,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.max_concurrent_downloads, MAX_CONCURRENT_DOWNLOADS);
    }

    #[test]
    fn test_queue_config_validate_keeps_valid() {
        let mut config = QueueConfig {
            max_concurrent_downloads: 3,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.max_concurrent_downloads, 3);
    }

    #[test]
    fn test_queue_config_serde() {
        let config = QueueConfig {
            max_concurrent_downloads: 3,
            auto_start: false,
            auto_retry: true,
            max_retries: 5,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: QueueConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, config);
    }

    // ========== DownloadRequest Tests ==========

    #[test]
    fn test_download_request_new() {
        let request = DownloadRequest::new("https://youtube.com/playlist", "/tmp/output");
        assert_eq!(request.url, "https://youtube.com/playlist");
        assert_eq!(request.output_dir, PathBuf::from("/tmp/output"));
        assert!(request.playlist_name.is_none());
        assert!(request.audio_quality.is_none());
        assert!(request.embed_thumbnail.is_none());
        assert_eq!(request.priority, DownloadPriority::Normal);
    }

    #[test]
    fn test_download_request_builders() {
        let request = DownloadRequest::new("url", "/tmp")
            .with_playlist_name("My Playlist")
            .with_audio_quality("320")
            .with_embed_thumbnail(true)
            .with_priority(DownloadPriority::High);

        assert_eq!(request.playlist_name, Some("My Playlist".to_string()));
        assert_eq!(request.audio_quality, Some("320".to_string()));
        assert_eq!(request.embed_thumbnail, Some(true));
        assert_eq!(request.priority, DownloadPriority::High);
    }

    // ========== QueueItem Tests ==========

    #[test]
    fn test_queue_item_display_name_with_name() {
        let request = DownloadRequest::new("url", "/tmp").with_playlist_name("Test Playlist");
        let item = QueueItem::new(1, request);
        assert_eq!(item.display_name(), "Test Playlist");
    }

    #[test]
    fn test_queue_item_display_name_without_name() {
        let request = DownloadRequest::new("https://youtube.com/playlist", "/tmp");
        let item = QueueItem::new(1, request);
        assert_eq!(item.display_name(), "https://youtube.com/playlist");
    }

    #[test]
    fn test_queue_item_is_finished() {
        let request = DownloadRequest::new("url", "/tmp");

        let mut item = QueueItem::new(1, request);
        assert!(!item.is_finished());

        item.status = QueueItemStatus::Downloading;
        assert!(!item.is_finished());

        item.status = QueueItemStatus::Completed;
        assert!(item.is_finished());

        item.status = QueueItemStatus::Failed("error".to_string());
        assert!(item.is_finished());

        item.status = QueueItemStatus::Cancelled;
        assert!(item.is_finished());
    }

    #[test]
    fn test_queue_item_can_retry() {
        let request = DownloadRequest::new("url", "/tmp");
        let mut item = QueueItem::new(1, request);

        // Pending items cannot be retried
        assert!(!item.can_retry(3));

        // Failed items can be retried
        item.status = QueueItemStatus::Failed("error".to_string());
        assert!(item.can_retry(3));

        // But not if max retries exceeded
        item.retry_count = 3;
        assert!(!item.can_retry(3));

        // Or if it's not failed
        item.retry_count = 0;
        item.status = QueueItemStatus::Completed;
        assert!(!item.can_retry(3));
    }

    // ========== QueueStats Tests ==========

    #[test]
    fn test_queue_stats_default() {
        let stats = QueueStats::default();
        assert_eq!(stats.total_items, 0);
        assert_eq!(stats.pending_count, 0);
        assert_eq!(stats.downloading_count, 0);
        assert_eq!(stats.completed_count, 0);
        assert_eq!(stats.failed_count, 0);
        assert_eq!(stats.cancelled_count, 0);
    }
}
