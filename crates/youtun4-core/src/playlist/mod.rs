//! Playlist management module.
//!
//! Handles creating, deleting, and syncing playlists.
//! Each playlist is represented as a folder containing MP3 files.

mod helpers;
mod manager;
mod types;

pub use helpers::*;
pub use manager::*;
pub use types::*;
