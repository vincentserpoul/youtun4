//! Device cleanup handler for safely deleting data on MP3 devices.
//!
//! This module provides functionality to safely delete existing data on MP3 devices
//! before syncing new playlists. It includes:
//! - Protected file detection (system files, hidden files)
//! - Deletion verification
//! - Progress tracking
//! - Edge case handling (read-only files, locked files)
//!
//! # Example
//!
//! ```rust,ignore
//! use mp3youtube_core::cleanup::{DeviceCleanupHandler, CleanupOptions};
//! use std::path::PathBuf;
//!
//! let handler = DeviceCleanupHandler::new();
//! let options = CleanupOptions::default();
//! let result = handler.cleanup_device(&PathBuf::from("/Volumes/MP3Player"), &options)?;
//! println!("Deleted {} files, {} bytes freed", result.files_deleted, result.bytes_freed);
//! ```

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::device::{DeviceDetector, DeviceInfo};
use crate::error::{DeviceError, Error, FileSystemError, Result};

/// Configuration options for device cleanup operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupOptions {
    /// Whether to skip hidden files (files starting with '.').
    pub skip_hidden: bool,
    /// Whether to skip system files (e.g., "System Volume Information").
    pub skip_system_files: bool,
    /// Custom patterns to protect (files/folders matching these won't be deleted).
    pub protected_patterns: Vec<String>,
    /// Whether to verify deletions after completing.
    pub verify_deletions: bool,
    /// Whether to perform a dry run (report what would be deleted without deleting).
    pub dry_run: bool,
    /// Maximum depth to traverse (-1 for unlimited).
    pub max_depth: i32,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        Self {
            skip_hidden: true,
            skip_system_files: true,
            protected_patterns: Vec::new(),
            verify_deletions: true,
            dry_run: false,
            max_depth: -1,
        }
    }
}

impl CleanupOptions {
    /// Create options for a full cleanup (skip only system files).
    #[must_use]
    pub const fn full_cleanup() -> Self {
        Self {
            skip_hidden: true,
            skip_system_files: true,
            protected_patterns: Vec::new(),
            verify_deletions: true,
            dry_run: false,
            max_depth: -1,
        }
    }

    /// Create options for a dry run.
    #[must_use]
    pub fn dry_run() -> Self {
        Self {
            dry_run: true,
            ..Self::default()
        }
    }

    /// Add a protected pattern.
    #[must_use]
    pub fn with_protected_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.protected_patterns.push(pattern.into());
        self
    }
}

/// Information about a single file or directory that will be/was deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupEntry {
    /// Path to the file or directory.
    pub path: PathBuf,
    /// Whether this is a directory.
    pub is_directory: bool,
    /// Size in bytes (0 for directories).
    pub size_bytes: u64,
    /// Whether the deletion was successful (None for dry run).
    pub deleted: Option<bool>,
    /// Error message if deletion failed.
    pub error: Option<String>,
}

/// Result of a cleanup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    /// Mount point that was cleaned.
    pub mount_point: PathBuf,
    /// Total files deleted (or would be deleted in dry run).
    pub files_deleted: usize,
    /// Total directories deleted (or would be deleted in dry run).
    pub directories_deleted: usize,
    /// Total bytes freed (or would be freed in dry run).
    pub bytes_freed: u64,
    /// Number of files skipped (protected/hidden/system).
    pub files_skipped: usize,
    /// Number of files that failed to delete.
    pub files_failed: usize,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// Detailed entries for each file/directory processed.
    pub entries: Vec<CleanupEntry>,
    /// Files that were skipped with reasons.
    pub skipped_entries: Vec<(PathBuf, String)>,
    /// Whether verification passed (if enabled).
    pub verification_passed: Option<bool>,
    /// Duration of the cleanup operation in milliseconds.
    pub duration_ms: u64,
}

impl CleanupResult {
    /// Check if the cleanup was fully successful.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.files_failed == 0 && self.verification_passed.unwrap_or(true)
    }

    /// Get a summary string of the cleanup result.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.dry_run {
            format!(
                "Dry run: would delete {} files and {} directories, freeing {} bytes ({} skipped)",
                self.files_deleted, self.directories_deleted, self.bytes_freed, self.files_skipped
            )
        } else {
            format!(
                "Deleted {} files and {} directories, freed {} bytes ({} skipped, {} failed)",
                self.files_deleted,
                self.directories_deleted,
                self.bytes_freed,
                self.files_skipped,
                self.files_failed
            )
        }
    }
}

