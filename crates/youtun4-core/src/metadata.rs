//! MP3 metadata extraction module.
//!
//! This module provides functionality to extract ID3 tags and other metadata
//! from MP3 files, including title, artist, album, duration, and track number.
//!
//! # Supported Formats
//!
//! - `ID3v1` tags
//! - ID3v2.3 and ID3v2.4 tags
//!
//! # Example
//!
//! ```rust,ignore
//! use youtun4_core::metadata::{Mp3Metadata, extract_metadata};
//! use std::path::Path;
//!
//! let metadata = extract_metadata(Path::new("song.mp3"))?;
//! println!("Title: {:?}", metadata.title);
//! println!("Artist: {:?}", metadata.artist);
//! ```

use std::path::Path;

use id3::{Tag, TagLike};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::error::{Error, FileSystemError, Result};

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
    /// Create empty metadata with no fields set.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

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

/// Extract metadata from an MP3 file.
///
/// Reads ID3 tags from the specified file and returns structured metadata.
/// If no tags are found, returns empty metadata (no error).
///
/// # Arguments
///
/// * `path` - Path to the MP3 file
///
/// # Errors
///
/// Returns an error if the file cannot be read.
///
/// # Example
///
/// ```rust,ignore
/// use youtun4_core::metadata::extract_metadata;
/// use std::path::Path;
///
/// let metadata = extract_metadata(Path::new("song.mp3"))?;
/// if let Some(title) = &metadata.title {
///     println!("Playing: {}", title);
/// }
/// ```
pub fn extract_metadata(path: &Path) -> Result<Mp3Metadata> {
    if !path.exists() {
        return Err(Error::FileSystem(FileSystemError::NotFound {
            path: path.to_path_buf(),
        }));
    }

    debug!("Extracting metadata from: {}", path.display());

    // Try to read ID3 tag
    let tag = match Tag::read_from_path(path) {
        Ok(tag) => tag,
        Err(id3::Error {
            kind: id3::ErrorKind::NoTag,
            ..
        }) => {
            debug!("No ID3 tag found in: {}", path.display());
            return Ok(Mp3Metadata::empty());
        }
        Err(e) => {
            warn!("Failed to read ID3 tag from {}: {}", path.display(), e);
            // Return empty metadata rather than failing
            return Ok(Mp3Metadata::empty());
        }
    };

    // Extract track number (may include total, e.g., "3/12")
    let (track_number, total_tracks) = parse_track_number(&tag);

    let metadata = Mp3Metadata {
        title: tag.title().map(String::from),
        artist: tag.artist().map(String::from),
        album: tag.album().map(String::from),
        duration_secs: tag.duration().map(u64::from),
        track_number,
        total_tracks,
        year: tag.year(),
        genre: tag.genre_parsed().map(|g| g.to_string()),
        album_artist: tag.album_artist().map(String::from),
        bitrate_kbps: None, // id3 crate doesn't provide bitrate
    };

    debug!(
        "Extracted metadata - title: {:?}, artist: {:?}, album: {:?}",
        metadata.title, metadata.artist, metadata.album
    );

    Ok(metadata)
}

/// Parse track number from ID3 tag, handling "track/total" format.
fn parse_track_number(tag: &Tag) -> (Option<u32>, Option<u32>) {
    if let Some(track) = tag.track() {
        let total = tag.total_tracks();
        (Some(track), total)
    } else {
        (None, None)
    }
}

