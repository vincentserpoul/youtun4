//! Download queue manager for handling multiple playlist download requests.
//!
//! This module provides a queue system for managing concurrent downloads with:
//! - Configurable concurrent download limits
//! - Priority-based ordering
//! - Queue item lifecycle management (pending, downloading, completed, failed, cancelled)
//! - Event emission for queue state changes

mod manager;
mod state;
mod types;

pub use manager::*;
pub use types::*;
