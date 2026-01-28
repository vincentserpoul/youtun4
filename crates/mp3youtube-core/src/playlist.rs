//! Playlist management module.
//!
//! Handles creating, deleting, and syncing playlists.
//! Each playlist is represented as a folder containing MP3 files.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::error::{Error, Result};

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
    pub path: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Manager for local playlist operations.
pub struct PlaylistManager {
    /// Base directory where playlists are stored.
    base_path: PathBuf,
}

impl PlaylistManager {
    /// Create a new playlist manager.
    ///
    /// # Errors
    ///
    /// Returns an error if the base path cannot be created.
    pub fn new(base_path: PathBuf) -> Result<Self> {
        if !base_path.exists() {
            fs::create_dir_all(&base_path).map_err(|e| Error::FileSystem {
                path: base_path.clone(),
                message: format!("Failed to create base directory: {e}"),
            })?;
        }
        Ok(Self { base_path })
    }

    /// Get the base path for playlists.
    #[must_use]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// List all playlists.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    pub fn list_playlists(&self) -> Result<Vec<PlaylistMetadata>> {
        let mut playlists = Vec::new();

        let entries = fs::read_dir(&self.base_path).map_err(|e| Error::FileSystem {
            path: self.base_path.clone(),
            message: format!("Failed to read playlists directory: {e}"),
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| Error::FileSystem {
                path: self.base_path.clone(),
                message: format!("Failed to read directory entry: {e}"),
            })?;

            let path = entry.path();
            if path.is_dir() {
                match self.get_playlist_metadata(&path) {
                    Ok(metadata) => playlists.push(metadata),
                    Err(e) => {
                        warn!("Failed to read playlist at {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Sort by name
        playlists.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(playlists)
    }

    /// Get metadata for a specific playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or cannot be read.
    pub fn get_playlist_metadata(&self, playlist_path: &Path) -> Result<PlaylistMetadata> {
        let name = playlist_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::InvalidPlaylistName("Invalid path".to_string()))?
            .to_string();

        let metadata_file = playlist_path.join("playlist.json");
        let (source_url, created_at) = if metadata_file.exists() {
            let content = fs::read_to_string(&metadata_file).map_err(|e| Error::FileSystem {
                path: metadata_file.clone(),
                message: format!("Failed to read metadata file: {e}"),
            })?;
            let saved: SavedPlaylistMetadata =
                serde_json::from_str(&content).map_err(Error::Serialization)?;
            (saved.source_url, saved.created_at)
        } else {
            let created = fs::metadata(playlist_path)
                .and_then(|m| m.created())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            (None, created)
        };

        let modified_at = fs::metadata(playlist_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());

        let (track_count, total_bytes) = self.count_tracks(playlist_path)?;

        Ok(PlaylistMetadata {
            name,
            source_url,
            created_at,
            modified_at,
            track_count,
            total_bytes,
        })
    }

    /// Count tracks and total size in a playlist folder.
    fn count_tracks(&self, playlist_path: &Path) -> Result<(usize, u64)> {
        let mut count = 0;
        let mut total_bytes = 0;

        for entry in WalkDir::new(playlist_path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();
            if path.is_file() && is_audio_file(path) {
                count += 1;
                if let Ok(meta) = fs::metadata(path) {
                    total_bytes += meta.len();
                }
            }
        }

        Ok((count, total_bytes))
    }

    /// Create a new empty playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist already exists or cannot be created.
    pub fn create_playlist(&self, name: &str, source_url: Option<String>) -> Result<PathBuf> {
        validate_playlist_name(name)?;

        let playlist_path = self.base_path.join(name);
        if playlist_path.exists() {
            return Err(Error::PlaylistAlreadyExists(name.to_string()));
        }

        fs::create_dir_all(&playlist_path).map_err(|e| Error::FileSystem {
            path: playlist_path.clone(),
            message: format!("Failed to create playlist directory: {e}"),
        })?;

        // Save metadata
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        let metadata = SavedPlaylistMetadata {
            source_url,
            created_at: now,
        };

        let metadata_path = playlist_path.join("playlist.json");
        let content = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_path, content).map_err(|e| Error::FileSystem {
            path: metadata_path,
            message: format!("Failed to write metadata: {e}"),
        })?;

        info!("Created playlist: {}", name);
        Ok(playlist_path)
    }

    /// Delete a playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or cannot be deleted.
    pub fn delete_playlist(&self, name: &str) -> Result<()> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::PlaylistNotFound(name.to_string()));
        }

        fs::remove_dir_all(&playlist_path).map_err(|e| Error::FileSystem {
            path: playlist_path,
            message: format!("Failed to delete playlist: {e}"),
        })?;