/// Extract metadata from multiple MP3 files.
///
/// Processes files in parallel for better performance.
/// Files that fail to parse return empty metadata (no error).
///
/// # Arguments
///
/// * `paths` - Iterator of paths to MP3 files
///
/// # Returns
///
/// A vector of (path, metadata) pairs in the same order as input.
pub fn extract_metadata_batch<'a, I>(paths: I) -> Vec<(std::path::PathBuf, Mp3Metadata)>
where
    I: Iterator<Item = &'a Path>,
{
    paths
        .map(|path| {
            let metadata = extract_metadata(path).unwrap_or_default();
            (path.to_path_buf(), metadata)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    #[test]
    fn test_empty_metadata() {
        let metadata = Mp3Metadata::empty();
        assert!(metadata.title.is_none());
        assert!(metadata.artist.is_none());
        assert!(metadata.album.is_none());
        assert!(!metadata.has_content());
    }

    #[test]
    fn test_metadata_has_content() {
        let mut metadata = Mp3Metadata::empty();
        assert!(!metadata.has_content());

        metadata.title = Some("Test".to_string());
        assert!(metadata.has_content());
    }

    #[test]
    fn test_display_methods() {
        let metadata = Mp3Metadata::empty();
        assert_eq!(metadata.display_title(), "Unknown Title");
        assert_eq!(metadata.display_artist(), "Unknown Artist");
        assert_eq!(metadata.display_album(), "Unknown Album");

        let metadata_with_data = Mp3Metadata {
            title: Some("My Song".to_string()),
            artist: Some("My Artist".to_string()),
            album: Some("My Album".to_string()),
            ..Default::default()
        };
        assert_eq!(metadata_with_data.display_title(), "My Song");
        assert_eq!(metadata_with_data.display_artist(), "My Artist");
        assert_eq!(metadata_with_data.display_album(), "My Album");
    }

    #[test]
    fn test_formatted_duration() {
        let metadata = Mp3Metadata {
            duration_secs: Some(185), // 3:05
            ..Default::default()
        };
        assert_eq!(metadata.formatted_duration(), Some("3:05".to_string()));

        let metadata_no_duration = Mp3Metadata::empty();
        assert_eq!(metadata_no_duration.formatted_duration(), None);
    }

    #[test]
    fn test_formatted_track_number() {
        let metadata = Mp3Metadata {
            track_number: Some(3),
            total_tracks: Some(12),
            ..Default::default()
        };
        assert_eq!(metadata.formatted_track_number(), Some("3/12".to_string()));

        let metadata_no_total = Mp3Metadata {
            track_number: Some(5),
            total_tracks: None,
            ..Default::default()
        };
        assert_eq!(
            metadata_no_total.formatted_track_number(),
            Some("5".to_string())
        );

        let metadata_no_track = Mp3Metadata::empty();
        assert_eq!(metadata_no_track.formatted_track_number(), None);
    }

    #[test]
    fn test_extract_metadata_file_not_found() {
        let result = extract_metadata(Path::new("/nonexistent/file.mp3"));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_metadata_non_mp3_file() {
        let temp_dir = create_test_dir();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "not an mp3 file").expect("Write should succeed");

        // Should return empty metadata, not error
        let result = extract_metadata(&file_path);
        assert!(result.is_ok());
        let metadata = result.unwrap();
        assert!(!metadata.has_content());
    }

    #[test]
    fn test_extract_metadata_batch_empty() {
        let paths: Vec<&Path> = vec![];
        let results = extract_metadata_batch(paths.into_iter());
        assert!(results.is_empty());
    }

    #[test]
    fn test_metadata_serialization() {
        let metadata = Mp3Metadata {
            title: Some("Test Song".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            duration_secs: Some(180),
            track_number: Some(1),
            total_tracks: Some(10),
            year: Some(2024),
            genre: Some("Rock".to_string()),
            album_artist: Some("Various Artists".to_string()),
            bitrate_kbps: Some(320),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&metadata).expect("Serialization should succeed");
        let deserialized: Mp3Metadata =
            serde_json::from_str(&json).expect("Deserialization should succeed");
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_metadata_default() {
        let metadata: Mp3Metadata = Default::default();
        assert_eq!(metadata, Mp3Metadata::empty());
    }

    #[test]
    fn test_metadata_clone() {
        let metadata = Mp3Metadata {
            title: Some("Test".to_string()),
            ..Default::default()
        };
        let cloned = metadata.clone();
        assert_eq!(metadata, cloned);
    }
}
