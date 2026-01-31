//! Cache management for downloaded thumbnails, metadata, and temporary files.
//!
//! This module provides a comprehensive caching system with:
//! - Metadata caching for extracted MP3 ID3 tags
//! - Thumbnail caching for `YouTube` video thumbnails
//! - Configurable cache size limits and TTL (time-to-live)
//! - Automatic cache cleanup policies
//!
//! # Example
//!
//! ```rust,ignore
//! use youtun4_core::cache::{CacheManager, CacheConfig};
//!
//! let config = CacheConfig::default();
//! let cache = CacheManager::new(config)?;
//!
//! // Cache metadata
//! cache.put_metadata("video_id", metadata)?;
//!
//! // Retrieve cached metadata
//! if let Some(metadata) = cache.get_metadata("video_id")? {
//!     println!("Found cached metadata: {:?}", metadata);
//! }
//!
//! // Clean up expired entries
//! let stats = cache.cleanup()?;
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{CacheError, Error, FileSystemError, Result};
use crate::metadata::Mp3Metadata;

/// Default maximum cache size in bytes (100 MB).
pub const DEFAULT_MAX_CACHE_SIZE: u64 = 100 * 1024 * 1024;

/// Default cache TTL in seconds (7 days).
pub const DEFAULT_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;

/// Default cleanup threshold percentage (80%).
/// Cache cleanup triggers when usage exceeds this percentage.
pub const DEFAULT_CLEANUP_THRESHOLD: f64 = 0.80;

/// Default cleanup target percentage (60%).
/// Cache cleanup reduces usage to this percentage.
pub const DEFAULT_CLEANUP_TARGET: f64 = 0.60;

/// Cache subdirectory names.
const METADATA_CACHE_DIR: &str = "metadata";
const THUMBNAIL_CACHE_DIR: &str = "thumbnails";
const TEMP_CACHE_DIR: &str = "temp";
const CACHE_MANIFEST_FILE: &str = "cache_manifest.json";

/// Cache configuration options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheConfig {
    /// Whether caching is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum total cache size in bytes.
    #[serde(default = "default_max_size")]
    pub max_size_bytes: u64,

    /// Time-to-live for cache entries in seconds.
    #[serde(default = "default_ttl")]
    pub ttl_secs: u64,

    /// Cleanup threshold percentage (0.0 - 1.0).
    /// Cleanup triggers when cache usage exceeds this threshold.
    #[serde(default = "default_cleanup_threshold")]
    pub cleanup_threshold: f64,

    /// Cleanup target percentage (0.0 - 1.0).
    /// Cleanup reduces cache usage to this target.
    #[serde(default = "default_cleanup_target")]
    pub cleanup_target: f64,

    /// Whether to cache MP3 metadata.
    #[serde(default = "default_true")]
    pub cache_metadata: bool,

    /// Whether to cache thumbnails.
    #[serde(default = "default_true")]
    pub cache_thumbnails: bool,

    /// Custom cache directory path (optional).
    /// If not set, uses default platform-specific location.
    #[serde(default)]
    pub custom_cache_dir: Option<PathBuf>,
}

const fn default_true() -> bool {
    true
}

const fn default_max_size() -> u64 {
    DEFAULT_MAX_CACHE_SIZE
}

const fn default_ttl() -> u64 {
    DEFAULT_CACHE_TTL_SECS
}

const fn default_cleanup_threshold() -> f64 {
    DEFAULT_CLEANUP_THRESHOLD
}

