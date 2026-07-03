//! `Youtun4` UI - Leptos-based user interface.
//!
//! This crate provides the frontend components for the `Youtun4` application.

pub mod app;
pub mod components;
pub mod tauri_api;
pub mod theme;
pub mod types;
pub mod utils;

pub use app::App;
pub use types::{DeviceInfo, PlaylistMetadata, TrackInfo};