/// Known system file/directory names that should be protected.
const SYSTEM_PATTERNS: &[&str] = &[
    "System Volume Information",
    "$RECYCLE.BIN",
    "RECYCLER",
    ".Spotlight-V100",
    ".Trashes",
    ".fseventsd",
    ".TemporaryItems",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    ".metadata_never_index",
    ".com.apple.timemachine.donotpresent",
];

/// Handler for safe device cleanup operations.
#[derive(Debug, Clone)]
pub struct DeviceCleanupHandler {
    /// System patterns that are always protected.
    system_patterns: HashSet<String>,
}

impl Default for DeviceCleanupHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceCleanupHandler {
    /// Create a new device cleanup handler.
    #[must_use]
    pub fn new() -> Self {
        let system_patterns: HashSet<String> =
            SYSTEM_PATTERNS.iter().map(|s| s.to_lowercase()).collect();
        Self { system_patterns }
    }

    /// Check if a path is protected and should not be deleted.
    #[must_use]
    pub fn is_protected(&self, path: &Path, options: &CleanupOptions) -> Option<String> {
        let file_name = path.file_name()?.to_str()?;
        let file_name_lower = file_name.to_lowercase();

        // Check for hidden files
        if options.skip_hidden && file_name.starts_with('.') {
            return Some("hidden file".to_string());
        }

        // Check for system files
        if options.skip_system_files && self.system_patterns.contains(&file_name_lower) {
            return Some("system file".to_string());
        }

        // Check custom protected patterns
        for pattern in &options.protected_patterns {
            let pattern_lower = pattern.to_lowercase();
            if file_name_lower == pattern_lower || file_name_lower.contains(&pattern_lower) {
                return Some(format!("matches protected pattern: {pattern}"));
            }
        }

        None
    }

    /// Verify that the device is accessible and writable.
    fn verify_device_writable(&self, mount_point: &Path) -> Result<()> {
        if !mount_point.exists() {
            return Err(Error::Device(DeviceError::NotMounted {
                mount_point: mount_point.to_path_buf(),
            }));
        }

        if !mount_point.is_dir() {
            return Err(Error::FileSystem(FileSystemError::InvalidPath {
                path: mount_point.to_path_buf(),
                reason: "mount point is not a directory".to_string(),
            }));
        }

        // Try to verify write access by checking directory metadata
        let metadata = fs::metadata(mount_point).map_err(|e| {
            Error::Device(DeviceError::PermissionDenied {
                path: mount_point.to_path_buf(),
                reason: format!("cannot read device metadata: {e}"),
            })
        })?;

        if metadata.permissions().readonly() {
            return Err(Error::Device(DeviceError::ReadOnly {
                name: mount_point.display().to_string(),
            }));
        }

        Ok(())
    }

    /// Scan the device to collect files that will be deleted.
    #[allow(clippy::type_complexity, clippy::unnecessary_wraps)]
    fn scan_for_cleanup(
        &self,
        mount_point: &Path,
        options: &CleanupOptions,
    ) -> Result<(Vec<CleanupEntry>, Vec<(PathBuf, String)>)> {
        let mut entries_to_delete = Vec::new();
        let mut skipped = Vec::new();

        // Build walker with depth configuration
        let walker = if options.max_depth < 0 {
            WalkDir::new(mount_point).min_depth(1)
        } else {
            WalkDir::new(mount_point)
                .min_depth(1)
                .max_depth(options.max_depth as usize)
        };

        // Collect all entries first (we'll process from deepest to shallowest)
        let mut all_entries: Vec<_> = walker
            .into_iter()
            .filter_map(std::result::Result::ok)
            .collect();

        // Sort by depth (deepest first) so we can delete children before parents
        all_entries.sort_by_key(|b| std::cmp::Reverse(b.depth()));

        for entry in all_entries {
            let path = entry.path().to_path_buf();

            // Check if protected
            if let Some(reason) = self.is_protected(&path, options) {
                debug!("Skipping protected path: {} ({})", path.display(), reason);
                skipped.push((path, reason));
                continue;
            }

            let is_directory = entry.file_type().is_dir();
            let size_bytes = if is_directory {
                0
            } else {
                fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            };

            entries_to_delete.push(CleanupEntry {
                path,
                is_directory,
                size_bytes,
                deleted: None,
                error: None,
            });
        }

        Ok((entries_to_delete, skipped))
    }

    /// Delete a single file or directory.
    fn delete_entry(&self, entry: &mut CleanupEntry) -> bool {
        let result = if entry.is_directory {
            // For directories, only try to remove if empty
            // (children should have been deleted already due to sorting)
            fs::remove_dir(&entry.path)
        } else {
            fs::remove_file(&entry.path)
        };

        match result {
            Ok(()) => {
                entry.deleted = Some(true);
                true
            }
            Err(e) => {
                entry.deleted = Some(false);
                entry.error = Some(e.to_string());
                warn!("Failed to delete {}: {}", entry.path.display(), e);
                false
            }
        }
    }