const fn default_cleanup_target() -> f64 {
    DEFAULT_CLEANUP_TARGET
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size_bytes: DEFAULT_MAX_CACHE_SIZE,
            ttl_secs: DEFAULT_CACHE_TTL_SECS,
            cleanup_threshold: DEFAULT_CLEANUP_THRESHOLD,
            cleanup_target: DEFAULT_CLEANUP_TARGET,
            cache_metadata: true,
            cache_thumbnails: true,
            custom_cache_dir: None,
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum cache size.
    #[must_use]
    pub const fn with_max_size(mut self, max_size_bytes: u64) -> Self {
        self.max_size_bytes = max_size_bytes;
        self
    }

    /// Set the cache TTL.
    #[must_use]
    pub const fn with_ttl(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = ttl_secs;
        self
    }

    /// Set the cache directory.
    #[must_use]
    pub fn with_cache_dir(mut self, path: PathBuf) -> Self {
        self.custom_cache_dir = Some(path);
        self
    }

    /// Disable caching entirely.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

/// Entry in the cache manifest tracking cached items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Unique cache key.
    pub key: String,
    /// Relative path to the cached file.
    pub path: PathBuf,
    /// Size of the cached data in bytes.
    pub size_bytes: u64,
    /// Timestamp when the entry was created (Unix epoch seconds).
    pub created_at: u64,
    /// Timestamp when the entry was last accessed (Unix epoch seconds).
    pub last_accessed_at: u64,
    /// Type of cached data.
    pub entry_type: CacheEntryType,
}

impl CacheEntry {
    /// Check if this entry has expired based on the given TTL.
    #[must_use]
    pub fn is_expired(&self, ttl_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now.saturating_sub(self.created_at) > ttl_secs
    }

    /// Update the last accessed timestamp.
    pub fn touch(&mut self) {
        self.last_accessed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
}

/// Type of cached data.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CacheEntryType {
    /// MP3 metadata (ID3 tags).
    Metadata,
    /// Video thumbnail image.
    Thumbnail,
    /// Temporary file.
    Temp,
}

/// Cache manifest storing all cache entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheManifest {
    /// Version of the cache manifest format.
    pub version: u32,
    /// Total size of all cached data in bytes.
    pub total_size_bytes: u64,
    /// Map of cache keys to entries.
    pub entries: HashMap<String, CacheEntry>,
    /// Timestamp of last cleanup (Unix epoch seconds).
    pub last_cleanup_at: u64,
}

impl Default for CacheManifest {
    fn default() -> Self {
        Self {
            version: 1,
            total_size_bytes: 0,
            entries: HashMap::new(),
            last_cleanup_at: 0,
        }
    }
}

/// Cached MP3 metadata with additional cache information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMetadata {
    /// The MP3 metadata.
    pub metadata: Mp3Metadata,
    /// File path that was analyzed.
    pub source_path: PathBuf,
    /// File modification time when metadata was extracted.
    pub source_modified_at: u64,
}

/// Statistics from a cache cleanup operation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheCleanupStats {
    /// Number of entries removed.
    pub entries_removed: usize,
    /// Bytes freed by cleanup.
    pub bytes_freed: u64,
    /// Number of expired entries removed.
    pub expired_entries: usize,
    /// Number of entries removed for space.
    pub space_reclaimed_entries: usize,
    /// Duration of the cleanup operation.
    pub duration_ms: u64,
}

/// Statistics about the current cache state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total number of entries in cache.
    pub total_entries: usize,
    /// Number of metadata entries.
    pub metadata_entries: usize,
    /// Number of thumbnail entries.
    pub thumbnail_entries: usize,
    /// Number of temp entries.
    pub temp_entries: usize,
    /// Total size in bytes.
    pub total_size_bytes: u64,
    /// Maximum allowed size in bytes.
    pub max_size_bytes: u64,
    /// Usage percentage (0.0 - 1.0).
    pub usage_percentage: f64,
    /// Whether caching is enabled.
    pub enabled: bool,
}

/// Cache manager for handling all caching operations.
pub struct CacheManager {
    /// Cache configuration.
    config: CacheConfig,
    /// Root cache directory.
    cache_dir: PathBuf,
    /// Cache manifest (in-memory).
    manifest: CacheManifest,
}

impl CacheManager {
    /// Create a new cache manager with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache directory cannot be created or the manifest
    /// cannot be loaded.
    pub fn new(config: CacheConfig) -> Result<Self> {
        let cache_dir = config
            .custom_cache_dir
            .clone()
            .unwrap_or_else(default_cache_directory);

        if !config.enabled {
            debug!("Cache is disabled");
            return Ok(Self {
                config,
                cache_dir,
                manifest: CacheManifest::default(),
            });
        }

        // Ensure cache directories exist
        Self::ensure_cache_directories(&cache_dir)?;

        // Load or create manifest
        let manifest = Self::load_or_create_manifest(&cache_dir)?;

        info!(
            "Cache initialized at {} with {} entries ({} bytes)",
            cache_dir.display(),
            manifest.entries.len(),
            manifest.total_size_bytes
        );

        Ok(Self {
            config,
            cache_dir,
            manifest,
        })
    }

