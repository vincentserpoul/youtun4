//! MP3YouTube UI - Leptos-based user interface.
//!
//! This crate provides the frontend components for the MP3YouTube application.

pub mod app;
pub mod components;
pub mod tauri_api;
pub mod theme;
pub mod types;

pub use app::App;
pub use types::{DeviceInfo, PlaylistMetadata, TrackInfo};