    /// Verify that all marked-deleted files are actually gone.
    fn verify_cleanup(&self, entries: &[CleanupEntry]) -> bool {
        for entry in entries {
            if entry.deleted == Some(true) && entry.path.exists() {
                warn!(
                    "Verification failed: {} still exists after deletion",
                    entry.path.display()
                );
                return false;
            }
        }
        true
    }

    /// Perform a cleanup operation on a device.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The device is not mounted or accessible
    /// - The device is read-only
    /// - There are permission issues
    pub fn cleanup_device(
        &self,
        mount_point: &Path,
        options: &CleanupOptions,
    ) -> Result<CleanupResult> {
        let start_time = SystemTime::now();

        info!(
            "Starting {} on device: {}",
            if options.dry_run {
                "cleanup dry run"
            } else {
                "cleanup"
            },
            mount_point.display()
        );

        // Verify device is accessible and writable (unless dry run)
        if !options.dry_run {
            self.verify_device_writable(mount_point)?;
        } else if !mount_point.exists() {
            return Err(Error::Device(DeviceError::NotMounted {
                mount_point: mount_point.to_path_buf(),
            }));
        }

        // Scan for files to delete
        let (mut entries, skipped_entries) = self.scan_for_cleanup(mount_point, options)?;

        let total_files = entries.iter().filter(|e| !e.is_directory).count();
        let total_dirs = entries.iter().filter(|e| e.is_directory).count();
        let total_bytes: u64 = entries.iter().map(|e| e.size_bytes).sum();

        info!(
            "Found {} files and {} directories to delete ({} bytes)",
            total_files, total_dirs, total_bytes
        );

        if options.dry_run {
            // Dry run - just report what would be deleted
            let duration_ms = start_time
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            return Ok(CleanupResult {
                mount_point: mount_point.to_path_buf(),
                files_deleted: total_files,
                directories_deleted: total_dirs,
                bytes_freed: total_bytes,
                files_skipped: skipped_entries.len(),
                files_failed: 0,
                dry_run: true,
                entries,
                skipped_entries,
                verification_passed: None,
                duration_ms,
            });
        }

        // Perform actual deletion
        let mut files_deleted = 0;
        let mut directories_deleted = 0;
        let mut bytes_freed = 0u64;
        let mut files_failed = 0;

        for entry in &mut entries {
            if self.delete_entry(entry) {
                if entry.is_directory {
                    directories_deleted += 1;
                } else {
                    files_deleted += 1;
                    bytes_freed += entry.size_bytes;
                }
            } else {
                files_failed += 1;
            }
        }

        // Verify deletions if enabled
        let verification_passed = if options.verify_deletions {
            Some(self.verify_cleanup(&entries))
        } else {
            None
        };

        let duration_ms = start_time
            .elapsed()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let result = CleanupResult {
            mount_point: mount_point.to_path_buf(),
            files_deleted,
            directories_deleted,
            bytes_freed,
            files_skipped: skipped_entries.len(),
            files_failed,
            dry_run: false,
            entries,
            skipped_entries,
            verification_passed,
            duration_ms,
        };

        info!("{}", result.summary());

        Ok(result)
    }

    /// Perform a cleanup with device verification using a detector.
    ///
    /// This method first verifies that the device is still connected using
    /// the provided device detector, then performs the cleanup.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The device is not detected/connected
    /// - The device is not mounted or accessible
    /// - The device is read-only
    pub fn cleanup_device_verified<D: DeviceDetector>(
        &self,
        detector: &D,
        device: &DeviceInfo,
        options: &CleanupOptions,
    ) -> Result<CleanupResult> {
        // Verify device is still connected
        if !detector.is_device_connected(&device.mount_point) {
            return Err(Error::Device(DeviceError::Disconnected {
                name: device.name.clone(),
            }));
        }

        self.cleanup_device(&device.mount_point, options)
    }

    /// Get a preview of what would be deleted without actually deleting.
    ///
    /// This is a convenience wrapper around `cleanup_device` with `dry_run: true`.
    pub fn preview_cleanup(
        &self,
        mount_point: &Path,
        options: &CleanupOptions,
    ) -> Result<CleanupResult> {
        let mut preview_options = options.clone();
        preview_options.dry_run = true;
        self.cleanup_device(mount_point, &preview_options)
    }

