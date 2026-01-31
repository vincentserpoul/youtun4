//! Shared types for the `Youtun4` UI.
//!
//! These types mirror the core types but are WASM-compatible.

use serde::{Deserialize, Serialize};

/// Information about a detected device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Device name/identifier.
    pub name: String,
    /// Mount point path as string.
    pub mount_point: String,
    /// Total capacity in bytes.
    pub total_bytes: u64,
    /// Available space in bytes.
    pub available_bytes: u64,
    /// File system type (e.g., FAT32, exFAT).
    pub file_system: String,
    /// Whether the device is removable.
    pub is_removable: bool,
}

impl DeviceInfo {
    /// Returns the used space in bytes.
    #[must_use]
    pub const fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }

    /// Returns the usage percentage (0.0 - 100.0).
    #[must_use]
    pub fn usage_percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes() as f64 / self.total_bytes as f64) * 100.0
    }
}

/// Metadata for a playlist (computed view).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistMetadata {
    /// Playlist name (also the folder name).
    pub name: String,
    /// Original `YouTube` playlist URL (if created from `YouTube`).
    pub source_url: Option<String>,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last modified timestamp (Unix epoch seconds).
    pub modified_at: u64,
    /// Number of tracks in the playlist.
    pub track_count: usize,
    /// Total size in bytes.
    pub total_bytes: u64,
    /// Thumbnail URL for the playlist (from `YouTube`).
    pub thumbnail_url: Option<String>,
}

/// Saved playlist metadata stored in playlist.json.
///
/// This mirrors the core crate's `SavedPlaylistMetadata` struct.
/// It represents the persistent metadata stored in each playlist folder.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedPlaylistMetadata {
    /// Optional title for the playlist (defaults to folder name if not set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional description for the playlist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Original `YouTube` playlist URL (if created from `YouTube`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last modified timestamp (Unix epoch seconds).
    #[serde(default)]
    pub modified_at: u64,
    /// Number of tracks in the playlist (cached value).
    #[serde(default)]
    pub track_count: usize,
    /// Total size of all audio files in bytes (cached value).
    #[serde(default)]
    pub total_size_bytes: u64,
    /// Thumbnail URL from `YouTube` (if available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
}

impl SavedPlaylistMetadata {
    /// Get the display title (falls back to folder name if not set).
    #[must_use]
    pub fn display_title<'a>(&'a self, folder_name: &'a str) -> &'a str {
        self.title.as_deref().unwrap_or(folder_name)
    }

    /// Format total size as a human-readable string.
    #[must_use]
    pub fn formatted_size(&self) -> String {
        let bytes = self.total_size_bytes;
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

    /// Check if the metadata has a custom title set.
    #[must_use]
    pub const fn has_custom_title(&self) -> bool {
        self.title.is_some()
    }

    /// Check if the metadata has a description.
    #[must_use]
    pub const fn has_description(&self) -> bool {
        self.description.is_some()
    }
}

/// Information about a single track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackInfo {
    /// Track file name.
    pub file_name: String,
    /// Full path to the track.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// MP3 metadata (ID3 tags) if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Mp3Metadata>,
}

/// Metadata extracted from an MP3 file.
///
/// Contains ID3 tag information commonly found in MP3 files.
/// All fields are optional since tags may not be present.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mp3Metadata {
    /// Track title from ID3 tag.
    pub title: Option<String>,
    /// Artist name from ID3 tag.
    pub artist: Option<String>,
    /// Album name from ID3 tag.
    pub album: Option<String>,
    /// Track duration in seconds (estimated from file size if not in tags).
    pub duration_secs: Option<u64>,
    /// Track number within the album.
    pub track_number: Option<u32>,
    /// Total tracks in the album.
    pub total_tracks: Option<u32>,
    /// Release year.
    pub year: Option<i32>,
    /// Genre of the track.
    pub genre: Option<String>,
    /// Album artist (may differ from track artist for compilations).
    pub album_artist: Option<String>,
    /// Bitrate in kbps (if available).
    pub bitrate_kbps: Option<u32>,
}

impl Mp3Metadata {
    /// Check if the metadata has any meaningful content.
    #[must_use]
    pub const fn has_content(&self) -> bool {
        self.title.is_some()
            || self.artist.is_some()
            || self.album.is_some()
            || self.duration_secs.is_some()
    }