        info!("Deleted playlist: {}", name);
        Ok(())
    }

    /// Get the path to a playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist.
    pub fn get_playlist_path(&self, name: &str) -> Result<PathBuf> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::PlaylistNotFound(name.to_string()));
        }
        Ok(playlist_path)
    }

    /// List tracks in a playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or cannot be read.
    pub fn list_tracks(&self, name: &str) -> Result<Vec<TrackInfo>> {
        let playlist_path = self.get_playlist_path(name)?;
        let mut tracks = Vec::new();

        for entry in WalkDir::new(&playlist_path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();
            if path.is_file() && is_audio_file(path) {
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

                tracks.push(TrackInfo {
                    file_name,
                    path: path.to_path_buf(),
                    size_bytes,
                });
            }
        }

        // Sort by filename
        tracks.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(tracks)
    }

    /// Sync a playlist to a device.
    ///
    /// This will delete all contents on the device and copy the playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the sync fails.
    pub fn sync_to_device(&self, playlist_name: &str, device_mount_point: &Path) -> Result<()> {
        let playlist_path = self.get_playlist_path(playlist_name)?;

        if !device_mount_point.exists() {
            return Err(Error::DeviceNotMounted(
                device_mount_point.display().to_string(),
            ));
        }

        info!(
            "Starting sync of '{}' to {}",
            playlist_name,
            device_mount_point.display()
        );

        // Clear device contents (except hidden files/system files)
        debug!("Clearing device contents...");
        clear_directory(device_mount_point)?;

        // Copy playlist contents
        debug!("Copying playlist contents...");
        copy_directory_contents(&playlist_path, device_mount_point)?;

        info!("Sync completed successfully");
        Ok(())
    }
}

/// Metadata saved to playlist.json.
#[derive(Debug, Serialize, Deserialize)]
struct SavedPlaylistMetadata {
    source_url: Option<String>,
    created_at: u64,
}

/// Check if a file is an audio file based on extension.
fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "mp3" | "m4a" | "wav" | "flac" | "ogg" | "aac"
            )
        })
        .unwrap_or(false)
}

/// Validate a playlist name.
fn validate_playlist_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidPlaylistName(
            "Playlist name cannot be empty".to_string(),
        ));
    }

    if name.len() > 255 {
        return Err(Error::InvalidPlaylistName(
            "Playlist name too long".to_string(),
        ));
    }

    // Check for invalid characters
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
    if name.chars().any(|c| invalid_chars.contains(&c)) {
        return Err(Error::InvalidPlaylistName(
            "Playlist name contains invalid characters".to_string(),
        ));
    }

    // Check for reserved names (Windows compatibility)
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&name.to_uppercase().as_str()) {
        return Err(Error::InvalidPlaylistName(
            "Playlist name is reserved".to_string(),
        ));
    }

    Ok(())
}

/// Clear all non-hidden contents of a directory.
fn clear_directory(path: &Path) -> Result<()> {
    let entries = fs::read_dir(path).map_err(|e| Error::FileSystem {
        path: path.to_path_buf(),
        message: format!("Failed to read directory: {e}"),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| Error::FileSystem {
            path: path.to_path_buf(),
            message: format!("Failed to read entry: {e}"),
        })?;

        let entry_path = entry.path();
        let file_name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Skip hidden files and system files
        if file_name.starts_with('.') || file_name.eq_ignore_ascii_case("System Volume Information")
        {
            continue;
        }

        if entry_path.is_dir() {
            fs::remove_dir_all(&entry_path).map_err(|e| Error::FileSystem {
                path: entry_path.clone(),
                message: format!("Failed to remove directory: {e}"),
            })?;
        } else {
            fs::remove_file(&entry_path).map_err(|e| Error::FileSystem {
                path: entry_path.clone(),
                message: format!("Failed to remove file: {e}"),
            })?;
        }
    }

    Ok(())
}