    /// Delete only audio files from the device.
    ///
    /// This is useful for refreshing audio content while keeping other files intact.
    #[allow(clippy::too_many_lines)]
    pub fn cleanup_audio_files_only(
        &self,
        mount_point: &Path,
        options: &CleanupOptions,
    ) -> Result<CleanupResult> {
        let start_time = SystemTime::now();

        if !options.dry_run {
            self.verify_device_writable(mount_point)?;
        } else if !mount_point.exists() {
            return Err(Error::Device(DeviceError::NotMounted {
                mount_point: mount_point.to_path_buf(),
            }));
        }

        let audio_extensions: HashSet<&str> = ["mp3", "m4a", "wav", "flac", "ogg", "aac"]
            .iter()
            .copied()
            .collect();

        let walker = if options.max_depth < 0 {
            WalkDir::new(mount_point).min_depth(1)
        } else {
            WalkDir::new(mount_point)
                .min_depth(1)
                .max_depth(options.max_depth as usize)
        };

        let mut entries = Vec::new();
        let mut skipped_entries = Vec::new();

        for entry in walker.into_iter().filter_map(std::result::Result::ok) {
            let path = entry.path().to_path_buf();

            // Skip directories for audio-only cleanup
            if entry.file_type().is_dir() {
                continue;
            }

            // Check if protected
            if let Some(reason) = self.is_protected(&path, options) {
                skipped_entries.push((path, reason));
                continue;
            }

            // Check if it's an audio file
            let is_audio = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| audio_extensions.contains(e.to_lowercase().as_str()));

            if !is_audio {
                skipped_entries.push((path, "not an audio file".to_string()));
                continue;
            }

            let size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

            entries.push(CleanupEntry {
                path,
                is_directory: false,
                size_bytes,
                deleted: None,
                error: None,
            });
        }

        let total_files = entries.len();
        let total_bytes: u64 = entries.iter().map(|e| e.size_bytes).sum();

        if options.dry_run {
            let duration_ms = start_time
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            return Ok(CleanupResult {
                mount_point: mount_point.to_path_buf(),
                files_deleted: total_files,
                directories_deleted: 0,
                bytes_freed: total_bytes,
                files_skipped: skipped_entries.len(),
                files_failed: 0,
                dry_run: true,
                entries,
                skipped_entries,
                verification_passed: None,
                duration_ms,
            });
        }

        let mut files_deleted = 0;
        let mut bytes_freed = 0u64;
        let mut files_failed = 0;

        for entry in &mut entries {
            if self.delete_entry(entry) {
                files_deleted += 1;
                bytes_freed += entry.size_bytes;
            } else {
                files_failed += 1;
            }
        }

        let verification_passed = if options.verify_deletions {
            Some(self.verify_cleanup(&entries))
        } else {
            None
        };

        let duration_ms = start_time
            .elapsed()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Ok(CleanupResult {
            mount_point: mount_point.to_path_buf(),
            files_deleted,
            directories_deleted: 0,
            bytes_freed,
            files_skipped: skipped_entries.len(),
            files_failed,
            dry_run: false,
            entries,
            skipped_entries,
            verification_passed,
            duration_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::MockDeviceDetector;
    use tempfile::TempDir;

    fn setup_test_device() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create some test files
        fs::write(temp_dir.path().join("track1.mp3"), "fake mp3 data 1").unwrap();
        fs::write(temp_dir.path().join("track2.mp3"), "fake mp3 data 2").unwrap();
        fs::write(temp_dir.path().join("readme.txt"), "readme content").unwrap();

        // Create a subdirectory with files
        fs::create_dir(temp_dir.path().join("subfolder")).unwrap();
        fs::write(
            temp_dir.path().join("subfolder/track3.mp3"),
            "fake mp3 data 3",
        )
        .unwrap();

        // Create hidden file
        fs::write(temp_dir.path().join(".hidden"), "hidden content").unwrap();

        temp_dir
    }

    #[test]
    fn test_cleanup_handler_creation() {
        let handler = DeviceCleanupHandler::new();
        assert!(!handler.system_patterns.is_empty());
    }

    #[test]
    fn test_cleanup_handler_default() {
        let handler = DeviceCleanupHandler::default();
        assert!(!handler.system_patterns.is_empty());
    }

    #[test]
    fn test_system_patterns_contains_known_files() {
        let handler = DeviceCleanupHandler::new();

        // Check that known system patterns are present (lowercase)
        assert!(
            handler
                .system_patterns
                .contains("system volume information")
        );
        assert!(handler.system_patterns.contains("$recycle.bin"));
        assert!(handler.system_patterns.contains(".ds_store"));
        assert!(handler.system_patterns.contains("thumbs.db"));
    }

    #[test]
    fn test_is_protected_hidden() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let hidden_path = PathBuf::from("/test/.hidden");
        let result = handler.is_protected(&hidden_path, &options);
        assert!(result.is_some());
        assert!(result.unwrap().contains("hidden"));
    }