    /// Get a display title, falling back to a default if title is not set.
    #[must_use]
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or("Unknown Title")
    }

    /// Get a display artist, falling back to a default if artist is not set.
    #[must_use]
    pub fn display_artist(&self) -> &str {
        self.artist.as_deref().unwrap_or("Unknown Artist")
    }

    /// Get a display album, falling back to a default if album is not set.
    #[must_use]
    pub fn display_album(&self) -> &str {
        self.album.as_deref().unwrap_or("Unknown Album")
    }

    /// Format duration as MM:SS string.
    #[must_use]
    pub fn formatted_duration(&self) -> Option<String> {
        self.duration_secs.map(|secs| {
            let mins = secs / 60;
            let secs = secs % 60;
            format!("{mins}:{secs:02}")
        })
    }

    /// Format track number with optional total (e.g., "3/12").
    #[must_use]
    pub fn formatted_track_number(&self) -> Option<String> {
        self.track_number.map(|num| {
            if let Some(total) = self.total_tracks {
                format!("{num}/{total}")
            } else {
                num.to_string()
            }
        })
    }
}

/// Result of validating a playlist folder structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderValidationResult {
    /// Whether the folder exists.
    pub exists: bool,
    /// Whether the folder has a valid metadata file.
    pub has_metadata: bool,
    /// Whether the metadata file is valid JSON.
    pub metadata_valid: bool,
    /// Number of audio files found.
    pub audio_file_count: usize,
    /// List of issues found during validation.
    pub issues: Vec<String>,
}

impl FolderValidationResult {
    /// Check if the folder is valid (exists and has no issues).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.exists && self.issues.is_empty()
    }
}

/// Statistics about a playlist folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderStatistics {
    /// Total number of files (including non-audio).
    pub total_files: usize,
    /// Number of audio files.
    pub audio_files: usize,
    /// Number of non-audio files (excluding metadata).
    pub other_files: usize,
    /// Total size of all files in bytes.
    pub total_size_bytes: u64,
    /// Total size of audio files in bytes.
    pub audio_size_bytes: u64,
    /// Whether the folder has a metadata file.
    pub has_metadata: bool,
}

impl FolderStatistics {
    /// Format total size as a human-readable string.
    #[must_use]
    pub fn formatted_total_size(&self) -> String {
        let bytes = self.total_size_bytes;
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
}

/// Download quality setting for `YouTube` downloads.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DownloadQuality {
    /// Low quality (128 kbps).
    Low,
    /// Medium quality (192 kbps).
    #[default]
    Medium,
    /// High quality (320 kbps or best available).
    High,
}

impl std::fmt::Display for DownloadQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low (128 kbps)"),
            Self::Medium => write!(f, "Medium (192 kbps)"),
            Self::High => write!(f, "High (320 kbps)"),
        }
    }
}

/// Theme setting for the application.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Dark theme (default).
    #[default]
    Dark,
    /// Light theme.
    Light,
    /// Follow system preference.
    System,
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dark => write!(f, "Dark"),
            Self::Light => write!(f, "Light"),
            Self::System => write!(f, "System"),
        }
    }
}

/// Notification preferences for the application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationPreferences {
    /// Show notifications for download completion.
    #[serde(default = "default_true")]
    pub download_complete: bool,
    /// Show notifications for sync completion.
    #[serde(default = "default_true")]
    pub sync_complete: bool,
    /// Show notifications for errors.
    #[serde(default = "default_true")]
    pub errors: bool,
    /// Show notifications for device connections.
    #[serde(default = "default_true")]
    pub device_connected: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            download_complete: true,
            sync_complete: true,
            errors: true,
            device_connected: true,
        }
    }
}

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    /// Directory where playlists are stored.
    pub playlists_directory: String,
    /// Download quality for `YouTube` downloads.
    #[serde(default)]
    pub download_quality: DownloadQuality,
    /// Theme preference.
    #[serde(default)]
    pub theme: Theme,
    /// Notification preferences.
    #[serde(default)]
    pub notification_preferences: NotificationPreferences,
}

/// Unique identifier for a spawned task.
pub type TaskId = u64;

/// Running task count by category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskCount {
    /// Task category name.
    pub category: String,
    /// Number of running tasks in this category.
    pub count: usize,
}