    /// Create cache directories if they don't exist.
    fn ensure_cache_directories(cache_dir: &Path) -> Result<()> {
        let dirs = [
            cache_dir.to_path_buf(),
            cache_dir.join(METADATA_CACHE_DIR),
            cache_dir.join(THUMBNAIL_CACHE_DIR),
            cache_dir.join(TEMP_CACHE_DIR),
        ];

        for dir in &dirs {
            if !dir.exists() {
                fs::create_dir_all(dir).map_err(|e| {
                    Error::Cache(CacheError::InitializationFailed {
                        reason: format!(
                            "Failed to create cache directory {}: {}",
                            dir.display(),
                            e
                        ),
                    })
                })?;
            }
        }

        Ok(())
    }

    /// Load the cache manifest from disk or create a new one.
    fn load_or_create_manifest(cache_dir: &Path) -> Result<CacheManifest> {
        let manifest_path = cache_dir.join(CACHE_MANIFEST_FILE);

        if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path).map_err(|e| {
                Error::FileSystem(FileSystemError::ReadFailed {
                    path: manifest_path.clone(),
                    reason: e.to_string(),
                })
            })?;

            match serde_json::from_str(&content) {
                Ok(manifest) => {
                    debug!("Loaded cache manifest from {}", manifest_path.display());
                    return Ok(manifest);
                }
                Err(e) => {
                    warn!("Failed to parse cache manifest, creating new one: {}", e);
                }
            }
        }

        Ok(CacheManifest::default())
    }

    /// Save the cache manifest to disk.
    fn save_manifest(&self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let manifest_path = self.cache_dir.join(CACHE_MANIFEST_FILE);
        let content = serde_json::to_string_pretty(&self.manifest)?;

        fs::write(&manifest_path, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: manifest_path,
                reason: e.to_string(),
            })
        })
    }

    /// Get the cache directory path.
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Check if caching is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get cache statistics.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        let mut metadata_entries = 0;
        let mut thumbnail_entries = 0;
        let mut temp_entries = 0;

        for entry in self.manifest.entries.values() {
            match entry.entry_type {
                CacheEntryType::Metadata => metadata_entries += 1,
                CacheEntryType::Thumbnail => thumbnail_entries += 1,
                CacheEntryType::Temp => temp_entries += 1,
            }
        }

        let usage_percentage = if self.config.max_size_bytes > 0 {
            self.manifest.total_size_bytes as f64 / self.config.max_size_bytes as f64
        } else {
            0.0
        };

        CacheStats {
            total_entries: self.manifest.entries.len(),
            metadata_entries,
            thumbnail_entries,
            temp_entries,
            total_size_bytes: self.manifest.total_size_bytes,
            max_size_bytes: self.config.max_size_bytes,
            usage_percentage,
            enabled: self.config.enabled,
        }
    }

    // =========================================================================
    // Metadata Caching
    // =========================================================================

    /// Generate a cache key for metadata based on file path and modification time.
    fn metadata_cache_key(path: &Path, modified_at: u64) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        modified_at.hash(&mut hasher);
        format!("meta_{:016x}", hasher.finish())
    }

    /// Cache MP3 metadata for a file.
    ///
    /// # Errors
    ///
    /// Returns an error if caching fails or the cache is full.
    pub fn put_metadata(&mut self, path: &Path, metadata: Mp3Metadata) -> Result<()> {
        if !self.config.enabled || !self.config.cache_metadata {
            return Ok(());
        }

        let modified_at = fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let key = Self::metadata_cache_key(path, modified_at);
        let cache_path = self
            .cache_dir
            .join(METADATA_CACHE_DIR)
            .join(format!("{key}.json"));

        let cached = CachedMetadata {
            metadata,
            source_path: path.to_path_buf(),
            source_modified_at: modified_at,
        };

        let content = serde_json::to_string(&cached)?;
        let size = content.len() as u64;

        // Check if we need to make space
        self.ensure_space(size)?;

        fs::write(&cache_path, &content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: cache_path.clone(),
                reason: e.to_string(),
            })
        })?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entry = CacheEntry {
            key: key.clone(),
            path: PathBuf::from(METADATA_CACHE_DIR).join(format!("{key}.json")),
            size_bytes: size,
            created_at: now,
            last_accessed_at: now,
            entry_type: CacheEntryType::Metadata,
        };

        // Remove old entry if exists
        if let Some(old_entry) = self.manifest.entries.remove(&key) {
            self.manifest.total_size_bytes = self
                .manifest
                .total_size_bytes
                .saturating_sub(old_entry.size_bytes);
        }

        self.manifest.total_size_bytes += size;
        self.manifest.entries.insert(key, entry);
        self.save_manifest()?;

        debug!("Cached metadata for {}", path.display());
        Ok(())
    }

    /// Get cached metadata for a file.
    ///
    /// Returns `None` if not cached or if the cache entry is stale.
    pub fn get_metadata(&mut self, path: &Path) -> Result<Option<Mp3Metadata>> {
        if !self.config.enabled || !self.config.cache_metadata {
            return Ok(None);
        }

        let modified_at = fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let key = Self::metadata_cache_key(path, modified_at);

        // Check if entry exists and is not expired
        let entry = match self.manifest.entries.get(&key) {
            Some(e) if !e.is_expired(self.config.ttl_secs) => e.clone(),
            _ => return Ok(None),
        };

        let cache_path = self.cache_dir.join(&entry.path);

        let Ok(content) = fs::read_to_string(&cache_path) else {
            // File doesn't exist, remove from manifest
            self.remove_entry(&key)?;
            return Ok(None);
        };

        let cached: CachedMetadata = if let Ok(c) = serde_json::from_str(&content) {
            c
        } else {
            // Invalid data, remove from manifest
            self.remove_entry(&key)?;
            return Ok(None);
        };

        // Verify the source file hasn't changed
        if cached.source_modified_at != modified_at {
            self.remove_entry(&key)?;
            return Ok(None);
        }

        // Update last accessed time
        if let Some(entry) = self.manifest.entries.get_mut(&key) {
            entry.touch();
        }

        debug!("Cache hit for metadata: {}", path.display());
        Ok(Some(cached.metadata))
    }

    // =========================================================================
    // Thumbnail Caching
    // =========================================================================

    /// Generate a cache key for a thumbnail.
    fn thumbnail_cache_key(video_id: &str) -> String {
        format!("thumb_{video_id}")
    }

    /// Cache a thumbnail image.
    ///
    /// # Errors
    ///
    /// Returns an error if caching fails.
    pub fn put_thumbnail(&mut self, video_id: &str, data: &[u8]) -> Result<()> {
        if !self.config.enabled || !self.config.cache_thumbnails {
            return Ok(());
        }

        let key = Self::thumbnail_cache_key(video_id);
        let cache_path = self
            .cache_dir
            .join(THUMBNAIL_CACHE_DIR)
            .join(format!("{key}.jpg"));

        let size = data.len() as u64;

        // Check if we need to make space
        self.ensure_space(size)?;

        fs::write(&cache_path, data).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: cache_path.clone(),
                reason: e.to_string(),
            })
        })?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entry = CacheEntry {
            key: key.clone(),
            path: PathBuf::from(THUMBNAIL_CACHE_DIR).join(format!("{key}.jpg")),
            size_bytes: size,
            created_at: now,
            last_accessed_at: now,
            entry_type: CacheEntryType::Thumbnail,
        };

        // Remove old entry if exists
        if let Some(old_entry) = self.manifest.entries.remove(&key) {
            self.manifest.total_size_bytes = self
                .manifest
                .total_size_bytes
                .saturating_sub(old_entry.size_bytes);
        }

        self.manifest.total_size_bytes += size;
        self.manifest.entries.insert(key, entry);
        self.save_manifest()?;

        debug!("Cached thumbnail for video {}", video_id);
        Ok(())
    }

    /// Get a cached thumbnail.
    ///
    /// Returns `None` if not cached or expired.
    pub fn get_thumbnail(&mut self, video_id: &str) -> Result<Option<Vec<u8>>> {
        if !self.config.enabled || !self.config.cache_thumbnails {
            return Ok(None);
        }

        let key = Self::thumbnail_cache_key(video_id);

        // Check if entry exists and is not expired
        let entry = match self.manifest.entries.get(&key) {
            Some(e) if !e.is_expired(self.config.ttl_secs) => e.clone(),
            _ => return Ok(None),
        };

        let cache_path = self.cache_dir.join(&entry.path);

        let Ok(data) = fs::read(&cache_path) else {
            self.remove_entry(&key)?;
            return Ok(None);
        };

        // Update last accessed time
        if let Some(entry) = self.manifest.entries.get_mut(&key) {
            entry.touch();
        }

        debug!("Cache hit for thumbnail: {}", video_id);
        Ok(Some(data))
    }

    /// Check if a thumbnail is cached.
    #[must_use]
    pub fn has_thumbnail(&self, video_id: &str) -> bool {
        if !self.config.enabled || !self.config.cache_thumbnails {
            return false;
        }

        let key = Self::thumbnail_cache_key(video_id);
        self.manifest
            .entries
            .get(&key)
            .is_some_and(|e| !e.is_expired(self.config.ttl_secs))
    }

    // =========================================================================
    // Temp File Management
    // =========================================================================

    /// Get the temp directory path for temporary files.
    #[must_use]
    pub fn temp_dir(&self) -> PathBuf {
        self.cache_dir.join(TEMP_CACHE_DIR)
    }

    /// Create a temporary file path with the given extension.
    #[must_use]
    pub fn temp_file_path(&self, prefix: &str, extension: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let filename = format!("{prefix}_{timestamp}.{extension}");
        self.temp_dir().join(filename)
    }

    /// Clean up all temporary files.
    pub fn cleanup_temp(&mut self) -> Result<CacheCleanupStats> {
        let temp_dir = self.temp_dir();
        let mut stats = CacheCleanupStats::default();
        let start = std::time::Instant::now();

        if !temp_dir.exists() {
            return Ok(stats);
        }

        // Remove temp entries from manifest
        let temp_keys: Vec<String> = self
            .manifest
            .entries
            .iter()
            .filter(|(_, e)| e.entry_type == CacheEntryType::Temp)
            .map(|(k, _)| k.clone())
            .collect();

        for key in temp_keys {
            if let Some(entry) = self.manifest.entries.remove(&key) {
                self.manifest.total_size_bytes = self
                    .manifest
                    .total_size_bytes
                    .saturating_sub(entry.size_bytes);
                stats.entries_removed += 1;
                stats.bytes_freed += entry.size_bytes;
            }
        }

        // Also clean up any orphaned temp files
        if let Ok(entries) = fs::read_dir(&temp_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata()
                    && metadata.is_file()
                {
                    let _ = fs::remove_file(entry.path());
                    stats.bytes_freed += metadata.len();
                }
            }
        }

        self.save_manifest()?;
        stats.duration_ms = start.elapsed().as_millis() as u64;

        info!(
            "Cleaned up temp files: {} entries, {} bytes freed",
            stats.entries_removed, stats.bytes_freed
        );
        Ok(stats)
    }

    // =========================================================================
    // Cache Maintenance
    // =========================================================================

    /// Remove a cache entry.
    fn remove_entry(&mut self, key: &str) -> Result<()> {
        if let Some(entry) = self.manifest.entries.remove(key) {
            let cache_path = self.cache_dir.join(&entry.path);
            let _ = fs::remove_file(&cache_path);
            self.manifest.total_size_bytes = self
                .manifest
                .total_size_bytes
                .saturating_sub(entry.size_bytes);
            self.save_manifest()?;
        }
        Ok(())
    }

    /// Ensure there's enough space for a new entry.
    fn ensure_space(&mut self, required_bytes: u64) -> Result<()> {
        if self.manifest.total_size_bytes + required_bytes <= self.config.max_size_bytes {
            return Ok(());
        }

        // Need to free up space
        let target_size = (self.config.max_size_bytes as f64 * self.config.cleanup_target) as u64;
        self.cleanup_to_target(target_size)?;

        // Check if we have enough space now
        if self.manifest.total_size_bytes + required_bytes > self.config.max_size_bytes {
            return Err(Error::cache_full(
                self.manifest.total_size_bytes,
                self.config.max_size_bytes,
            ));
        }

        Ok(())
    }

    /// Clean up cache to reach target size.
    fn cleanup_to_target(&mut self, target_size: u64) -> Result<CacheCleanupStats> {
        let mut stats = CacheCleanupStats::default();
        let start = std::time::Instant::now();

        // First, remove expired entries
        let expired_keys: Vec<String> = self
            .manifest
            .entries
            .iter()
            .filter(|(_, e)| e.is_expired(self.config.ttl_secs))
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired_keys {
            if let Some(entry) = self.manifest.entries.remove(&key) {
                let cache_path = self.cache_dir.join(&entry.path);
                let _ = fs::remove_file(&cache_path);
                self.manifest.total_size_bytes = self
                    .manifest
                    .total_size_bytes
                    .saturating_sub(entry.size_bytes);
                stats.entries_removed += 1;
                stats.expired_entries += 1;
                stats.bytes_freed += entry.size_bytes;
            }
        }

        // If still over target, remove by LRU
        if self.manifest.total_size_bytes > target_size {
            // Sort entries by last accessed time (oldest first)
            let mut entries: Vec<_> = self.manifest.entries.iter().collect();
            entries.sort_by_key(|(_, e)| e.last_accessed_at);

            let keys_to_remove: Vec<String> = entries
                .into_iter()
                .take_while(|_| self.manifest.total_size_bytes > target_size)
                .map(|(k, _)| k.clone())
                .collect();

            for key in keys_to_remove {
                if let Some(entry) = self.manifest.entries.remove(&key) {
                    let cache_path = self.cache_dir.join(&entry.path);
                    let _ = fs::remove_file(&cache_path);
                    self.manifest.total_size_bytes = self
                        .manifest
                        .total_size_bytes
                        .saturating_sub(entry.size_bytes);
                    stats.entries_removed += 1;
                    stats.space_reclaimed_entries += 1;
                    stats.bytes_freed += entry.size_bytes;
                }
            }
        }

        self.manifest.last_cleanup_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.save_manifest()?;
        stats.duration_ms = start.elapsed().as_millis() as u64;

        Ok(stats)
    }

    /// Perform a full cache cleanup.
    ///
    /// This removes:
    /// - All expired entries
    /// - Entries exceeding the cleanup threshold (by LRU)
    /// - Orphaned files not in the manifest
    pub fn cleanup(&mut self) -> Result<CacheCleanupStats> {
        if !self.config.enabled {
            return Ok(CacheCleanupStats::default());
        }

        let start = std::time::Instant::now();
        let threshold_size =
            (self.config.max_size_bytes as f64 * self.config.cleanup_threshold) as u64;

        let mut stats = if self.manifest.total_size_bytes > threshold_size {
            let target_size =
                (self.config.max_size_bytes as f64 * self.config.cleanup_target) as u64;
            self.cleanup_to_target(target_size)?
        } else {
            // Just clean expired entries
            self.cleanup_expired()?
        };

        // Clean up orphaned files
        let orphan_stats = self.cleanup_orphaned_files();
        stats.entries_removed += orphan_stats.entries_removed;
        stats.bytes_freed += orphan_stats.bytes_freed;

        stats.duration_ms = start.elapsed().as_millis() as u64;

        info!(
            "Cache cleanup complete: {} entries removed, {} bytes freed in {}ms",
            stats.entries_removed, stats.bytes_freed, stats.duration_ms
        );

        Ok(stats)
    }

    /// Remove only expired entries.
    fn cleanup_expired(&mut self) -> Result<CacheCleanupStats> {
        let mut stats = CacheCleanupStats::default();

        let expired_keys: Vec<String> = self
            .manifest
            .entries
            .iter()
            .filter(|(_, e)| e.is_expired(self.config.ttl_secs))
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired_keys {
            if let Some(entry) = self.manifest.entries.remove(&key) {
                let cache_path = self.cache_dir.join(&entry.path);
                let _ = fs::remove_file(&cache_path);
                self.manifest.total_size_bytes = self
                    .manifest
                    .total_size_bytes
                    .saturating_sub(entry.size_bytes);
                stats.entries_removed += 1;
                stats.expired_entries += 1;
                stats.bytes_freed += entry.size_bytes;
            }
        }

        if stats.entries_removed > 0 {
            self.save_manifest()?;
        }

        Ok(stats)
    }

    /// Clean up files that exist on disk but not in the manifest.
    fn cleanup_orphaned_files(&self) -> CacheCleanupStats {
        let mut stats = CacheCleanupStats::default();

        let subdirs = [METADATA_CACHE_DIR, THUMBNAIL_CACHE_DIR, TEMP_CACHE_DIR];

        for subdir in &subdirs {
            let dir_path = self.cache_dir.join(subdir);
            if !dir_path.exists() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(&dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let relative_path = PathBuf::from(subdir).join(entry.file_name());

                        // Check if this file is in the manifest
                        let in_manifest = self
                            .manifest
                            .entries
                            .values()
                            .any(|e| e.path == relative_path);

                        if !in_manifest {
                            if let Ok(metadata) = entry.metadata() {
                                stats.bytes_freed += metadata.len();
                            }
                            let _ = fs::remove_file(&path);
                            stats.entries_removed += 1;
                            debug!("Removed orphaned file: {}", path.display());
                        }
                    }
                }
            }
        }

        stats
    }

    /// Clear all cached data.
    pub fn clear(&mut self) -> Result<CacheCleanupStats> {
        let stats = CacheCleanupStats {
            entries_removed: self.manifest.entries.len(),
            bytes_freed: self.manifest.total_size_bytes,
            ..Default::default()
        };

        // Remove all cache files
        let subdirs = [METADATA_CACHE_DIR, THUMBNAIL_CACHE_DIR, TEMP_CACHE_DIR];
        for subdir in &subdirs {
            let dir_path = self.cache_dir.join(subdir);
            if dir_path.exists() {
                let _ = fs::remove_dir_all(&dir_path);
                let _ = fs::create_dir_all(&dir_path);
            }
        }

        // Reset manifest
        self.manifest = CacheManifest::default();
        self.save_manifest()?;

        info!(
            "Cache cleared: {} entries, {} bytes",
            stats.entries_removed, stats.bytes_freed
        );

        Ok(stats)
    }

    /// Get the cache configuration.
    #[must_use]
    pub const fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Update the cache configuration.
    ///
    /// Note: Changes to `custom_cache_dir` require creating a new `CacheManager`.
    pub fn update_config(&mut self, config: CacheConfig) -> Result<()> {
        // Don't allow changing the cache directory through update
        let config = CacheConfig {
            custom_cache_dir: self.config.custom_cache_dir.clone(),
            ..config
        };
        self.config = config;
        Ok(())
    }
}