/// Copy contents of one directory to another.
fn copy_directory_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in WalkDir::new(src)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let src_path = entry.path();
        let file_name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Skip metadata file
        if file_name == "playlist.json" {
            continue;
        }

        let dst_path = dst.join(file_name);

        if src_path.is_file() {
            fs::copy(src_path, &dst_path).map_err(|e| Error::FileSystem {
                path: dst_path.clone(),
                message: format!("Failed to copy file: {e}"),
            })?;
        } else if src_path.is_dir() {
            fs::create_dir_all(&dst_path).map_err(|e| Error::FileSystem {
                path: dst_path.clone(),
                message: format!("Failed to create directory: {e}"),
            })?;
            copy_directory_contents(src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_manager() -> (PlaylistManager, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let manager =
            PlaylistManager::new(temp_dir.path().to_path_buf()).expect("Failed to create manager");
        (manager, temp_dir)
    }

    #[test]
    fn test_create_playlist() {
        let (manager, _temp) = setup_test_manager();

        let result = manager.create_playlist("My Playlist", None);
        assert!(result.is_ok());

        let path = result.expect("Should have path");
        assert!(path.exists());
        assert!(path.join("playlist.json").exists());
    }

    #[test]
    fn test_create_playlist_with_source_url() {
        let (manager, _temp) = setup_test_manager();

        let url = "https://www.youtube.com/playlist?list=PLtest";
        let result = manager.create_playlist("YouTube Playlist", Some(url.to_string()));
        assert!(result.is_ok());

        let metadata = manager.get_playlist_metadata(&result.expect("Should have path"));
        assert!(metadata.is_ok());
        assert_eq!(metadata.expect("Should have metadata").source_url, Some(url.to_string()));
    }

    #[test]
    fn test_create_duplicate_playlist() {
        let (manager, _temp) = setup_test_manager();

        manager
            .create_playlist("Duplicate", None)
            .expect("First creation should succeed");
        let result = manager.create_playlist("Duplicate", None);

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::PlaylistAlreadyExists(_))));
    }

    #[test]
    fn test_delete_playlist() {
        let (manager, _temp) = setup_test_manager();

        let path = manager
            .create_playlist("ToDelete", None)
            .expect("Creation should succeed");
        assert!(path.exists());

        let result = manager.delete_playlist("ToDelete");
        assert!(result.is_ok());
        assert!(!path.exists());
    }

    #[test]
    fn test_delete_nonexistent_playlist() {
        let (manager, _temp) = setup_test_manager();

        let result = manager.delete_playlist("NonExistent");
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::PlaylistNotFound(_))));
    }

    #[test]
    fn test_list_playlists() {
        let (manager, _temp) = setup_test_manager();

        manager.create_playlist("Alpha", None).expect("Should create");
        manager.create_playlist("Beta", None).expect("Should create");
        manager
            .create_playlist("Gamma", None)
            .expect("Should create");

        let playlists = manager.list_playlists().expect("Should list");
        assert_eq!(playlists.len(), 3);
        assert_eq!(playlists[0].name, "Alpha");
        assert_eq!(playlists[1].name, "Beta");
        assert_eq!(playlists[2].name, "Gamma");
    }

    #[test]
    fn test_validate_playlist_name_empty() {
        let result = validate_playlist_name("");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_playlist_name_invalid_chars() {
        let invalid_names = ["test/name", "test\\name", "test:name", "test*name"];
        for name in invalid_names {
            let result = validate_playlist_name(name);
            assert!(result.is_err(), "Name '{}' should be invalid", name);
        }
    }

    #[test]
    fn test_validate_playlist_name_reserved() {
        let result = validate_playlist_name("CON");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_audio_file() {
        assert!(is_audio_file(Path::new("song.mp3")));
        assert!(is_audio_file(Path::new("song.MP3")));
        assert!(is_audio_file(Path::new("song.m4a")));
        assert!(is_audio_file(Path::new("song.flac")));
        assert!(!is_audio_file(Path::new("song.txt")));
        assert!(!is_audio_file(Path::new("song")));
    }

    #[test]
    fn test_list_tracks() {
        let (manager, temp) = setup_test_manager();

        let playlist_path = manager
            .create_playlist("TrackTest", None)
            .expect("Should create");

        // Create some test MP3 files
        fs::write(playlist_path.join("song1.mp3"), "fake mp3 data").expect("Write should succeed");
        fs::write(playlist_path.join("song2.mp3"), "fake mp3 data").expect("Write should succeed");
        fs::write(playlist_path.join("readme.txt"), "not an mp3").expect("Write should succeed");

        let tracks = manager.list_tracks("TrackTest").expect("Should list tracks");
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().any(|t| t.file_name == "song1.mp3"));
        assert!(tracks.iter().any(|t| t.file_name == "song2.mp3"));

        drop(temp);
    }

    #[test]
    fn test_sync_to_device() {
        let (manager, _temp) = setup_test_manager();
        let device_dir = TempDir::new().expect("Failed to create device dir");

        // Create playlist with tracks
        let playlist_path = manager
            .create_playlist("SyncTest", None)
            .expect("Should create");
        fs::write(playlist_path.join("track1.mp3"), "mp3 data 1").expect("Write should succeed");
        fs::write(playlist_path.join("track2.mp3"), "mp3 data 2").expect("Write should succeed");

        // Add some existing content to device
        fs::write(device_dir.path().join("old_file.txt"), "old content")
            .expect("Write should succeed");

        // Sync
        let result = manager.sync_to_device("SyncTest", device_dir.path());
        assert!(result.is_ok());

        // Verify old content is gone
        assert!(!device_dir.path().join("old_file.txt").exists());

        // Verify new content is present
        assert!(device_dir.path().join("track1.mp3").exists());
        assert!(device_dir.path().join("track2.mp3").exists());

        // Verify playlist.json is NOT copied
        assert!(!device_dir.path().join("playlist.json").exists());
    }
}