// =============================================================================
// Transfer Types
// =============================================================================

/// Status of a transfer operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    /// Transfer is preparing (calculating sizes, etc.).
    Preparing,
    /// Transfer is actively copying files.
    Transferring,
    /// Transfer is verifying file integrity.
    Verifying,
    /// Transfer completed successfully.
    Completed,
    /// Transfer failed.
    Failed,
    /// Transfer was cancelled.
    Cancelled,
}

impl std::fmt::Display for TransferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparing => write!(f, "Preparing"),
            Self::Transferring => write!(f, "Transferring"),
            Self::Verifying => write!(f, "Verifying"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Progress information for a transfer operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    /// Current status of the transfer.
    pub status: TransferStatus,

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

    /// Transfer speed in bytes per second (rolling average).
    pub transfer_speed_bps: f64,

    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,

    /// Elapsed time in seconds.
    pub elapsed_secs: f64,
}

impl TransferProgress {
    /// Calculate the overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn overall_progress_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            if self.total_files == 0 {
                return 100.0;
            }
            let completed = self.files_completed + self.files_skipped;
            return (completed as f64 / self.total_files as f64) * 100.0;
        }
        (self.total_bytes_transferred as f64 / self.total_bytes as f64) * 100.0
    }

    /// Calculate the current file progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn current_file_progress_percent(&self) -> f64 {
        if self.current_file_total == 0 {
            return 100.0;
        }
        (self.current_file_bytes as f64 / self.current_file_total as f64) * 100.0
    }

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
    pub fn formatted_remaining_time(&self) -> String {
        match self.estimated_remaining_secs {
            Some(secs) if secs >= 3600.0 => {
                let hours = secs / 3600.0;
                format!("{hours:.1}h remaining")
            }
            Some(secs) if secs >= 60.0 => {
                let mins = secs / 60.0;
                format!("{mins:.1}m remaining")
            }
            Some(secs) => {
                format!("{secs:.0}s remaining")
            }
            None => "calculating...".to_string(),
        }
    }
}

/// Information about a single transferred file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferredFile {
    /// Source file path.
    pub source: String,
    /// Destination file path.
    pub destination: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// SHA-256 checksum (if verification was enabled).
    pub checksum: Option<String>,
    /// Transfer duration for this file.
    pub duration_secs: f64,
    /// Whether the file was skipped (already existed).
    pub skipped: bool,
}

/// Information about a failed transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedTransfer {
    /// Source file path.
    pub source: String,
    /// Intended destination path.
    pub destination: String,
    /// Error message.
    pub error: String,
    /// Number of retry attempts made.
    pub retry_count: u32,
}

/// Result of a transfer operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
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

    /// Total bytes skipped.
    pub bytes_skipped: u64,

    /// Total duration of the transfer operation.
    pub duration_secs: f64,

    /// Average transfer speed in bytes per second.
    pub average_speed_bps: f64,

    /// List of successfully transferred files.
    pub transferred_files: Vec<TransferredFile>,

    /// List of failed transfers.
    pub failed_transfers: Vec<FailedTransfer>,

    /// Whether the transfer was cancelled.
    pub was_cancelled: bool,

    /// Whether all files were transferred successfully.
    pub success: bool,
}

