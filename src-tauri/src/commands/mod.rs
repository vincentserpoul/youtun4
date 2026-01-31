//! Tauri commands for the `Youtun4` application.
//!
//! These commands are invoked from the frontend via Tauri's IPC mechanism.
//!
//! This module is organized into submodules by feature area:
//! - `state`: Application state management
//! - `error`: Error handling utilities
//! - `device`: Device detection and management
//! - `device_watcher`: Device connection monitoring
//! - `device_mount`: Mount/unmount operations
//! - `device_cleanup`: Device cleanup operations
//! - `playlist`: Playlist management
//! - `task`: Background task management
//! - `config`: Application configuration
//! - `transfer`: File transfer operations
//! - `integrity`: File integrity verification
//! - `sync`: Playlist sync operations
//! - `sync_orchestrator`: Multi-playlist orchestrated sync
//! - `youtube`: YouTube URL validation and downloads
//! - `cache`: Cache management
//! - `queue`: Download queue management

mod cache;
mod config;
mod device;
mod device_cleanup;
mod device_mount;
mod device_watcher;
mod error;
mod integrity;
mod playlist;
mod queue;
mod state;
mod sync;
mod sync_orchestrator;
mod task;
mod transfer;
mod youtube;

// Re-export AppState for main.rs
pub use state::AppState;

// Re-export all commands
pub use cache::*;
pub use config::*;
pub use device::*;
pub use device_cleanup::*;
pub use device_mount::*;
pub use device_watcher::*;
pub use integrity::*;
pub use playlist::*;
pub use queue::*;
pub use sync::*;
pub use sync_orchestrator::*;
pub use task::*;
pub use transfer::*;
pub use youtube::*;