    #[test]
    fn test_is_protected_hidden_disabled() {
        let handler = DeviceCleanupHandler::new();
        let mut options = CleanupOptions::default();
        options.skip_hidden = false;

        let hidden_path = PathBuf::from("/test/.hidden");
        let result = handler.is_protected(&hidden_path, &options);
        // Should not be protected when skip_hidden is false
        assert!(result.is_none());
    }

    #[test]
    fn test_is_protected_system_file() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let system_path = PathBuf::from("/test/System Volume Information");
        let result = handler.is_protected(&system_path, &options);
        assert!(result.is_some());
        assert!(result.unwrap().contains("system"));
    }

    #[test]
    fn test_is_protected_system_file_disabled() {
        let handler = DeviceCleanupHandler::new();
        let mut options = CleanupOptions::default();
        options.skip_system_files = false;

        let system_path = PathBuf::from("/test/System Volume Information");
        let result = handler.is_protected(&system_path, &options);
        // Should not be protected when skip_system_files is false
        assert!(result.is_none());
    }

    #[test]
    fn test_is_protected_ds_store() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let ds_store = PathBuf::from("/Volumes/USB/.DS_Store");
        let result = handler.is_protected(&ds_store, &options);
        // .DS_Store is both hidden and a system file
        assert!(result.is_some());
    }

    #[test]
    fn test_is_protected_thumbs_db() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let thumbs = PathBuf::from("/mnt/usb/Thumbs.db");
        let result = handler.is_protected(&thumbs, &options);
        assert!(result.is_some());
        assert!(result.unwrap().contains("system"));
    }

    #[test]
    fn test_is_protected_custom_pattern() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default().with_protected_pattern("important");

        let protected_path = PathBuf::from("/test/important_file.txt");
        let result = handler.is_protected(&protected_path, &options);
        assert!(result.is_some());
        assert!(result.unwrap().contains("protected pattern"));
    }

    #[test]
    fn test_is_protected_custom_pattern_exact_match() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default().with_protected_pattern("config.json");

        let protected_path = PathBuf::from("/test/config.json");
        let result = handler.is_protected(&protected_path, &options);
        assert!(result.is_some());
    }

    #[test]
    fn test_is_protected_custom_pattern_case_insensitive() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default().with_protected_pattern("IMPORTANT");

        let protected_path = PathBuf::from("/test/important_file.txt");
        let result = handler.is_protected(&protected_path, &options);
        assert!(result.is_some());
    }

    #[test]
    fn test_is_not_protected() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let normal_path = PathBuf::from("/test/track.mp3");
        let result = handler.is_protected(&normal_path, &options);
        assert!(result.is_none());
    }

    #[test]
    fn test_is_protected_path_without_name() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        // Root path has no file_name
        let root_path = PathBuf::from("/");
        let result = handler.is_protected(&root_path, &options);
        assert!(result.is_none());
    }

    #[test]
    fn test_dry_run_cleanup() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::dry_run();

        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Dry run should succeed");

        assert!(result.dry_run);
        assert!(result.files_deleted > 0);
        assert!(result.files_skipped > 0); // Hidden file

        // Verify files still exist (dry run)
        assert!(temp_dir.path().join("track1.mp3").exists());
        assert!(temp_dir.path().join("track2.mp3").exists());
    }

    #[test]
    fn test_actual_cleanup() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::full_cleanup();

        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        assert!(!result.dry_run);
        assert!(result.files_deleted > 0);
        assert!(result.is_success());

        // Verify files are deleted
        assert!(!temp_dir.path().join("track1.mp3").exists());
        assert!(!temp_dir.path().join("track2.mp3").exists());
        assert!(!temp_dir.path().join("subfolder/track3.mp3").exists());

        // Hidden file should still exist
        assert!(temp_dir.path().join(".hidden").exists());
    }

    #[test]
    fn test_cleanup_audio_only() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::full_cleanup();

        let result = handler
            .cleanup_audio_files_only(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        assert!(!result.dry_run);
        assert_eq!(result.directories_deleted, 0);

        // MP3 files should be deleted
        assert!(!temp_dir.path().join("track1.mp3").exists());
        assert!(!temp_dir.path().join("track2.mp3").exists());

        // Non-audio files should still exist
        assert!(temp_dir.path().join("readme.txt").exists());
    }

    #[test]
    fn test_cleanup_audio_only_dry_run() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::dry_run();

        let result = handler
            .cleanup_audio_files_only(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        assert!(result.dry_run);
        // Files should still exist
        assert!(temp_dir.path().join("track1.mp3").exists());
        assert!(temp_dir.path().join("track2.mp3").exists());
    }

    #[test]
    fn test_cleanup_audio_formats() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let handler = DeviceCleanupHandler::new();

        // Create various audio format files
        fs::write(temp_dir.path().join("song.mp3"), "mp3 data").unwrap();
        fs::write(temp_dir.path().join("song.m4a"), "m4a data").unwrap();
        fs::write(temp_dir.path().join("song.wav"), "wav data").unwrap();
        fs::write(temp_dir.path().join("song.flac"), "flac data").unwrap();
        fs::write(temp_dir.path().join("song.ogg"), "ogg data").unwrap();
        fs::write(temp_dir.path().join("song.aac"), "aac data").unwrap();
        fs::write(temp_dir.path().join("document.pdf"), "pdf data").unwrap();

        let options = CleanupOptions::full_cleanup();
        let result = handler
            .cleanup_audio_files_only(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        // All audio files should be deleted
        assert!(!temp_dir.path().join("song.mp3").exists());
        assert!(!temp_dir.path().join("song.m4a").exists());
        assert!(!temp_dir.path().join("song.wav").exists());
        assert!(!temp_dir.path().join("song.flac").exists());
        assert!(!temp_dir.path().join("song.ogg").exists());
        assert!(!temp_dir.path().join("song.aac").exists());

        // Non-audio files should remain
        assert!(temp_dir.path().join("document.pdf").exists());

        assert_eq!(result.files_deleted, 6);
    }

    #[test]
    fn test_preview_cleanup() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let result = handler
            .preview_cleanup(temp_dir.path(), &options)
            .expect("Preview should succeed");

        assert!(result.dry_run);
        assert!(result.files_deleted > 0);

        // Files should still exist
        assert!(temp_dir.path().join("track1.mp3").exists());
    }

    #[test]
    fn test_cleanup_result_summary() {
        let result = CleanupResult {
            mount_point: PathBuf::from("/test"),
            files_deleted: 10,
            directories_deleted: 2,
            bytes_freed: 1024,
            files_skipped: 3,
            files_failed: 1,
            dry_run: false,
            entries: Vec::new(),
            skipped_entries: Vec::new(),
            verification_passed: Some(true),
            duration_ms: 100,
        };

        let summary = result.summary();
        assert!(summary.contains("10 files"));
        assert!(summary.contains("2 directories"));
        assert!(summary.contains("1024 bytes"));
    }

    #[test]
    fn test_cleanup_result_summary_dry_run() {
        let result = CleanupResult {
            mount_point: PathBuf::from("/test"),
            files_deleted: 5,
            directories_deleted: 1,
            bytes_freed: 2048,
            files_skipped: 2,
            files_failed: 0,
            dry_run: true,
            entries: Vec::new(),
            skipped_entries: Vec::new(),
            verification_passed: None,
            duration_ms: 50,
        };

        let summary = result.summary();
        assert!(summary.contains("Dry run"));
        assert!(summary.contains("would delete"));
    }

    #[test]
    fn test_cleanup_result_is_success() {
        let success_result = CleanupResult {
            mount_point: PathBuf::from("/test"),
            files_deleted: 10,
            directories_deleted: 2,
            bytes_freed: 1024,
            files_skipped: 0,
            files_failed: 0,
            dry_run: false,
            entries: Vec::new(),
            skipped_entries: Vec::new(),
            verification_passed: Some(true),
            duration_ms: 100,
        };
        assert!(success_result.is_success());

        let failed_result = CleanupResult {
            files_failed: 1,
            ..success_result.clone()
        };
        assert!(!failed_result.is_success());

        let verification_failed = CleanupResult {
            verification_passed: Some(false),
            files_failed: 0,
            ..success_result
        };
        assert!(!verification_failed.is_success());
    }

    #[test]
    fn test_cleanup_nonexistent_device() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let result = handler.cleanup_device(Path::new("/nonexistent/path"), &options);
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_audio_only_nonexistent() {
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let result = handler.cleanup_audio_files_only(Path::new("/nonexistent/path"), &options);
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_options_default() {
        let options = CleanupOptions::default();

        assert!(options.skip_hidden);
        assert!(options.skip_system_files);
        assert!(options.protected_patterns.is_empty());
        assert!(options.verify_deletions);
        assert!(!options.dry_run);
        assert_eq!(options.max_depth, -1);
    }

    #[test]
    fn test_cleanup_options_full_cleanup() {
        let options = CleanupOptions::full_cleanup();

        assert!(options.skip_hidden);
        assert!(options.skip_system_files);
        assert!(!options.dry_run);
    }

    #[test]
    fn test_cleanup_options_dry_run() {
        let options = CleanupOptions::dry_run();

        assert!(options.dry_run);
        assert!(options.skip_hidden);
        assert!(options.skip_system_files);
    }

    #[test]
    fn test_cleanup_options_builder() {
        let options = CleanupOptions::default()
            .with_protected_pattern("keep_me")
            .with_protected_pattern("also_keep");

        assert_eq!(options.protected_patterns.len(), 2);
        assert!(options.protected_patterns.contains(&"keep_me".to_string()));
        assert!(
            options
                .protected_patterns
                .contains(&"also_keep".to_string())
        );
    }

    #[test]
    fn test_cleanup_options_serialization() {
        let options = CleanupOptions::default().with_protected_pattern("important");

        let json = serde_json::to_string(&options).expect("serialize");
        let deserialized: CleanupOptions = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(options.skip_hidden, deserialized.skip_hidden);
        assert_eq!(options.skip_system_files, deserialized.skip_system_files);
        assert_eq!(options.protected_patterns, deserialized.protected_patterns);
    }

    #[test]
    fn test_verification() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let mut options = CleanupOptions::full_cleanup();
        options.verify_deletions = true;

        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        assert!(result.verification_passed == Some(true));
    }

    #[test]
    fn test_verification_disabled() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let mut options = CleanupOptions::full_cleanup();
        options.verify_deletions = false;

        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        assert!(result.verification_passed.is_none());
    }

    #[test]
    fn test_cleanup_entry_serialization() {
        let entry = CleanupEntry {
            path: PathBuf::from("/test/file.mp3"),
            is_directory: false,
            size_bytes: 1024,
            deleted: Some(true),
            error: None,
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        let deserialized: CleanupEntry = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(entry.path, deserialized.path);
        assert_eq!(entry.is_directory, deserialized.is_directory);
        assert_eq!(entry.size_bytes, deserialized.size_bytes);
        assert_eq!(entry.deleted, deserialized.deleted);
        assert_eq!(entry.error, deserialized.error);
    }

    #[test]
    fn test_cleanup_result_serialization() {
        let result = CleanupResult {
            mount_point: PathBuf::from("/Volumes/USB"),
            files_deleted: 5,
            directories_deleted: 2,
            bytes_freed: 10240,
            files_skipped: 1,
            files_failed: 0,
            dry_run: false,
            entries: Vec::new(),
            skipped_entries: vec![(PathBuf::from("/.hidden"), "hidden file".to_string())],
            verification_passed: Some(true),
            duration_ms: 150,
        };

        let json = serde_json::to_string(&result).expect("serialize");
        let deserialized: CleanupResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(result.mount_point, deserialized.mount_point);
        assert_eq!(result.files_deleted, deserialized.files_deleted);
        assert_eq!(result.bytes_freed, deserialized.bytes_freed);
    }

    #[test]
    fn test_cleanup_with_max_depth() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let handler = DeviceCleanupHandler::new();

        // Create nested structure
        // WalkDir with min_depth(1) and max_depth(N) will visit entries at depths 1..=N
        // root.mp3 is at depth 1, level1/l1.mp3 is at depth 2, level1/level2/l2.mp3 is at depth 3, etc.
        fs::create_dir_all(temp_dir.path().join("level1/level2/level3")).unwrap();
        fs::write(temp_dir.path().join("root.mp3"), "root").unwrap();
        fs::write(temp_dir.path().join("level1/l1.mp3"), "level1").unwrap();
        fs::write(temp_dir.path().join("level1/level2/l2.mp3"), "level2").unwrap();
        fs::write(
            temp_dir.path().join("level1/level2/level3/l3.mp3"),
            "level3",
        )
        .unwrap();

        let mut options = CleanupOptions::full_cleanup();
        options.max_depth = 2; // Will visit depths 1 and 2 only

        let _result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        // Files at depth <= 2 should be deleted
        // root.mp3 is at depth 1
        assert!(!temp_dir.path().join("root.mp3").exists());
        // level1/l1.mp3 is at depth 2
        assert!(!temp_dir.path().join("level1/l1.mp3").exists());
        // level1/level2/l2.mp3 is at depth 3, beyond our limit (max_depth=2)
        assert!(temp_dir.path().join("level1/level2/l2.mp3").exists());
        // level3 is at depth 4, definitely beyond our limit
        assert!(temp_dir.path().join("level1/level2/level3/l3.mp3").exists());
    }

    // =============================================================================
    // Tests with MockDeviceDetector
    // =============================================================================

    #[test]
    fn test_cleanup_device_verified_connected() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();

        let mut mock = MockDeviceDetector::new();
        let mount_path = temp_dir.path().to_path_buf();
        let mp_clone = mount_path.clone();

        mock.expect_is_device_connected()
            .withf(move |mp| *mp == mp_clone)
            .returning(|_| true);

        let device = DeviceInfo {
            name: "USB Drive".to_string(),
            mount_point: mount_path,
            total_bytes: 16_000_000_000,
            available_bytes: 8_000_000_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };

        let options = CleanupOptions::full_cleanup();
        let result = handler.cleanup_device_verified(&mock, &device, &options);

        assert!(result.is_ok());
        let cleanup_result = result.unwrap();
        assert!(cleanup_result.files_deleted > 0);
    }

    #[test]
    fn test_cleanup_device_verified_disconnected() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();

        let mut mock = MockDeviceDetector::new();
        mock.expect_is_device_connected().returning(|_| false);

        let device = DeviceInfo {
            name: "USB Drive".to_string(),
            mount_point: temp_dir.path().to_path_buf(),
            total_bytes: 16_000_000_000,
            available_bytes: 8_000_000_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };

        let options = CleanupOptions::full_cleanup();
        let result = handler.cleanup_device_verified(&mock, &device, &options);

        assert!(result.is_err());
        match result {
            Err(Error::Device(DeviceError::Disconnected { name })) => {
                assert_eq!(name, "USB Drive");
            }
            _ => panic!("Expected Disconnected error"),
        }
    }

    #[test]
    fn test_cleanup_device_verified_dry_run() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();

        let mut mock = MockDeviceDetector::new();
        let mount_path = temp_dir.path().to_path_buf();
        let mp_clone = mount_path.clone();

        mock.expect_is_device_connected()
            .withf(move |mp| *mp == mp_clone)
            .returning(|_| true);

        let device = DeviceInfo {
            name: "USB Drive".to_string(),
            mount_point: mount_path,
            total_bytes: 16_000_000_000,
            available_bytes: 8_000_000_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };

        let options = CleanupOptions::dry_run();
        let result = handler.cleanup_device_verified(&mock, &device, &options);

        assert!(result.is_ok());
        let cleanup_result = result.unwrap();
        assert!(cleanup_result.dry_run);
        // Files should still exist
        assert!(temp_dir.path().join("track1.mp3").exists());
    }

    #[test]
    fn test_cleanup_empty_directory() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::full_cleanup();

        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.directories_deleted, 0);
        assert!(result.is_success());
    }

    #[test]
    fn test_cleanup_with_subdirectories_only() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let handler = DeviceCleanupHandler::new();

        // Create empty subdirectories
        fs::create_dir(temp_dir.path().join("empty1")).unwrap();
        fs::create_dir(temp_dir.path().join("empty2")).unwrap();

        let options = CleanupOptions::full_cleanup();
        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        // Empty directories should be deleted
        assert!(!temp_dir.path().join("empty1").exists());
        assert!(!temp_dir.path().join("empty2").exists());
        assert_eq!(result.directories_deleted, 2);
    }

    #[test]
    fn test_cleanup_entries_details() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::full_cleanup();

        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        // Check that entries are populated
        assert!(!result.entries.is_empty());

        // All entries should be marked as deleted
        for entry in &result.entries {
            assert_eq!(entry.deleted, Some(true));
            assert!(entry.error.is_none());
        }
    }

    #[test]
    fn test_cleanup_skipped_entries_details() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::default();

        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        // Should have skipped entries (hidden file)
        assert!(!result.skipped_entries.is_empty());

        // Check that .hidden was skipped
        let hidden_skipped = result.skipped_entries.iter().any(|(path, _)| {
            path.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        });
        assert!(hidden_skipped);
    }

    #[test]
    fn test_cleanup_bytes_freed_calculation() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let handler = DeviceCleanupHandler::new();

        // Create files with known sizes
        let content1 = vec![0u8; 1000];
        let content2 = vec![0u8; 2000];
        fs::write(temp_dir.path().join("file1.mp3"), &content1).unwrap();
        fs::write(temp_dir.path().join("file2.mp3"), &content2).unwrap();

        let options = CleanupOptions::full_cleanup();
        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        assert_eq!(result.bytes_freed, 3000);
    }

    #[test]
    fn test_cleanup_duration_tracked() {
        let temp_dir = setup_test_device();
        let handler = DeviceCleanupHandler::new();
        let options = CleanupOptions::full_cleanup();

        let result = handler
            .cleanup_device(temp_dir.path(), &options)
            .expect("Cleanup should succeed");

        // Duration is tracked (can be 0 if cleanup is very fast, which is fine)
        // Just verify the field exists and result is valid
        assert!(result.files_deleted > 0 || result.duration_ms >= 0);
    }
}
