//! `Youtun4` - Desktop/Mobile app for managing MP4 playlists from `YouTube`.
//!
//! This is the main entry point for the Tauri application.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
// Tauri main function is inherently long
#![allow(clippy::too_many_lines)]

mod commands;
pub mod logging;
pub mod runtime;

use commands::AppState;
use tracing::{error, info};

fn main() {
    // Initialize structured logging with automatic configuration
    // (development in debug builds, production in release builds)
    let _logging_guard = match logging::init_auto() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("FATAL: Failed to initialize logging system: {e}");
            std::process::exit(1);
        }
    };

    info!("Starting Youtun4 application");
    info!(
        log_directory = %logging::default_log_directory().display(),
        "Logging initialized"
    );

    // Create app state (loads config automatically)
    let app_state = match AppState::new() {
        Ok(state) => state,
        Err(e) => {
            error!("Failed to create application state: {e}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Device API commands
            commands::list_devices,
            commands::get_device_info,
            commands::check_device_available,
            commands::verify_device_space,
            commands::check_sync_capacity,
            commands::start_device_watcher,
            commands::stop_device_watcher,
            commands::is_device_watcher_running,
            // Device mount/unmount commands
            commands::get_mount_status,
            commands::mount_device,
            commands::unmount_device,
            commands::eject_device,
            commands::is_mount_point_accessible,
            commands::get_mount_handler_platform,
            // Device cleanup commands
            commands::preview_device_cleanup,
            commands::cleanup_device,
            commands::cleanup_device_audio_only,
            commands::cleanup_device_verified,
            // Playlist commands
            commands::list_playlists,
            commands::create_playlist,
            commands::delete_playlist,
            commands::sync_playlist,
            commands::get_playlist_tracks,
            commands::get_playlist_tracks_fast,
            commands::get_playlist_details,
            commands::validate_playlist_folder,
            commands::get_playlist_statistics,
            commands::repair_playlist_folder,
            commands::import_playlist_folder,
            commands::rename_playlist,
            commands::playlist_exists,
            commands::ensure_playlist_structure,
            // Playlist metadata management commands
            commands::get_playlist_saved_metadata,
            commands::update_playlist_metadata,
            commands::refresh_playlist_stats,
            // MP3 metadata commands
            commands::extract_track_metadata,
            // File transfer commands
            commands::sync_playlist_with_progress,
            commands::get_default_transfer_options,
            commands::get_fast_transfer_options,
            commands::get_reliable_transfer_options,
            commands::transfer_files_to_device,
            commands::compute_file_checksum,
            commands::verify_file_integrity,
            // Integrity verification commands
            commands::create_checksum_manifest,
            commands::load_checksum_manifest,
            commands::has_checksum_manifest,
            commands::verify_directory_integrity,
            commands::verify_file_checksum,
            commands::update_manifest_file,
            commands::remove_from_manifest,
            commands::get_default_verification_options,
            commands::get_strict_verification_options,
            commands::get_quick_verification_options,
            // Task management commands
            commands::get_task_status,
            commands::get_running_tasks,
            commands::cancel_task,
            // Configuration commands
            commands::get_config,
            commands::update_config,
            commands::get_storage_directory,
            commands::set_storage_directory,
            commands::get_default_storage_directory,
            // Sync API commands
            commands::start_sync,
            commands::cancel_sync,
            commands::get_sync_status,
            commands::list_active_syncs,
            // Sync Orchestrator commands
            commands::start_orchestrated_sync,
            commands::sync_playlists_to_device,
            commands::get_default_sync_options,
            commands::get_fast_sync_options,
            commands::get_reliable_sync_options,
            // YouTube URL validation commands
            commands::validate_youtube_playlist_url,
            commands::is_valid_youtube_playlist_url,
            commands::extract_youtube_playlist_id,
            // YouTube download commands
            commands::check_yt_dlp_available,
            commands::fetch_youtube_playlist_info,
            commands::download_youtube_playlist,
            commands::download_youtube_to_playlist,
            // Cache management commands
            commands::get_cache_stats,
            commands::get_cache_config,
            commands::update_cache_config,
            commands::cleanup_cache,
            commands::clear_cache,
            commands::cleanup_cache_temp,
            commands::get_default_cache_directory,
            commands::is_cache_enabled,
            commands::set_cache_enabled,
            commands::set_cache_max_size,
            // Download queue commands
            commands::queue_add_download,
            commands::queue_add_to_playlist,
            commands::queue_add_batch,
            commands::queue_remove_item,
            commands::queue_cancel_item,
            commands::queue_set_priority,
            commands::queue_move_to_front,
            commands::queue_retry_item,
            commands::queue_get_item,
            commands::queue_get_all_items,
            commands::queue_get_pending_items,
            commands::queue_get_downloading_items,
            commands::queue_get_stats,
            commands::queue_pause,
            commands::queue_resume,
            commands::queue_is_paused,
            commands::queue_clear_finished,
            commands::queue_clear_all,
            commands::queue_get_config,
            commands::queue_set_config,
            commands::queue_set_max_concurrent,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            error!("Tauri application error: {e}");
            std::process::exit(1);
        });
}
