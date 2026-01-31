//! `Youtun4` Core Library
//!
//! This crate provides the core functionality for the `Youtun4` application:
//! - Device detection for USB-mounted MP3 players
//! - Device cleanup for safe data deletion before syncing
//! - Playlist management (create, delete, sync)
//! - `YouTube` audio downloading
//! - Application configuration management
//! - Cache management for thumbnails, metadata, and temporary files
//!
//! # Error Handling
//!
//! This crate uses a comprehensive error handling framework with typed errors
//! for each domain. See the [`error`] module for details.
//!
//! ```rust,ignore
//! use youtun4_core::{Error, Result, ErrorContext};
//!
//! fn do_something() -> Result<()> {
//!     // Your code here
//!     Ok(())
//! }
//! ```

pub mod cache;
pub mod cleanup;
pub mod config;
pub mod device;
pub mod error;
pub mod fs;
pub mod integrity;
pub mod metadata;
pub mod playlist;
pub mod queue;
pub mod sync;
pub mod thumbnail;
pub mod transfer;
pub mod youtube;

pub use cache::{
    CacheCleanupStats, CacheConfig, CacheEntry, CacheEntryType, CacheManager, CacheManifest,
    CacheStats, CachedMetadata, DEFAULT_CACHE_TTL_SECS, DEFAULT_CLEANUP_TARGET,
    DEFAULT_CLEANUP_THRESHOLD, DEFAULT_MAX_CACHE_SIZE, default_cache_directory,
};
pub use cleanup::{CleanupEntry, CleanupOptions, CleanupResult, DeviceCleanupHandler};
pub use config::{AppConfig, ConfigManager, DownloadQuality, NotificationPreferences, Theme};
pub use device::{
    DEFAULT_POLL_INTERVAL, DeviceDetector, DeviceEvent, DeviceInfo, DeviceManager,
    DeviceMountHandler, DeviceWatcher, DeviceWatcherHandle, MountResult, MountStatus,
    PlatformMountHandler, UnmountResult,
};
pub use error::{
    CacheError, DeviceError, DownloadError, Error, ErrorContext, ErrorKind, FileSystemError,
    PathError, PlaylistError, Result, TransferError,
};
pub use fs::{FileMetadata, FileSystem, RealFileSystem};
pub use integrity::{
    ChecksumManifest, DEFAULT_MANIFEST_FILE, FileChecksum, FileVerificationResult,
    IntegrityVerifier, MANIFEST_VERSION, VerificationOptions, VerificationProgress,
    VerificationResult, compute_file_checksum, create_and_save_manifest, verify_directory,
};
pub use metadata::{Mp3Metadata, extract_metadata, extract_metadata_batch};
pub use playlist::{
    FolderStatistics, FolderValidationResult, PlaylistManager, PlaylistMetadata,
    SavedPlaylistMetadata, TrackInfo, is_audio_file, validate_playlist_name,
};
pub use queue::{
    DEFAULT_MAX_CONCURRENT_DOWNLOADS, DownloadPriority, DownloadQueueManager, DownloadRequest,
    MAX_CONCURRENT_DOWNLOADS, MIN_CONCURRENT_DOWNLOADS, QueueConfig, QueueEvent, QueueItem,
    QueueItemId, QueueItemStatus, QueueStats,
};
pub use sync::{
    PlaylistTransferResult, SyncOptions, SyncOrchestrator, SyncPhase, SyncProgress, SyncRequest,
    SyncResult,
};
pub use thumbnail::{
    DEFAULT_FETCH_TIMEOUT_SECS, ThumbnailManager, get_playlist_thumbnail_url,
    youtube_thumbnail_url, youtube_thumbnail_url_maxres,
};
pub use transfer::{
    DEFAULT_CHUNK_SIZE, FailedTransfer, TransferEngine, TransferItem, TransferOptions,
    TransferProgress, TransferResult, TransferStatus, TransferredFile,
};
pub use youtube::{
    DefaultYouTubeDownloader, DownloadProgress, DownloadResult, DownloadStatus, PlaylistInfo,
    RustyYtdlConfig, RustyYtdlDownloader, VideoInfo, YouTubeDownloader, YouTubeUrlType,
    YouTubeUrlValidation, extract_playlist_id, sanitize_filename, validate_youtube_url,
};