impl TransferResult {
    /// Format average speed as a human-readable string.
    #[must_use]
    pub fn formatted_average_speed(&self) -> String {
        let speed = self.average_speed_bps;
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

    /// Format bytes transferred as a human-readable string.
    #[must_use]
    pub fn formatted_bytes_transferred(&self) -> String {
        let bytes = self.bytes_transferred;
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
}

/// Configuration options for file transfers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOptions {
    /// Size of chunks for reading/writing files (in bytes).
    pub chunk_size: usize,

    /// Whether to verify file integrity after transfer using checksums.
    pub verify_integrity: bool,

    /// Whether to skip files that already exist at the destination.
    pub skip_existing: bool,

    /// Whether to verify existing files by checksum.
    pub verify_existing_checksum: bool,

    /// Whether to preserve file timestamps during transfer.
    pub preserve_timestamps: bool,

    /// Whether to continue transferring other files if one fails.
    pub continue_on_error: bool,

    /// Maximum number of retry attempts for failed transfers.
    pub max_retries: u32,
}

// =============================================================================
// Sync Types
// =============================================================================

/// Status of a sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Sync is preparing (gathering files, etc.).
    Preparing,
    /// Sync is actively transferring files.
    Transferring,
    /// Sync is verifying file integrity.
    Verifying,
    /// Sync completed successfully.
    Completed,
    /// Sync failed.
    Failed,
    /// Sync was cancelled.
    Cancelled,
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparing => write!(f, "Preparing"),
            Self::Transferring => write!(f, "Transferring"),
            Self::Verifying => write!(f, "Verifying"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Progress information for a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    /// Task ID for this sync operation.
    pub task_id: TaskId,
    /// Current status of the sync.
    pub status: SyncStatus,
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
}

impl SyncProgress {
    /// Calculate the overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn overall_progress_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            if self.total_files == 0 {
                return 100.0;
            }
            let completed = self.files_completed + self.files_skipped;
            return (completed as f64 / self.total_files as f64) * 100.0;
        }
        (self.total_bytes_transferred as f64 / self.total_bytes as f64) * 100.0
    }

    /// Calculate the current file progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn current_file_progress_percent(&self) -> f64 {
        if self.current_file_total == 0 {
            return 100.0;
        }
        (self.current_file_bytes as f64 / self.current_file_total as f64) * 100.0
    }
}

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// Task ID for this sync operation.
    pub task_id: TaskId,
    /// Whether the sync was successful.
    pub success: bool,
    /// Whether the sync was cancelled.
    pub was_cancelled: bool,
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

// =============================================================================
// YouTube URL Validation Types
// =============================================================================

/// Type of `YouTube` URL detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum YouTubeUrlType {
    /// Standard playlist URL (youtube.com/playlist?list=...)
    Playlist,
    /// Watch URL with playlist parameter (youtube.com/watch?v=...&list=...)
    WatchWithPlaylist,
    /// Single video URL without playlist
    SingleVideo,
    /// Short URL (youtu.be/...)
    ShortUrl,
    /// Invalid or unrecognized URL
    #[default]
    Invalid,
}

impl std::fmt::Display for YouTubeUrlType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Playlist => write!(f, "Playlist"),
            Self::WatchWithPlaylist => write!(f, "Watch with Playlist"),
            Self::SingleVideo => write!(f, "Single Video"),
            Self::ShortUrl => write!(f, "Short URL"),
            Self::Invalid => write!(f, "Invalid"),
        }
    }
}

/// Result of `YouTube` URL validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeUrlValidation {
    /// Whether the URL is valid.
    pub is_valid: bool,
    /// The extracted playlist ID (if valid).
    pub playlist_id: Option<String>,
    /// The normalized/canonical URL.
    pub normalized_url: Option<String>,
    /// Error message if validation failed.
    pub error_message: Option<String>,
    /// The URL type detected.
    pub url_type: YouTubeUrlType,
}

impl YouTubeUrlValidation {
    /// Create a placeholder for pending validation.
    #[must_use]
    pub const fn pending() -> Self {
        Self {
            is_valid: false,
            playlist_id: None,
            normalized_url: None,
            error_message: None,
            url_type: YouTubeUrlType::Invalid,
        }
    }

    /// Check if this is a playlist URL (either standard or watch with playlist).
    #[must_use]
    pub const fn is_playlist_url(&self) -> bool {
        matches!(
            self.url_type,
            YouTubeUrlType::Playlist | YouTubeUrlType::WatchWithPlaylist
        )
    }
}

// =============================================================================
// YouTube Download Progress Types
// =============================================================================

/// Status of a download operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DownloadStatus {
    /// Download is starting.
    #[default]
    Starting,
    /// Actively downloading.
    Downloading,
    /// Converting audio.
    Converting,
    /// Download completed successfully.
    Completed,
    /// Download failed with error message.
    Failed(String),
    /// Download was skipped (file already exists).
    Skipped,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "Starting"),
            Self::Downloading => write!(f, "Downloading"),
            Self::Converting => write!(f, "Converting"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed(msg) => write!(f, "Failed: {msg}"),
            Self::Skipped => write!(f, "Skipped"),
        }
    }
}

/// Progress information for a `YouTube` download operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
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

