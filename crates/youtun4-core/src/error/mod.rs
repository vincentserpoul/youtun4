//! Error types for `Youtun4` core operations.
//!
//! This module provides a comprehensive error handling framework using `thiserror`
//! for defining custom error types with meaningful messages, and integrates with
//! `anyhow` for error context propagation.
//!
//! # Error Categories
//!
//! - **Device errors**: USB device detection, mounting, capacity, and access issues
//! - **Download errors**: `YouTube` downloading, network, and conversion failures
//! - **Transfer errors**: File sync, copy, and integrity verification issues
//! - **Playlist errors**: Playlist management operations
//! - **File management errors**: File system operations
//!
//! # Example
//!
//! ```rust
//! use youtun4_core::error::{Error, Result, ErrorContext};
//!
//! fn do_operation() -> Result<()> {
//!     // Operations that might fail...
//!     Ok(())
//! }
//! ```

mod cache;
mod core;
mod device;
mod download;
mod filesystem;
mod playlist;
mod transfer;

pub use cache::*;
pub use core::*;
pub use device::*;
pub use download::*;
pub use filesystem::*;
pub use playlist::*;
pub use transfer::*;

/// Result type alias using the crate's Error type.
pub type Result<T> = std::result::Result<T, Error>;
