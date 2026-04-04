//! `Youtun4` UI - Leptos-based user interface.
//!
//! This crate provides the frontend components for the `Youtun4` application.

// Component files tend to be large by nature - they contain view logic
#![allow(
    clippy::too_many_lines,
    reason = "UI component files are naturally large"
)]
// expect_used and unwrap_used are restricted to documented cases
#![allow(
    clippy::expect_used,
    reason = "expect_used is restricted to documented cases"
)]
// Option<Option<T>> is sometimes cleaner for nullable fields
#![allow(
    clippy::option_option,
    reason = "Option<Option<T>> is cleaner for nullable fields"
)]
// Cast wrapping is acceptable for display purposes
#![allow(
    clippy::cast_possible_wrap,
    reason = "cast wrapping is acceptable for display purposes"
)]
// Pass by value suggestions for small types like bool - not always clearer
#![allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "pass by value not always clearer for small types"
)]

pub mod app;
pub mod components;
pub mod tauri_api;
pub mod theme;
pub mod types;
pub mod utils;

pub use app::App;
pub use types::{DeviceInfo, PlaylistMetadata, TrackInfo};
