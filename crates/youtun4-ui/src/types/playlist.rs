use serde::{Deserialize, Serialize};

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
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for display formatting"
    )]
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
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for display formatting"
    )]
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