impl DownloadProgress {
    /// Calculate the overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn overall_progress_percent(&self) -> f64 {
        self.overall_progress * 100.0
    }

    /// Calculate the current video progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn current_progress_percent(&self) -> f64 {
        self.current_progress * 100.0
    }

    /// Check if the download is actively in progress.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self.status.as_str(),
            "downloading" | "converting" | "starting"
        )
    }

    /// Check if the download has completed (successfully or with errors).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "cancelled")
            || self.status.starts_with("failed")
    }
}

/// Category of YouTube-related errors for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    #[default]
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

    /// Check if this error category is retryable.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network | Self::YouTubeService | Self::AudioExtraction
        )
    }
}

impl std::fmt::Display for YouTubeErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title())
    }
}

/// Result of a download operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
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

impl DownloadResult {
    /// Get the display error message, preferring the user-friendly description.
    #[must_use]
    pub fn display_error(&self) -> Option<String> {
        if let Some(ref desc) = self.error_description {
            Some(desc.clone())
        } else {
            self.error_message.clone()
        }
    }

    /// Get the error title for display.
    #[must_use]
    pub fn display_error_title(&self) -> String {
        self.error_title
            .clone()
            .or_else(|| self.error_category.map(|c| c.title().to_string()))
            .unwrap_or_else(|| "Download Failed".to_string())
    }

    /// Check if the error is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        self.error_category.is_some_and(|c| c.is_retryable())
    }
}

/// Result of downloading a single video.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// =============================================================================
// Capacity Check Types
// =============================================================================

/// Warning level for capacity checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CapacityWarningLevel {
    /// Sufficient space available.
    #[default]
    Ok,
    /// Space is limited (usage will be high after sync).
    Warning,
    /// Insufficient space for sync.
    Critical,
}

impl std::fmt::Display for CapacityWarningLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "Ok"),
            Self::Warning => write!(f, "Warning"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

/// Result of checking device capacity for a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl CapacityCheckResult {
    /// Format required bytes as a human-readable string.
    #[must_use]
    pub fn formatted_required(&self) -> String {
        format_bytes(self.required_bytes)
    }

    /// Format available bytes as a human-readable string.
    #[must_use]
    pub fn formatted_available(&self) -> String {
        format_bytes(self.available_bytes)
    }

    /// Get the remaining bytes after sync (can be negative if insufficient).
    #[must_use]
    pub const fn remaining_after_sync(&self) -> i64 {
        self.available_bytes as i64 - self.required_bytes as i64
    }

    /// Format remaining bytes after sync as a human-readable string.
    #[must_use]
    pub fn formatted_remaining(&self) -> String {
        let remaining = self.remaining_after_sync();
        if remaining >= 0 {
            format_bytes(remaining as u64)
        } else {
            format!("-{}", format_bytes((-remaining) as u64))
        }
    }
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
// Notification Types
// =============================================================================

/// Type of notification to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NotificationType {
    /// Informational message.
    #[default]
    Info,
    /// Success message.
    Success,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

impl std::fmt::Display for NotificationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Success => write!(f, "success"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A toast notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Unique identifier for this notification.
    pub id: u64,
    /// The notification type.
    pub notification_type: NotificationType,
    /// The main message to display.
    pub message: String,
    /// Optional title/heading for the notification.
    pub title: Option<String>,
    /// Duration in milliseconds before auto-dismiss (None = manual dismiss only).
    pub duration_ms: Option<u64>,
}

impl Notification {
    /// Create a new notification with a unique ID.
    #[must_use]
    pub fn new(notification_type: NotificationType, message: impl Into<String>) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);

        Self {
            id: COUNTER.fetch_add(1, Ordering::Relaxed),
            notification_type,
            message: message.into(),
            title: None,
            duration_ms: Some(5000), // Default 5 seconds
        }
    }

    /// Create an info notification.
    #[must_use]
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(NotificationType::Info, message)
    }

    /// Create a success notification.
    #[must_use]
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(NotificationType::Success, message)
    }

    /// Create a warning notification.
    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(NotificationType::Warning, message)
    }

    /// Create an error notification.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        let mut notification = Self::new(NotificationType::Error, message);
        notification.duration_ms = Some(8000); // Errors stay longer
        notification
    }

    /// Set the title for this notification.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the duration for this notification.
    #[must_use]
    pub const fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Make this notification persist until manually dismissed.
    #[must_use]
    pub const fn persistent(mut self) -> Self {
        self.duration_ms = None;
        self
    }
}
