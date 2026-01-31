//! Playlist management commands.

#![allow(clippy::similar_names, clippy::option_option)]

use std::path::PathBuf;

use tauri::State;
use tracing::{debug, info};
use youtun4_core::Error;
use youtun4_core::metadata::{Mp3Metadata, extract_metadata};
use youtun4_core::playlist::{
    FolderStatistics, FolderValidationResult, PlaylistMetadata, SavedPlaylistMetadata, TrackInfo,
    validate_playlist_name,
};

use super::error::map_err;
use super::state::AppState;

/// List all playlists.
#[tauri::command]
pub async fn list_playlists(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<PlaylistMetadata>, String> {
    debug!("Listing playlists");
    let manager = state.playlist_manager.read().await;
    manager.list_playlists().map_err(map_err)
}

/// Create a new playlist.
#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    source_url: Option<String>,
    thumbnail_url: Option<String>,
) -> std::result::Result<String, String> {
    info!("Creating playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    let path = manager
        .create_playlist(&name, source_url)
        .map_err(map_err)?;

    if let Some(thumb) = thumbnail_url {
        manager
            .update_playlist_metadata_full(&name, None, None, None, Some(Some(thumb)))
            .map_err(map_err)?;
    }

    Ok(path.display().to_string())
}

/// Delete a playlist.
#[tauri::command]
pub async fn delete_playlist(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    info!("Deleting playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.delete_playlist(&name).map_err(map_err)
}

/// Sync a playlist to a device.
#[tauri::command]
pub async fn sync_playlist(
    state: State<'_, AppState>,
    playlist_name: String,
    device_mount_point: String,
) -> std::result::Result<(), String> {
    info!(
        "Syncing playlist '{}' to device at '{}'",
        playlist_name, device_mount_point
    );

    let mount_point = PathBuf::from(&device_mount_point);
    let manager = state.playlist_manager.read().await;
    manager
        .sync_to_device(&playlist_name, &mount_point)
        .map_err(map_err)
}

/// Get tracks for a playlist.
#[tauri::command]
pub async fn get_playlist_tracks(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<Vec<TrackInfo>, String> {
    debug!("Getting tracks for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.list_tracks(&name).map_err(map_err)
}

/// Get detailed metadata for a specific playlist.
#[tauri::command]
pub async fn get_playlist_details(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<PlaylistMetadata, String> {
    debug!("Getting details for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    let playlist_path = manager.get_playlist_path(&name).map_err(map_err)?;
    let metadata = manager
        .get_playlist_metadata(&playlist_path)
        .map_err(map_err)?;
    info!(
        "Retrieved details for playlist '{}': {} tracks, {} bytes",
        name, metadata.track_count, metadata.total_bytes
    );
    Ok(metadata)
}

/// Validate a playlist folder structure.
#[tauri::command]
pub async fn validate_playlist_folder(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<FolderValidationResult, String> {
    debug!("Validating playlist folder: {}", name);
    let manager = state.playlist_manager.read().await;
    let result = manager.validate_folder(&name);
    info!(
        "Validation result for '{}': exists={}, has_metadata={}, metadata_valid={}, audio_files={}, issues={}",
        name,
        result.exists,
        result.has_metadata,
        result.metadata_valid,
        result.audio_file_count,
        result.issues.len()
    );
    Ok(result)
}

/// Get statistics about a playlist folder.
#[tauri::command]
pub async fn get_playlist_statistics(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<FolderStatistics, String> {
    debug!("Getting statistics for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    let stats = manager.get_folder_statistics(&name).map_err(map_err)?;
    info!(
        "Statistics for '{}': {} audio files, {} other files, {} bytes total",
        name, stats.audio_files, stats.other_files, stats.total_size_bytes
    );
    Ok(stats)
}

/// Repair a playlist folder by fixing common issues.
#[tauri::command]
pub async fn repair_playlist_folder(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<Vec<String>, String> {
    info!("Repairing playlist folder: {}", name);
    let manager = state.playlist_manager.read().await;
    let repairs = manager.repair_folder(&name).map_err(map_err)?;
    if repairs.is_empty() {
        info!("No repairs needed for playlist '{}'", name);
    } else {
        info!("Repaired playlist '{}': {:?}", name, repairs);
    }
    Ok(repairs)
}

/// Extract MP3 metadata (ID3 tags) from a single file.
#[tauri::command]
pub async fn extract_track_metadata(path: String) -> std::result::Result<Mp3Metadata, String> {
    debug!("Extracting metadata from: {}", path);
    let path_buf = PathBuf::from(&path);
    extract_metadata(&path_buf).map_err(map_err)
}

/// Get tracks for a playlist without metadata extraction.
#[tauri::command]
pub async fn get_playlist_tracks_fast(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<Vec<TrackInfo>, String> {
    debug!("Getting tracks (fast) for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager
        .list_tracks_with_options(&name, false)
        .map_err(map_err)
}

/// Import an existing folder as a playlist.
#[tauri::command]
pub async fn import_playlist_folder(
    state: State<'_, AppState>,
    folder_name: String,
    source_url: Option<String>,
) -> std::result::Result<String, String> {
    info!("Importing folder as playlist: {}", folder_name);
    let manager = state.playlist_manager.read().await;
    let folder_path = manager.base_path().join(&folder_name);
    let name = manager
        .import_folder(&folder_path, source_url)
        .map_err(map_err)?;
    info!("Successfully imported folder '{}' as playlist", name);
    Ok(name)
}

/// Rename a playlist.
#[tauri::command]
pub async fn rename_playlist(
    state: State<'_, AppState>,
    old_name: String,
    new_name: String,
) -> std::result::Result<(), String> {
    info!("Renaming playlist '{}' to '{}'", old_name, new_name);

    validate_playlist_name(&new_name).map_err(map_err)?;

    let manager = state.playlist_manager.read().await;
    let old_path = manager.get_playlist_path(&old_name).map_err(map_err)?;
    let new_path = manager.base_path().join(&new_name);

    if new_path.exists() {
        return Err(map_err(Error::Playlist(
            youtun4_core::error::PlaylistError::AlreadyExists { name: new_name },
        )));
    }

    std::fs::rename(&old_path, &new_path).map_err(|e| {
        map_err(Error::FileSystem(
            youtun4_core::error::FileSystemError::WriteFailed {
                path: new_path.clone(),
                reason: format!("Failed to rename playlist folder: {e}"),
            },
        ))
    })?;

    info!(
        "Successfully renamed playlist '{}' to '{}'",
        old_name, new_name
    );
    Ok(())
}

/// Check if a playlist exists.
#[tauri::command]
pub async fn playlist_exists(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<bool, String> {
    debug!("Checking if playlist exists: {}", name);
    let manager = state.playlist_manager.read().await;
    let exists = manager.get_playlist_path(&name).is_ok();
    Ok(exists)
}

/// Ensure a playlist folder has proper structure.
#[tauri::command]
pub async fn ensure_playlist_structure(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    debug!("Ensuring folder structure for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.ensure_folder_structure(&name).map_err(map_err)?;
    info!("Ensured folder structure for playlist '{}'", name);
    Ok(())
}

/// Get the saved metadata for a playlist.
#[tauri::command]
pub async fn get_playlist_saved_metadata(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<SavedPlaylistMetadata, String> {
    debug!("Getting saved metadata for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.get_saved_metadata(&name).map_err(map_err)
}

/// Update playlist metadata.
#[tauri::command]
pub async fn update_playlist_metadata(
    state: State<'_, AppState>,
    name: String,
    title: Option<String>,
    description: Option<String>,
    source_url: Option<Option<String>>,
    thumbnail_url: Option<Option<String>>,
) -> std::result::Result<SavedPlaylistMetadata, String> {
    info!("Updating metadata for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager
        .update_playlist_metadata_full(&name, title, description, source_url, thumbnail_url)
        .map_err(map_err)
}

/// Refresh the cached track count and total size for a playlist.
#[tauri::command]
pub async fn refresh_playlist_stats(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<SavedPlaylistMetadata, String> {
    debug!("Refreshing stats for playlist: {}", name);
    let manager = state.playlist_manager.read().await;
    manager.refresh_playlist_stats(&name).map_err(map_err)
}
