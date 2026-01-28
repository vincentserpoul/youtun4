//! MP3YouTube - Desktop/Mobile app for managing MP3 playlists from YouTube.
//!
//! This is the main entry point for the Tauri application.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;

use std::path::PathBuf;

use commands::AppState;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::from_default_env()
                .add_directive("mp3youtube=debug".parse().expect("valid directive")),
        )
        .init();

    info!("Starting MP3YouTube application");

    // Determine playlists directory
    let playlists_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mp3youtube")
        .join("playlists");

    info!("Playlists directory: {}", playlists_dir.display());

    // Create app state
    let app_state = AppState::new(playlists_dir).expect("Failed to create app state");

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::list_playlists,
            commands::create_playlist,
            commands::delete_playlist,
            commands::sync_playlist,
            commands::get_playlist_tracks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
