//! Shared types for the MP3YouTube UI.
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
    pub fn used_bytes(&self) -> u64 {
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

/// Metadata for a playlist.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistMetadata {
    /// Playlist name (also the folder name).
    pub name: String,
    /// Original YouTube playlist URL (if created from YouTube).
    pub source_url: Option<String>,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last modified timestamp (Unix epoch seconds).
    pub modified_at: u64,
    /// Number of tracks in the playlist.
    pub track_count: usize,
    /// Total size in bytes.
    pub total_bytes: u64,
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
}
