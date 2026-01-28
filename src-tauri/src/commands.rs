//! Tauri commands for the MP3YouTube application.
//!
//! These commands are invoked from the frontend via Tauri's IPC mechanism.

use std::path::PathBuf;
use std::sync::Mutex;

use mp3youtube_core::{
    device::{DeviceDetector, DeviceInfo, DeviceManager},
    playlist::{PlaylistManager, PlaylistMetadata, TrackInfo},
    Error, Result,
};
use tauri::State;
use tracing::{debug, error, info};

/// Application state managed by Tauri.
pub struct AppState {
    /// Device manager for detecting USB devices.
    device_manager: Mutex<DeviceManager>,
    /// Playlist manager for local playlist operations.
    playlist_manager: PlaylistManager,
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist manager cannot be created.
    pub fn new(playlists_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            device_manager: Mutex::new(DeviceManager::new()),
            playlist_manager: PlaylistManager::new(playlists_dir)?,
        })
    }
}

/// Convert our error type to a string for Tauri.
fn map_err(e: Error) -> String {
    error!("Command error: {}", e);
    e.to_string()
}

/// List all detected devices.
#[tauri::command]
pub fn list_devices(state: State<'_, AppState>) -> std::result::Result<Vec<DeviceInfo>, String> {
    debug!("Listing devices");

    let mut manager = state
        .device_manager
        .lock()
        .map_err(|e| format!("Lock error: {e}"))?;

    manager.refresh();
    let devices = manager.list_devices().map_err(map_err)?;
    info!("Found {} devices: {:?}", devices.len(), devices.iter().map(|d| &d.name).collect::<Vec<_>>());
    Ok(devices)
}

/// List all playlists.
#[tauri::command]
pub fn list_playlists(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<PlaylistMetadata>, String> {
    debug!("Listing playlists");
    state.playlist_manager.list_playlists().map_err(map_err)
}

/// Create a new playlist.
#[tauri::command]
pub fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    source_url: Option<String>,
) -> std::result::Result<String, String> {
    info!("Creating playlist: {}", name);
    state
        .playlist_manager
        .create_playlist(&name, source_url)
        .map(|p| p.display().to_string())
        .map_err(map_err)
}

/// Delete a playlist.
#[tauri::command]
pub fn delete_playlist(state: State<'_, AppState>, name: String) -> std::result::Result<(), String> {
    info!("Deleting playlist: {}", name);
    state.playlist_manager.delete_playlist(&name).map_err(map_err)
}

/// Sync a playlist to a device.
#[tauri::command]
pub fn sync_playlist(
    state: State<'_, AppState>,
    playlist_name: String,
    device_mount_point: String,
) -> std::result::Result<(), String> {
    info!(
        "Syncing playlist '{}' to device at '{}'",
        playlist_name, device_mount_point
    );

    let mount_point = PathBuf::from(&device_mount_point);
    state
        .playlist_manager
        .sync_to_device(&playlist_name, &mount_point)
        .map_err(map_err)
}

/// Get tracks for a playlist.
#[tauri::command]
pub fn get_playlist_tracks(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<Vec<TrackInfo>, String> {
    debug!("Getting tracks for playlist: {}", name);
    state.playlist_manager.list_tracks(&name).map_err(map_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_state() -> (AppState, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let state = AppState::new(temp_dir.path().to_path_buf()).expect("Failed to create state");
        (state, temp_dir)
    }

    #[test]
    fn test_app_state_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let result = AppState::new(temp_dir.path().to_path_buf());
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_err() {
        let error = Error::PlaylistNotFound("test".to_string());
        let result = map_err(error);
        assert!(result.contains("test"));
    }
}
