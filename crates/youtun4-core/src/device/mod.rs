//! Device detection and management for USB-mounted MP3 players.
//!
//! This module provides:
//! - Device detection via [`DeviceDetector`] trait and [`DeviceManager`] implementation
//! - Mount/unmount operations via [`DeviceMountHandler`] trait and platform-specific implementations
//! - Device event monitoring for real-time mount/unmount notifications

mod detection;
mod mount;
mod watcher;

pub use detection::*;
pub use mount::*;
pub use watcher::*;
