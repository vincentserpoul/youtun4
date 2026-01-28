//! MP3YouTube Core Library
//!
//! This crate provides the core functionality for the MP3YouTube application:
//! - Device detection for USB-mounted MP3 players
//! - Playlist management (create, delete, sync)
//! - YouTube audio downloading

pub mod device;
pub mod error;
pub mod playlist;
pub mod youtube;

pub use device::DeviceManager;
pub use error::{Error, Result};
pub use playlist::PlaylistManager;
