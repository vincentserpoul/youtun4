//! `YouTube` playlist downloading module.
//!
//! Handles downloading audio from `YouTube` playlists.
//! Uses pure Rust libraries - no external dependencies like yt-dlp required.
//!
//! # Pure Rust Implementation
//!
//! This module uses `rusty_ytdl` for `YouTube` video downloading, which is a
//! pure Rust implementation that doesn't require any external tools.
//!
//! ## Quality Settings
//!
//! Downloads are configured to:
//! - Extract the best audio stream available
//! - Save as the original format (usually m4a/webm)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use youtun4_core::youtube::{RustyYtdlDownloader, YouTubeDownloader};
//! use std::path::Path;
//!
//! let downloader = RustyYtdlDownloader::new();
//! let playlist = downloader.parse_playlist_url("https://www.youtube.com/playlist?list=PLtest").unwrap();
//! let results = downloader.download_playlist(&playlist, Path::new("/tmp/music"), None).unwrap();
//! ```

mod audio;
mod default;
mod downloader;
mod model;
mod progress;
mod rusty_ytdl;
mod url;
mod ytdlp;

pub use default::*;
pub use downloader::*;
pub use model::*;
pub use progress::*;
pub use rusty_ytdl::*;
pub use url::*;
pub use ytdlp::*;
