use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::time::unix_timestamp_secs;

/// Metadata for a playlist.
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
}

/// Information about a single track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackInfo {
    /// Track file name.
    pub file_name: String,
    /// Full path to the track.
    pub path: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Optional MP3 metadata (ID3 tags).
    pub metadata: Option<crate::metadata::Mp3Metadata>,
}

/// Statistics about a playlist folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderStatistics {
    /// Total number of files.
    pub total_files: usize,
    /// Number of audio files.
    pub audio_files: usize,
    /// Number of non-audio files.
    pub other_files: usize,
    /// Total size of audio files in bytes.
    pub audio_size_bytes: u64,
    /// Total size of all files in bytes.
    pub total_size_bytes: u64,
    /// Whether metadata file exists.
    pub has_metadata: bool,
}

/// Result of validating a playlist folder structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderValidationResult {
    /// Whether the folder exists.
    pub exists: bool,
    /// Whether the metadata file exists.
    pub has_metadata: bool,
    /// Whether the metadata file is valid JSON.
    pub metadata_valid: bool,
    /// Number of audio files found.
    pub audio_file_count: usize,
    /// List of issues found during validation.
    pub issues: Vec<String>,
}

impl FolderValidationResult {
    /// Check if the folder is valid (exists, has valid metadata, has audio files).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.exists && self.has_metadata && self.metadata_valid && self.audio_file_count > 0
    }
}

/// Metadata for a single track stored in playlist.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedTrackMetadata {
    /// Track file name (e.g., "song.mp3").
    pub file_name: String,
    /// Original `YouTube` video ID.
    #[serde(default)]
    pub video_id: Option<String>,
    /// Original `YouTube` video URL.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Video/track title from `YouTube`.
    #[serde(default)]
    pub title: Option<String>,
    /// Channel/artist name from `YouTube`.
    #[serde(default)]
    pub channel: Option<String>,
    /// Duration in seconds.
    #[serde(default)]
    pub duration_secs: Option<u64>,
    /// Thumbnail URL.
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    /// Download timestamp (Unix epoch seconds).
    #[serde(default)]
    pub downloaded_at: u64,
}

impl SavedTrackMetadata {
    /// Create a new track metadata from `YouTube` video info.
    #[must_use]
    pub fn from_youtube_video(
        file_name: String,
        video_id: &str,
        title: Option<String>,
        channel: Option<String>,
        duration_secs: Option<u64>,
        thumbnail_url: Option<String>,
    ) -> Self {
        let source_url = Some(format!("https://www.youtube.com/watch?v={video_id}"));
        let now = unix_timestamp_secs();

        Self {
            file_name,
            video_id: Some(video_id.to_string()),
            source_url,
            title,
            channel,
            duration_secs,
            thumbnail_url,
            downloaded_at: now,
        }
    }
}

/// Metadata saved to playlist.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedPlaylistMetadata {
    /// Optional title (different from folder name).
    #[serde(default)]
    pub title: Option<String>,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Source `YouTube` URL if applicable.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Thumbnail URL.
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    /// Creation timestamp (Unix epoch seconds).
    #[serde(default)]
    pub created_at: u64,
    /// Last modified timestamp (Unix epoch seconds).
    #[serde(default)]
    pub modified_at: u64,
    /// Number of tracks.
    #[serde(default)]
    pub track_count: usize,
    /// Total size in bytes.
    #[serde(default)]
    pub total_size_bytes: u64,
    /// Metadata for individual tracks (includes `YouTube` source URLs).
    #[serde(default)]
    pub tracks: Vec<SavedTrackMetadata>,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;

    #[test]
    fn test_playlist_metadata_equality() {
        let meta1 = PlaylistMetadata {
            name: "Test".to_string(),
            source_url: None,
            created_at: 1000,
            modified_at: 2000,
            track_count: 5,
            total_bytes: 1024,
        };
        let meta2 = meta1.clone();
        assert_eq!(meta1, meta2);
    }

    #[test]
    fn test_track_info_equality() {
        let track1 = TrackInfo {
            file_name: "song.mp3".to_string(),
            path: PathBuf::from("/test/song.mp3"),
            size_bytes: 1024,
            metadata: None,
        };
        let track2 = track1.clone();
        assert_eq!(track1, track2);
    }

    #[test]
    fn test_folder_statistics_equality() {
        let stats1 = FolderStatistics {
            total_files: 10,
            audio_files: 8,
            other_files: 2,
            audio_size_bytes: 1000,
            total_size_bytes: 1200,
            has_metadata: true,
        };
        let stats2 = stats1.clone();
        assert_eq!(stats1, stats2);
    }

    #[test]
    fn test_folder_validation_result_is_valid() {
        let valid = FolderValidationResult {
            exists: true,
            has_metadata: true,
            metadata_valid: true,
            audio_file_count: 5,
            issues: vec![],
        };
        assert!(valid.is_valid());

        let no_audio = FolderValidationResult {
            exists: true,
            has_metadata: true,
            metadata_valid: true,
            audio_file_count: 0,
            issues: vec![],
        };
        assert!(!no_audio.is_valid());
    }
}
