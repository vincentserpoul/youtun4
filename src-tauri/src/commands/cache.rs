//! Cache management commands.

use tauri::State;
use tracing::{debug, info};
use youtun4_core::cache::{
    CacheCleanupStats, CacheConfig, CacheManager, CacheStats, default_cache_directory,
};

use super::error::map_err;
use super::state::AppState;

/// Get cache statistics.
#[tauri::command]
pub async fn get_cache_stats(
    state: State<'_, AppState>,
) -> std::result::Result<CacheStats, String> {
    debug!("Getting cache statistics");

    let config_manager = state.config_manager.read().await;
    let cache_config = config_manager.config().cache.clone();
    drop(config_manager);

    let cache = CacheManager::new(cache_config).map_err(map_err)?;
    Ok(cache.stats())
}

/// Get the cache configuration.
#[tauri::command]
pub async fn get_cache_config(
    state: State<'_, AppState>,
) -> std::result::Result<CacheConfig, String> {
    debug!("Getting cache configuration");

    let config_manager = state.config_manager.read().await;
    Ok(config_manager.config().cache.clone())
}

/// Update the cache configuration.
#[tauri::command]
pub async fn update_cache_config(
    state: State<'_, AppState>,
    config: CacheConfig,
) -> std::result::Result<(), String> {
    info!("Updating cache configuration");

    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.cache = config;
    config_manager.update(app_config).map_err(map_err)
}

/// Clean up the cache.
#[tauri::command]
pub async fn cleanup_cache(
    state: State<'_, AppState>,
) -> std::result::Result<CacheCleanupStats, String> {
    info!("Running cache cleanup");

    let config_manager = state.config_manager.read().await;
    let cache_config = config_manager.config().cache.clone();
    drop(config_manager);

    let mut cache = CacheManager::new(cache_config).map_err(map_err)?;
    cache.cleanup().map_err(map_err)
}

/// Clear all cached data.
#[tauri::command]
pub async fn clear_cache(
    state: State<'_, AppState>,
) -> std::result::Result<CacheCleanupStats, String> {
    info!("Clearing all cache data");

    let config_manager = state.config_manager.read().await;
    let cache_config = config_manager.config().cache.clone();
    drop(config_manager);

    let mut cache = CacheManager::new(cache_config).map_err(map_err)?;
    cache.clear().map_err(map_err)
}

/// Clean up temporary files.
#[tauri::command]
pub async fn cleanup_cache_temp(
    state: State<'_, AppState>,
) -> std::result::Result<CacheCleanupStats, String> {
    info!("Cleaning up cache temp files");

    let config_manager = state.config_manager.read().await;
    let cache_config = config_manager.config().cache.clone();
    drop(config_manager);

    let mut cache = CacheManager::new(cache_config).map_err(map_err)?;
    cache.cleanup_temp().map_err(map_err)
}

/// Get the default cache directory path.
#[tauri::command]
pub fn get_default_cache_directory() -> String {
    default_cache_directory().display().to_string()
}

/// Check if caching is enabled.
#[tauri::command]
pub async fn is_cache_enabled(state: State<'_, AppState>) -> std::result::Result<bool, String> {
    let config_manager = state.config_manager.read().await;
    Ok(config_manager.config().cache.enabled)
}

/// Enable or disable caching.
#[tauri::command]
pub async fn set_cache_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> std::result::Result<(), String> {
    info!("Setting cache enabled: {}", enabled);

    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.cache.enabled = enabled;
    config_manager.update(app_config).map_err(map_err)
}

/// Set the maximum cache size in bytes.
#[tauri::command]
pub async fn set_cache_max_size(
    state: State<'_, AppState>,
    max_size_bytes: u64,
) -> std::result::Result<(), String> {
    info!("Setting cache max size: {} bytes", max_size_bytes);

    let mut config_manager = state.config_manager.write().await;
    let mut app_config = config_manager.config().clone();
    app_config.cache.max_size_bytes = max_size_bytes;
    config_manager.update(app_config).map_err(map_err)
}