/// Get the default cache directory path.
#[must_use]
pub fn default_cache_directory() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("youtun4")
        .join("cache")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_cache(temp_dir: &TempDir) -> CacheManager {
        let config = CacheConfig::new().with_cache_dir(temp_dir.path().to_path_buf());
        CacheManager::new(config).expect("Failed to create cache manager")
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_size_bytes, DEFAULT_MAX_CACHE_SIZE);
        assert_eq!(config.ttl_secs, DEFAULT_CACHE_TTL_SECS);
        assert!(config.cache_metadata);
        assert!(config.cache_thumbnails);
    }

    #[test]
    fn test_cache_config_builder() {
        let config = CacheConfig::new()
            .with_max_size(50 * 1024 * 1024)
            .with_ttl(3600)
            .with_cache_dir(PathBuf::from("/tmp/cache"));

        assert_eq!(config.max_size_bytes, 50 * 1024 * 1024);
        assert_eq!(config.ttl_secs, 3600);
        assert_eq!(config.custom_cache_dir, Some(PathBuf::from("/tmp/cache")));
    }

    #[test]
    fn test_cache_config_disabled() {
        let config = CacheConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_cache_manager_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = create_test_cache(&temp_dir);

        assert!(cache.is_enabled());
        assert!(cache.cache_dir().exists());
    }

    #[test]
    fn test_cache_manager_disabled() {
        let config = CacheConfig::disabled();
        let cache = CacheManager::new(config).expect("Failed to create cache manager");

        assert!(!cache.is_enabled());
    }

    #[test]
    fn test_cache_stats() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = create_test_cache(&temp_dir);

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert!(stats.enabled);
    }

    #[test]
    fn test_thumbnail_caching() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let mut cache = create_test_cache(&temp_dir);

        let video_id = "test_video_123";
        let thumbnail_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header bytes

        // Cache thumbnail
        cache
            .put_thumbnail(video_id, &thumbnail_data)
            .expect("Failed to cache thumbnail");

        // Check it's cached
        assert!(cache.has_thumbnail(video_id));

        // Retrieve thumbnail
        let retrieved = cache
            .get_thumbnail(video_id)
            .expect("Failed to get thumbnail")
            .expect("Thumbnail should exist");

        assert_eq!(retrieved, thumbnail_data);

        // Check stats
        let stats = cache.stats();
        assert_eq!(stats.thumbnail_entries, 1);
    }

    #[test]
    fn test_cache_entry_expiration() {
        let entry = CacheEntry {
            key: "test".to_string(),
            path: PathBuf::from("test.json"),
            size_bytes: 100,
            created_at: 0, // Very old timestamp
            last_accessed_at: 0,
            entry_type: CacheEntryType::Metadata,
        };

        assert!(entry.is_expired(3600)); // Should be expired
    }

    #[test]
    fn test_cache_cleanup() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let mut cache = create_test_cache(&temp_dir);

        // Add some thumbnails
        for i in 0..5 {
            let video_id = format!("video_{i}");
            let data = vec![0u8; 1000];
            cache.put_thumbnail(&video_id, &data).unwrap();
        }

        let stats_before = cache.stats();
        assert_eq!(stats_before.total_entries, 5);

        // Clear the cache
        let cleanup_stats = cache.clear().expect("Failed to clear cache");
        assert_eq!(cleanup_stats.entries_removed, 5);

        let stats_after = cache.stats();
        assert_eq!(stats_after.total_entries, 0);
    }

    #[test]
    fn test_cache_temp_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = create_test_cache(&temp_dir);

        let temp_path = cache.temp_dir();
        assert!(temp_path.exists());
        assert!(temp_path.ends_with("temp"));
    }

    #[test]
    fn test_cache_temp_file_path() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = create_test_cache(&temp_dir);

        let path = cache.temp_file_path("download", "mp3");
        assert!(path.to_string_lossy().contains("download_"));
        assert!(path.to_string_lossy().ends_with(".mp3"));
    }

    #[test]
    fn test_default_cache_directory() {
        let dir = default_cache_directory();
        assert!(dir.to_string_lossy().contains("youtun4"));
        assert!(dir.to_string_lossy().contains("cache"));
    }

    #[test]
    fn test_cache_entry_touch() {
        let mut entry = CacheEntry {
            key: "test".to_string(),
            path: PathBuf::from("test.json"),
            size_bytes: 100,
            created_at: 0,
            last_accessed_at: 0,
            entry_type: CacheEntryType::Metadata,
        };

        entry.touch();
        assert!(entry.last_accessed_at > 0);
    }

    #[test]
    fn test_cache_manifest_default() {
        let manifest = CacheManifest::default();
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.total_size_bytes, 0);
        assert!(manifest.entries.is_empty());
    }

    #[test]
    fn test_cache_config_serialization() {
        let config = CacheConfig::default();
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: CacheConfig = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(config, deserialized);
    }
}
