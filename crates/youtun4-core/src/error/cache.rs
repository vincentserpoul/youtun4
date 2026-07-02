use std::path::PathBuf;
use thiserror::Error;

/// Errors related to cache operations.
#[derive(Debug, Error)]
pub enum CacheError {
    /// Cache directory not found or not accessible.
    #[error("cache directory not found: {path}")]
    DirectoryNotFound {
        /// Path to the cache directory.
        path: PathBuf,
    },

    /// Cache entry not found.
    #[error("cache entry not found: {key}")]
    EntryNotFound {
        /// Cache key that was not found.
        key: String,
    },

    /// Cache entry is corrupted or invalid.
    #[error("cache entry corrupted: {key} - {reason}")]
    EntryCorrupted {
        /// Cache key.
        key: String,
        /// Reason for corruption.
        reason: String,
    },

    /// Cache entry has expired.
    #[error("cache entry expired: {key}")]
    EntryExpired {
        /// Cache key.
        key: String,
    },

    /// Failed to serialize cache entry.
    #[error("failed to serialize cache entry '{key}': {reason}")]
    SerializationFailed {
        /// Cache key.
        key: String,
        /// Reason for failure.
        reason: String,
    },

    /// Failed to deserialize cache entry.
    #[error("failed to deserialize cache entry '{key}': {reason}")]
    DeserializationFailed {
        /// Cache key.
        key: String,
        /// Reason for failure.
        reason: String,
    },

    /// Cache is full and cannot accept new entries.
    #[error("cache is full: {current_size} bytes used, max {max_size} bytes")]
    CacheFull {
        /// Current cache size in bytes.
        current_size: u64,
        /// Maximum allowed cache size in bytes.
        max_size: u64,
    },

    /// Cache cleanup failed.
    #[error("cache cleanup failed: {reason}")]
    CleanupFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Cache initialization failed.
    #[error("cache initialization failed: {reason}")]
    InitializationFailed {
        /// Reason for failure.
        reason: String,
    },
}
