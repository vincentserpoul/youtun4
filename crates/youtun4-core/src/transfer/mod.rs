//! File transfer engine for efficiently copying MP3 files to USB devices.
//!
//! This module provides:
//! - Chunked file transfers with progress tracking
//! - Integrity verification using checksums
//! - Resumable transfers (detecting already-transferred files)
//! - Batch transfer operations with detailed statistics
//!
//! # Example
//!
//! ```rust,ignore
//! use youtun4_core::transfer::{TransferEngine, TransferOptions, TransferProgress};
//! use std::path::PathBuf;
//!
//! let mut engine = TransferEngine::new();
//! let options = TransferOptions::default();
//!
//! let result = engine.transfer_files(
//!     &[PathBuf::from("/source/song.mp3")],
//!     &PathBuf::from("/mnt/usb"),
//!     &options,
//!     Some(|progress| println!("Progress: {:?}", progress)),
//! ).await?;
//!
//! println!("Transferred {} files", result.files_transferred);
//! ```

mod engine;
mod types;

pub use engine::*;
pub use types::*;
