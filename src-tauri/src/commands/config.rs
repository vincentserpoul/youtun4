//! Configuration commands.

use std::path::PathBuf;

use tauri::State;
use tracing::{debug, info};
use youtun4_core::AppConfig;

use super::error::map_err;
use super::state::AppState;

/// Get the current application configuration.
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> std::result::Result<AppConfig, String> {
    debug!("Getting config");
    let config_manager = state.config_manager.read().await;
    Ok(config_manager.config().clone())
}

/// Update the application configuration.
#[tauri::command]
pub async fn update_config(
    state: State<'_, AppState>,
    config: AppConfig,
) -> std::result::Result<(), String> {
    info!("Updating config");
    debug!(
        "New playlists directory: {}",
        config.playlists_directory.display()
    );

    let new_playlists_dir = config.playlists_directory.clone();

    {
        let mut config_manager = state.config_manager.write().await;
        config_manager.update(config).map_err(map_err)?;
    }

    state
        .reinitialize_playlist_manager(new_playlists_dir)
        .await
        .map_err(map_err)?;

    info!("Config updated successfully");
    Ok(())
}

/// Get the current playlists storage directory.
#[tauri::command]
pub async fn get_storage_directory(
    state: State<'_, AppState>,
) -> std::result::Result<String, String> {
    debug!("Getting storage directory");
    let config_manager = state.config_manager.read().await;
    Ok(config_manager.playlists_directory().display().to_string())
}

/// Set the playlists storage directory.
#[tauri::command]
pub async fn set_storage_directory(
    state: State<'_, AppState>,
    path: String,
) -> std::result::Result<(), String> {
    let new_path = PathBuf::from(&path);
    info!("Setting storage directory to: {}", new_path.display());

    {
        let mut config_manager = state.config_manager.write().await;
        config_manager
            .set_playlists_directory(new_path.clone())
            .map_err(map_err)?;
    }

    state
        .reinitialize_playlist_manager(new_path)
        .await
        .map_err(map_err)?;

    info!("Storage directory updated successfully");
    Ok(())
}

/// Get the default storage directory.
#[tauri::command]
pub fn get_default_storage_directory() -> String {
    youtun4_core::config::default_playlists_directory()
        .display()
        .to_string()
}
