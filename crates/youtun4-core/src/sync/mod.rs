//! Sync orchestrator for coordinating device cleanup and file transfer.
//!
//! This module provides the main synchronization logic that coordinates:
//! - Device verification (checking connection and available space)
//! - Device cleanup (deleting old content)
//! - File transfer (copying selected playlists to the device)
//!
//! The orchestrator handles the complete workflow with progress tracking,
//! cancellation support, and error recovery.
//!
//! # Example
//!
//! ```rust,ignore
//! use youtun4_core::sync::{SyncOrchestrator, SyncOptions, SyncRequest};
//! use std::path::PathBuf;
//!
//! let orchestrator = SyncOrchestrator::new();
//! let request = SyncRequest {
//!     playlists: vec!["My Playlist".to_string()],
//!     device_mount_point: PathBuf::from("/Volumes/MP3Player"),
//! };
//!
//! let result = orchestrator.sync(
//!     &playlist_manager,
//!     &device_manager,
//!     request,
//!     &SyncOptions::default(),
//!     Some(|progress| println!("Progress: {:?}", progress)),
//! )?;
//!
//! println!("Synced {} files", result.files_transferred);
//! ```

mod orchestrator;
mod types;

pub use orchestrator::*;
pub use types::*;
