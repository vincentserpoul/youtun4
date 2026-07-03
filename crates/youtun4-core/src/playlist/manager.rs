use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::error::{Error, FileSystemError, Result};
use crate::time::{system_time_to_secs, unix_timestamp_secs};

use super::helpers::{
    clear_directory, copy_directory_contents, is_audio_file, validate_playlist_name,
};
use super::types::{
    FolderStatistics, FolderValidationResult, PlaylistMetadata, SavedPlaylistMetadata,
    SavedTrackMetadata, TrackInfo,
};

/// Manager for local playlist operations.
#[derive(Debug)]
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
            fs::create_dir_all(&base_path).map_err(|e| {
                Error::FileSystem(FileSystemError::CreateDirFailed {
                    path: base_path.clone(),
                    reason: e.to_string(),
                })
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

        let entries = fs::read_dir(&self.base_path).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: self.base_path.clone(),
                reason: e.to_string(),
            })
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                Error::FileSystem(FileSystemError::ReadFailed {
                    path: self.base_path.clone(),
                    reason: e.to_string(),
                })
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
            .ok_or_else(|| {
                Error::Playlist(crate::error::PlaylistError::InvalidName {
                    name: String::new(),
                    reason: "Invalid path".to_string(),
                })
            })?
            .to_string();

        let metadata_file = playlist_path.join("playlist.json");
        let (source_url, created_at) = if metadata_file.exists() {
            let content = fs::read_to_string(&metadata_file).map_err(|e| {
                Error::FileSystem(FileSystemError::ReadFailed {
                    path: metadata_file.clone(),
                    reason: e.to_string(),
                })
            })?;
            let saved: SavedPlaylistMetadata =
                serde_json::from_str(&content).map_err(Error::Serialization)?;
            (saved.source_url, saved.created_at)
        } else {
            let created = fs::metadata(playlist_path)
                .and_then(|m| m.created())
                .ok()
                .map_or(0, system_time_to_secs);
            (None, created)
        };

        let modified_at = fs::metadata(playlist_path)
            .and_then(|m| m.modified())
            .ok()
            .map_or(0, system_time_to_secs);

        let (track_count, total_bytes) = self.count_tracks(playlist_path);

        Ok(PlaylistMetadata {
            name,
            source_url,
            created_at,
            modified_at,
            track_count,
            total_bytes,
        })
    }

    /// Get statistics about a playlist folder.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist.
    pub fn get_folder_statistics(&self, name: &str) -> Result<FolderStatistics> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        let mut audio_files = 0;
        let mut other_files = 0;
        let mut audio_size_bytes = 0u64;
        let mut total_size_bytes = 0u64;
        let metadata_file = playlist_path.join("playlist.json");
        let has_metadata = metadata_file.exists();

        for entry in WalkDir::new(&playlist_path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();
            if path.is_file() {
                let size = fs::metadata(path).map_or(0, |m| m.len());
                total_size_bytes += size;

                if is_audio_file(path) {
                    audio_files += 1;
                    audio_size_bytes += size;
                } else {
                    // Exclude playlist.json from "other" files count
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if file_name != "playlist.json" {
                        other_files += 1;
                    }
                }
            }
        }

        Ok(FolderStatistics {
            total_files: audio_files + other_files,
            audio_files,
            other_files,
            audio_size_bytes,
            total_size_bytes,
            has_metadata,
        })
    }

    /// Count tracks and total size in a playlist folder.
    #[allow(
        clippy::unused_self,
        reason = "method for consistency with other PlaylistManager methods"
    )]
    fn count_tracks(&self, playlist_path: &Path) -> (usize, u64) {
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

        (count, total_bytes)
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
            return Err(Error::Playlist(
                crate::error::PlaylistError::AlreadyExists {
                    name: name.to_string(),
                },
            ));
        }

        fs::create_dir_all(&playlist_path).map_err(|e| {
            Error::FileSystem(FileSystemError::CreateDirFailed {
                path: playlist_path.clone(),
                reason: e.to_string(),
            })
        })?;

        // Save metadata
        let now = unix_timestamp_secs();

        let metadata = SavedPlaylistMetadata {
            title: None,
            description: None,
            source_url,
            thumbnail_url: None,
            created_at: now,
            modified_at: now,
            track_count: 0,
            total_size_bytes: 0,
            tracks: Vec::new(),
        };

        let metadata_path = playlist_path.join("playlist.json");
        let content = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_path, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: metadata_path,
                reason: e.to_string(),
            })
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
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        fs::remove_dir_all(&playlist_path).map_err(|e| {
            Error::FileSystem(FileSystemError::DeleteFailed {
                path: playlist_path,
                reason: e.to_string(),
            })
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
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }
        Ok(playlist_path)
    }

    /// List tracks in a playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or cannot be read.
    pub fn list_tracks(&self, name: &str) -> Result<Vec<TrackInfo>> {
        self.list_tracks_with_options(name, false)
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
            return Err(Error::Device(crate::error::DeviceError::NotMounted {
                mount_point: device_mount_point.to_path_buf(),
            }));
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

    /// Validate a playlist folder structure.
    ///
    /// Returns a `FolderValidationResult` with details about the folder's state.
    #[must_use]
    pub fn validate_folder(&self, name: &str) -> FolderValidationResult {
        let playlist_path = self.base_path.join(name);
        let mut issues = Vec::new();

        let exists = playlist_path.exists() && playlist_path.is_dir();
        if !exists {
            return FolderValidationResult {
                exists: false,
                has_metadata: false,
                metadata_valid: false,
                audio_file_count: 0,
                issues: vec!["Folder does not exist".to_string()],
            };
        }

        let metadata_file = playlist_path.join("playlist.json");
        let has_metadata = metadata_file.exists();
        let mut metadata_valid = false;

        if has_metadata {
            if let Ok(content) = fs::read_to_string(&metadata_file) {
                if serde_json::from_str::<SavedPlaylistMetadata>(&content).is_ok() {
                    metadata_valid = true;
                } else {
                    issues.push("Metadata file contains invalid JSON".to_string());
                }
            } else {
                issues.push("Could not read metadata file".to_string());
            }
        } else {
            issues.push("Missing playlist.json metadata file".to_string());
        }

        let audio_file_count = WalkDir::new(&playlist_path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().is_file() && is_audio_file(e.path()))
            .count();

        if audio_file_count == 0 {
            issues.push("No audio files found".to_string());
        }

        FolderValidationResult {
            exists,
            has_metadata,
            metadata_valid,
            audio_file_count,
            issues,
        }
    }

    /// Ensure a playlist folder has the proper structure.
    ///
    /// Creates the metadata file if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or metadata cannot be created.
    pub fn ensure_folder_structure(&self, name: &str) -> Result<()> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        let metadata_file = playlist_path.join("playlist.json");
        if !metadata_file.exists() {
            let now = unix_timestamp_secs();

            let (track_count, total_size_bytes) = self.count_tracks(&playlist_path);

            let metadata = SavedPlaylistMetadata {
                title: None,
                description: None,
                source_url: None,
                thumbnail_url: None,
                created_at: now,
                modified_at: now,
                track_count,
                total_size_bytes,
                tracks: Vec::new(),
            };

            let content = serde_json::to_string_pretty(&metadata)?;
            fs::write(&metadata_file, content).map_err(|e| {
                Error::FileSystem(FileSystemError::WriteFailed {
                    path: metadata_file,
                    reason: e.to_string(),
                })
            })?;
        }

        Ok(())
    }

    /// Repair a playlist folder by fixing common issues.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist.
    pub fn repair_folder(&self, name: &str) -> Result<Vec<String>> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        let mut repairs = Vec::new();

        // Check and fix metadata
        let metadata_file = playlist_path.join("playlist.json");
        let needs_new_metadata = if metadata_file.exists() {
            match fs::read_to_string(&metadata_file) {
                Ok(content) => serde_json::from_str::<SavedPlaylistMetadata>(&content).is_err(),
                Err(_) => true,
            }
        } else {
            true
        };

        if needs_new_metadata {
            let now = unix_timestamp_secs();

            let (track_count, total_size_bytes) = self.count_tracks(&playlist_path);

            let metadata = SavedPlaylistMetadata {
                title: None,
                description: None,
                source_url: None,
                thumbnail_url: None,
                created_at: now,
                modified_at: now,
                track_count,
                total_size_bytes,
                tracks: Vec::new(),
            };

            let content = serde_json::to_string_pretty(&metadata)?;
            fs::write(&metadata_file, content).map_err(|e| {
                Error::FileSystem(FileSystemError::WriteFailed {
                    path: metadata_file,
                    reason: e.to_string(),
                })
            })?;

            repairs.push("Created or fixed metadata file".to_string());
        }

        Ok(repairs)
    }

    /// Import an existing folder as a playlist.
    ///
    /// Creates metadata for a folder that already contains audio files.
    ///
    /// # Errors
    ///
    /// Returns an error if the folder doesn't exist or metadata cannot be created.
    pub fn import_folder(&self, folder_path: &Path, source_url: Option<String>) -> Result<String> {
        if !folder_path.exists() {
            return Err(Error::FileSystem(FileSystemError::ReadFailed {
                path: folder_path.to_path_buf(),
                reason: "Folder does not exist".to_string(),
            }));
        }

        let name = folder_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                Error::Playlist(crate::error::PlaylistError::InvalidName {
                    name: String::new(),
                    reason: "Invalid folder name".to_string(),
                })
            })?
            .to_string();

        let now = unix_timestamp_secs();

        let (track_count, total_size_bytes) = self.count_tracks(folder_path);

        let metadata = SavedPlaylistMetadata {
            title: None,
            description: None,
            source_url,
            thumbnail_url: None,
            created_at: now,
            modified_at: now,
            track_count,
            total_size_bytes,
            tracks: Vec::new(),
        };

        let metadata_file = folder_path.join("playlist.json");
        let content = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_file, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: metadata_file,
                reason: e.to_string(),
            })
        })?;

        info!("Imported folder '{}' as playlist", name);
        Ok(name)
    }

    /// List tracks with options.
    ///
    /// # Arguments
    ///
    /// * `name` - The playlist name
    /// * `include_metadata` - Whether to include ID3 metadata
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist.
    pub fn list_tracks_with_options(
        &self,
        name: &str,
        include_metadata: bool,
    ) -> Result<Vec<TrackInfo>> {
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
                let size_bytes = fs::metadata(path).map_or(0, |m| m.len());

                let metadata = if include_metadata {
                    crate::metadata::extract_metadata(path).ok()
                } else {
                    None
                };

                tracks.push(TrackInfo {
                    file_name,
                    path: path.to_path_buf(),
                    size_bytes,
                    metadata,
                });
            }
        }

        // Sort by filename
        tracks.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(tracks)
    }

    /// Get the saved metadata for a playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or metadata cannot be read.
    pub fn get_saved_metadata(&self, name: &str) -> Result<SavedPlaylistMetadata> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        let metadata_file = playlist_path.join("playlist.json");
        if metadata_file.exists() {
            let content = fs::read_to_string(&metadata_file).map_err(|e| {
                Error::FileSystem(FileSystemError::ReadFailed {
                    path: metadata_file.clone(),
                    reason: e.to_string(),
                })
            })?;
            serde_json::from_str(&content).map_err(Error::Serialization)
        } else {
            // Return default metadata if file doesn't exist
            let now = unix_timestamp_secs();
            let (track_count, total_size_bytes) = self.count_tracks(&playlist_path);

            Ok(SavedPlaylistMetadata {
                title: None,
                description: None,
                source_url: None,
                thumbnail_url: None,
                created_at: now,
                modified_at: now,
                track_count,
                total_size_bytes,
                tracks: Vec::new(),
            })
        }
    }

    /// Update playlist metadata.
    ///
    /// Pass `None` for fields that should not be changed.
    /// Pass `Some(None)` to clear a field.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or metadata cannot be updated.
    pub fn update_playlist_metadata_full(
        &self,
        name: &str,
        title: Option<String>,
        description: Option<String>,
        source_url: Option<Option<String>>,
        thumbnail_url: Option<Option<String>>,
    ) -> Result<SavedPlaylistMetadata> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        let mut metadata = self.get_saved_metadata(name)?;

        // Update fields if provided
        if let Some(t) = title {
            metadata.title = if t.is_empty() { None } else { Some(t) };
        }
        if let Some(d) = description {
            metadata.description = if d.is_empty() { None } else { Some(d) };
        }
        if let Some(url) = source_url {
            metadata.source_url = url;
        }
        if let Some(url) = thumbnail_url {
            metadata.thumbnail_url = url;
        }

        // Update modified time
        metadata.modified_at = unix_timestamp_secs();

        // Save
        let metadata_file = playlist_path.join("playlist.json");
        let content = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_file, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: metadata_file,
                reason: e.to_string(),
            })
        })?;

        Ok(metadata)
    }

    /// Refresh the cached track count and total size for a playlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or metadata cannot be updated.
    pub fn refresh_playlist_stats(&self, name: &str) -> Result<SavedPlaylistMetadata> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        let mut metadata = self.get_saved_metadata(name)?;

        // Recount tracks and size
        let (track_count, total_size_bytes) = self.count_tracks(&playlist_path);
        metadata.track_count = track_count;
        metadata.total_size_bytes = total_size_bytes;
        metadata.modified_at = unix_timestamp_secs();

        // Save
        let metadata_file = playlist_path.join("playlist.json");
        let content = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_file, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: metadata_file,
                reason: e.to_string(),
            })
        })?;

        Ok(metadata)
    }

    /// Add a track to the playlist metadata.
    ///
    /// This updates the playlist.json with the new track's metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or metadata cannot be updated.
    pub fn add_track_metadata(
        &self,
        name: &str,
        track: SavedTrackMetadata,
    ) -> Result<SavedPlaylistMetadata> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        let mut metadata = self.get_saved_metadata(name)?;

        // Check if track already exists (by file_name)
        if let Some(existing) = metadata
            .tracks
            .iter_mut()
            .find(|t| t.file_name == track.file_name)
        {
            // Update existing track
            *existing = track;
        } else {
            // Add new track
            metadata.tracks.push(track);
        }

        // Update counts
        let (track_count, total_size_bytes) = self.count_tracks(&playlist_path);
        metadata.track_count = track_count;
        metadata.total_size_bytes = total_size_bytes;
        metadata.modified_at = unix_timestamp_secs();

        // Save
        let metadata_file = playlist_path.join("playlist.json");
        let content = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_file, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: metadata_file,
                reason: e.to_string(),
            })
        })?;

        Ok(metadata)
    }

    /// Add multiple tracks to the playlist metadata at once.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist doesn't exist or metadata cannot be updated.
    pub fn add_tracks_metadata(
        &self,
        name: &str,
        tracks: Vec<SavedTrackMetadata>,
    ) -> Result<SavedPlaylistMetadata> {
        let playlist_path = self.base_path.join(name);
        if !playlist_path.exists() {
            return Err(Error::Playlist(crate::error::PlaylistError::NotFound {
                name: name.to_string(),
            }));
        }

        let mut metadata = self.get_saved_metadata(name)?;

        for track in tracks {
            // Check if track already exists (by file_name)
            if let Some(existing) = metadata
                .tracks
                .iter_mut()
                .find(|t| t.file_name == track.file_name)
            {
                // Update existing track
                *existing = track;
            } else {
                // Add new track
                metadata.tracks.push(track);
            }
        }

        // Update counts
        let (track_count, total_size_bytes) = self.count_tracks(&playlist_path);
        metadata.track_count = track_count;
        metadata.total_size_bytes = total_size_bytes;
        metadata.modified_at = unix_timestamp_secs();

        // Save
        let metadata_file = playlist_path.join("playlist.json");
        let content = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_file, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: metadata_file,
                reason: e.to_string(),
            })
        })?;

        Ok(metadata)
    }

    /// Sync a playlist to a device with progress reporting.
    ///
    /// Uses the transfer engine to copy files with progress tracking.
    ///
    /// # Errors
    ///
    /// Returns an error if the sync fails.
    pub fn sync_to_device_with_progress<F>(
        &self,
        playlist_name: &str,
        device_mount_point: &Path,
        options: &crate::transfer::TransferOptions,
        progress_callback: Option<F>,
    ) -> Result<crate::transfer::TransferResult>
    where
        F: FnMut(&crate::transfer::TransferProgress),
    {
        use crate::transfer::TransferEngine;

        let playlist_path = self.get_playlist_path(playlist_name)?;

        if !device_mount_point.exists() {
            return Err(Error::Device(crate::error::DeviceError::NotMounted {
                mount_point: device_mount_point.to_path_buf(),
            }));
        }

        info!(
            "Starting sync of '{}' to {} with progress tracking",
            playlist_name,
            device_mount_point.display()
        );

        // Clear device contents first
        clear_directory(device_mount_point)?;

        // Collect source files
        let source_files: Vec<PathBuf> = WalkDir::new(&playlist_path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                let path = e.path();
                path.is_file()
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_none_or(|n| n != "playlist.json")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        // Use transfer engine
        let mut engine = TransferEngine::new();
        engine.transfer_files(
            &source_files,
            device_mount_point,
            options,
            progress_callback,
        )
    }

    /// Sync a playlist to a device with cancellation support.
    ///
    /// Uses the transfer engine to copy files with cancellation support.
    ///
    /// # Errors
    ///
    /// Returns an error if the sync fails.
    pub fn sync_to_device_cancellable<F>(
        &self,
        playlist_name: &str,
        device_mount_point: &Path,
        options: &crate::transfer::TransferOptions,
        cancel_token: std::sync::Arc<std::sync::atomic::AtomicBool>,
        progress_callback: Option<F>,
    ) -> Result<crate::transfer::TransferResult>
    where
        F: FnMut(&crate::transfer::TransferProgress),
    {
        use crate::transfer::TransferEngine;

        let playlist_path = self.get_playlist_path(playlist_name)?;

        if !device_mount_point.exists() {
            return Err(Error::Device(crate::error::DeviceError::NotMounted {
                mount_point: device_mount_point.to_path_buf(),
            }));
        }

        info!(
            "Starting cancellable sync of '{}' to {}",
            playlist_name,
            device_mount_point.display()
        );

        // Clear device contents first
        clear_directory(device_mount_point)?;

        // Collect source files
        let source_files: Vec<PathBuf> = WalkDir::new(&playlist_path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                let path = e.path();
                path.is_file()
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_none_or(|n| n != "playlist.json")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        // Use transfer engine with cancellation
        let mut engine = TransferEngine::with_cancellation(cancel_token);
        engine.transfer_files(
            &source_files,
            device_mount_point,
            options,
            progress_callback,
        )
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
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
        assert_eq!(
            metadata.expect("Should have metadata").source_url,
            Some(url.to_string())
        );
    }

    #[test]
    fn test_create_duplicate_playlist() {
        let (manager, _temp) = setup_test_manager();

        manager
            .create_playlist("Duplicate", None)
            .expect("First creation should succeed");
        let result = manager.create_playlist("Duplicate", None);

        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Playlist(
                crate::error::PlaylistError::AlreadyExists { .. }
            ))
        ));
    }

    #[test]
    fn test_delete_playlist() {
        let (manager, _temp) = setup_test_manager();

        let path = manager
            .create_playlist("ToDelete", None)
            .expect("Creation should succeed");
        assert!(path.exists());

        let result = manager.delete_playlist("ToDelete");
        result.unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_delete_nonexistent_playlist() {
        let (manager, _temp) = setup_test_manager();

        let result = manager.delete_playlist("NonExistent");
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Playlist(
                crate::error::PlaylistError::NotFound { .. }
            ))
        ));
    }

    #[test]
    fn test_list_playlists() {
        let (manager, _temp) = setup_test_manager();

        manager
            .create_playlist("Alpha", None)
            .expect("Should create");
        manager
            .create_playlist("Beta", None)
            .expect("Should create");
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
    fn test_list_tracks() {
        let (manager, temp) = setup_test_manager();

        let playlist_path = manager
            .create_playlist("TrackTest", None)
            .expect("Should create");

        // Create some test MP3 files
        fs::write(playlist_path.join("song1.mp3"), "fake mp3 data").expect("Write should succeed");
        fs::write(playlist_path.join("song2.mp3"), "fake mp3 data").expect("Write should succeed");
        fs::write(playlist_path.join("readme.txt"), "not an mp3").expect("Write should succeed");

        let tracks = manager
            .list_tracks("TrackTest")
            .expect("Should list tracks");
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
        result.unwrap();

        // Verify old content is gone
        assert!(!device_dir.path().join("old_file.txt").exists());

        // Verify new content is present
        assert!(device_dir.path().join("track1.mp3").exists());
        assert!(device_dir.path().join("track2.mp3").exists());

        // Verify playlist.json is NOT copied
        assert!(!device_dir.path().join("playlist.json").exists());
    }

    // =========================================================================
    // Additional tests for better coverage
    // =========================================================================

    #[test]
    fn test_get_folder_statistics() {
        let (manager, _temp) = setup_test_manager();

        let playlist_path = manager
            .create_playlist("StatsTest", None)
            .expect("Should create");

        // Add some files
        fs::write(playlist_path.join("song1.mp3"), "mp3 data 1").expect("Write");
        fs::write(playlist_path.join("song2.mp3"), "mp3 data 2 longer").expect("Write");
        fs::write(playlist_path.join("notes.txt"), "text").expect("Write");

        let stats = manager
            .get_folder_statistics("StatsTest")
            .expect("Should get stats");

        assert_eq!(stats.audio_files, 2);
        assert_eq!(stats.other_files, 1);
        assert!(stats.has_metadata);
        assert!(stats.audio_size_bytes > 0);
        assert!(stats.total_size_bytes > stats.audio_size_bytes);
    }

    #[test]
    fn test_get_folder_statistics_nonexistent() {
        let (manager, _temp) = setup_test_manager();
        let result = manager.get_folder_statistics("NonExistent");
        result.unwrap_err();
    }

    #[test]
    fn test_validate_folder_valid() {
        let (manager, _temp) = setup_test_manager();

        let playlist_path = manager
            .create_playlist("ValidFolder", None)
            .expect("Should create");
        fs::write(playlist_path.join("track.mp3"), "mp3 data").expect("Write");

        let result = manager.validate_folder("ValidFolder");
        assert!(result.exists);
        assert!(result.has_metadata);
        assert!(result.metadata_valid);
        assert_eq!(result.audio_file_count, 1);
        assert!(result.is_valid());
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_validate_folder_nonexistent() {
        let (manager, _temp) = setup_test_manager();
        let result = manager.validate_folder("DoesNotExist");
        assert!(!result.exists);
        assert!(!result.is_valid());
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_validate_folder_no_audio() {
        let (manager, _temp) = setup_test_manager();

        let playlist_path = manager
            .create_playlist("NoAudio", None)
            .expect("Should create");
        fs::write(playlist_path.join("readme.txt"), "text only").expect("Write");

        let result = manager.validate_folder("NoAudio");
        assert!(result.exists);
        assert!(result.has_metadata);
        assert_eq!(result.audio_file_count, 0);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_validate_folder_invalid_metadata() {
        let (manager, _temp) = setup_test_manager();

        let playlist_path = manager
            .create_playlist("InvalidMeta", None)
            .expect("Should create");
        fs::write(playlist_path.join("track.mp3"), "mp3 data").expect("Write");
        // Overwrite with invalid JSON
        fs::write(playlist_path.join("playlist.json"), "not valid json {{{").expect("Write");

        let result = manager.validate_folder("InvalidMeta");
        assert!(result.exists);
        assert!(result.has_metadata);
        assert!(!result.metadata_valid);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_ensure_folder_structure_creates_metadata() {
        let (manager, temp) = setup_test_manager();

        // Create a folder without using create_playlist
        let folder_path = temp.path().join("ManualFolder");
        fs::create_dir(&folder_path).expect("Create dir");
        fs::write(folder_path.join("song.mp3"), "mp3 data").expect("Write");

        // Ensure structure
        manager
            .ensure_folder_structure("ManualFolder")
            .expect("Should succeed");

        // Check metadata was created
        assert!(folder_path.join("playlist.json").exists());
    }

    #[test]
    fn test_ensure_folder_structure_nonexistent() {
        let (manager, _temp) = setup_test_manager();
        let result = manager.ensure_folder_structure("NonExistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_repair_folder_creates_metadata() {
        let (manager, temp) = setup_test_manager();

        // Create a folder without metadata
        let folder_path = temp.path().join("NeedsRepair");
        fs::create_dir(&folder_path).expect("Create dir");
        fs::write(folder_path.join("song.mp3"), "mp3 data").expect("Write");

        let repairs = manager.repair_folder("NeedsRepair").expect("Should repair");
        assert!(!repairs.is_empty());
        assert!(folder_path.join("playlist.json").exists());
    }

    #[test]
    fn test_repair_folder_fixes_invalid_metadata() {
        let (manager, _temp) = setup_test_manager();

        let playlist_path = manager
            .create_playlist("CorruptMeta", None)
            .expect("Should create");
        fs::write(playlist_path.join("track.mp3"), "mp3 data").expect("Write");
        fs::write(playlist_path.join("playlist.json"), "invalid json").expect("Write");

        let repairs = manager.repair_folder("CorruptMeta").expect("Should repair");
        assert!(!repairs.is_empty());

        // Verify metadata is now valid
        let result = manager.validate_folder("CorruptMeta");
        assert!(result.metadata_valid);
    }

    #[test]
    fn test_repair_folder_nonexistent() {
        let (manager, _temp) = setup_test_manager();
        let result = manager.repair_folder("NonExistent");
        result.unwrap_err();
    }

    #[test]
    fn test_import_folder() {
        let (manager, temp) = setup_test_manager();

        // Create a folder outside the manager
        let external_folder = temp.path().join("ExternalMusic");
        fs::create_dir(&external_folder).expect("Create dir");
        fs::write(external_folder.join("track1.mp3"), "mp3 data 1").expect("Write");
        fs::write(external_folder.join("track2.mp3"), "mp3 data 2").expect("Write");

        let url = "https://youtube.com/playlist?list=test";
        let name = manager
            .import_folder(&external_folder, Some(url.to_string()))
            .expect("Should import");

        assert_eq!(name, "ExternalMusic");
        assert!(external_folder.join("playlist.json").exists());

        // Check metadata contains source URL
        let content = fs::read_to_string(external_folder.join("playlist.json")).expect("Read");
        assert!(content.contains(url));
    }

    #[test]
    fn test_import_folder_nonexistent() {
        let (manager, temp) = setup_test_manager();
        let fake_path = temp.path().join("DoesNotExist");
        let result = manager.import_folder(&fake_path, None);
        result.unwrap_err();
    }

    #[test]
    fn test_get_playlist_path() {
        let (manager, _temp) = setup_test_manager();

        manager
            .create_playlist("PathTest", None)
            .expect("Should create");

        let path = manager.get_playlist_path("PathTest").expect("Should get");
        assert!(path.exists());
        assert!(path.ends_with("PathTest"));
    }

    #[test]
    fn test_get_playlist_path_nonexistent() {
        let (manager, _temp) = setup_test_manager();
        let result = manager.get_playlist_path("NonExistent");
        result.unwrap_err();
    }

    #[test]
    fn test_get_playlist_metadata() {
        let (manager, _temp) = setup_test_manager();

        let url = "https://youtube.com/test";
        let playlist_path = manager
            .create_playlist("MetadataTest", Some(url.to_string()))
            .expect("Should create");
        fs::write(playlist_path.join("song.mp3"), "mp3 data").expect("Write");

        let metadata = manager
            .get_playlist_metadata(&playlist_path)
            .expect("Should get");

        assert_eq!(metadata.name, "MetadataTest");
        assert_eq!(metadata.source_url, Some(url.to_string()));
        assert_eq!(metadata.track_count, 1);
        assert!(metadata.total_bytes > 0);
        assert!(metadata.created_at > 0);
    }

    #[test]
    fn test_list_tracks_empty_playlist() {
        let (manager, _temp) = setup_test_manager();
        manager
            .create_playlist("EmptyPlaylist", None)
            .expect("Should create");

        let tracks = manager.list_tracks("EmptyPlaylist").expect("Should list");
        assert!(tracks.is_empty());
    }

    #[test]
    fn test_list_tracks_nonexistent() {
        let (manager, _temp) = setup_test_manager();
        let result = manager.list_tracks("NonExistent");
        result.unwrap_err();
    }

    #[test]
    fn test_sync_to_nonexistent_device() {
        let (manager, _temp) = setup_test_manager();
        manager
            .create_playlist("SyncFail", None)
            .expect("Should create");

        let fake_device = PathBuf::from("/nonexistent/device/path");
        let result = manager.sync_to_device("SyncFail", &fake_device);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_nonexistent_playlist() {
        let (manager, _temp) = setup_test_manager();
        let device_dir = TempDir::new().expect("Create device dir");

        let result = manager.sync_to_device("NonExistent", device_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_base_path() {
        let (manager, temp) = setup_test_manager();
        assert_eq!(manager.base_path(), temp.path());
    }

    #[test]
    fn test_list_playlists_empty() {
        let (manager, _temp) = setup_test_manager();
        let playlists = manager.list_playlists().expect("Should list");
        assert!(playlists.is_empty());
    }

    #[test]
    fn test_create_playlist_creates_directory() {
        let (manager, _temp) = setup_test_manager();
        let path = manager
            .create_playlist("NewPlaylist", None)
            .expect("Should create");
        assert!(path.is_dir());
    }
}
