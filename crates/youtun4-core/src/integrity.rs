//! File integrity verification module.
//!
//! This module provides checksum-based file integrity verification:
//! - SHA-256 checksum calculation for files
//! - Checksum manifest management (storage and retrieval)
//! - Batch verification of files against stored checksums
//! - Integration with the transfer engine for automatic manifest generation
//!
//! # Example
//!
//! ```rust,ignore
//! use youtun4_core::integrity::{ChecksumManifest, IntegrityVerifier, VerificationResult};
//! use std::path::Path;
//!
//! // Create a manifest from transferred files
//! let manifest = ChecksumManifest::from_transfer_result(&transfer_result);
//! manifest.save(Path::new("/device/checksums.json"))?;
//!
//! // Later, verify files against the manifest
//! let verifier = IntegrityVerifier::new();
//! let result = verifier.verify_directory(Path::new("/device"), &manifest)?;
//! println!("Verification: {} passed, {} failed", result.passed, result.failed);
//! ```

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::error::{Error, FileSystemError, Result};
use crate::transfer::{DEFAULT_CHUNK_SIZE, TransferResult, TransferredFile};

// =============================================================================
// Constants
// =============================================================================

/// Default manifest file name.
pub const DEFAULT_MANIFEST_FILE: &str = "checksums.json";

/// Manifest format version for forward compatibility.
pub const MANIFEST_VERSION: u32 = 1;

// =============================================================================
// File Checksum
// =============================================================================

/// Checksum information for a single file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChecksum {
    /// File name (relative to the manifest location).
    pub file_name: String,

    /// SHA-256 checksum as a lowercase hex string.
    pub checksum: String,

    /// File size in bytes.
    pub size_bytes: u64,

    /// Timestamp when the checksum was computed (Unix epoch seconds).
    pub computed_at: u64,
}

impl FileChecksum {
    /// Create a new file checksum entry.
    #[must_use]
    pub fn new(file_name: String, checksum: String, size_bytes: u64) -> Self {
        let computed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        Self {
            file_name,
            checksum,
            size_bytes,
            computed_at,
        }
    }

    /// Create a file checksum from a transferred file.
    #[must_use]
    pub fn from_transferred_file(file: &TransferredFile) -> Option<Self> {
        let checksum = file.checksum.as_ref()?;
        let file_name = file.destination.file_name()?.to_str()?.to_string();

        Some(Self::new(file_name, checksum.clone(), file.size_bytes))
    }
}

// =============================================================================
// Checksum Manifest
// =============================================================================

/// A manifest containing checksums for multiple files.
///
/// The manifest is typically stored as a JSON file (checksums.json) and can be
/// used to verify file integrity after transfers or detect corruption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecksumManifest {
    /// Manifest format version.
    pub version: u32,

    /// When the manifest was created (Unix epoch seconds).
    pub created_at: u64,

    /// When the manifest was last updated (Unix epoch seconds).
    pub updated_at: u64,

    /// Optional description or source information.
    pub description: Option<String>,

    /// Map of file names to their checksums.
    #[serde(default)]
    pub files: HashMap<String, FileChecksum>,
}

impl Default for ChecksumManifest {
    fn default() -> Self {
        Self::new()
    }
}

impl ChecksumManifest {
    /// Create a new empty manifest.
    #[must_use]
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        Self {
            version: MANIFEST_VERSION,
            created_at: now,
            updated_at: now,
            description: None,
            files: HashMap::new(),
        }
    }

    /// Create a manifest with a description.
    #[must_use]
    pub fn with_description(description: impl Into<String>) -> Self {
        let mut manifest = Self::new();
        manifest.description = Some(description.into());
        manifest
    }

    /// Create a manifest from a transfer result.
    ///
    /// Extracts checksums from all successfully transferred files that have
    /// checksum information available.
    #[must_use]
    pub fn from_transfer_result(result: &TransferResult) -> Self {
        let mut manifest = Self::with_description("Generated from file transfer");

        for file in &result.transferred_files {
            if file.skipped {
                continue;
            }

            if let Some(file_checksum) = FileChecksum::from_transferred_file(file) {
                manifest.add_file(file_checksum);
            }
        }

        manifest
    }

    /// Add a file checksum to the manifest.
    pub fn add_file(&mut self, checksum: FileChecksum) {
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.files.insert(checksum.file_name.clone(), checksum);
    }

    /// Remove a file from the manifest.
    pub fn remove_file(&mut self, file_name: &str) -> Option<FileChecksum> {
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.files.remove(file_name)
    }

    /// Get a file checksum by name.
    #[must_use]
    pub fn get_file(&self, file_name: &str) -> Option<&FileChecksum> {
        self.files.get(file_name)
    }

    /// Get the number of files in the manifest.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if the manifest is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Merge another manifest into this one.
    ///
    /// Files from the other manifest will overwrite files with the same name
    /// in this manifest.
    pub fn merge(&mut self, other: &Self) {
        for (name, checksum) in &other.files {
            self.files.insert(name.clone(), checksum.clone());
        }
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
    }

    /// Load a manifest from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        let manifest: Self = serde_json::from_str(&content)?;

        debug!(
            "Loaded checksum manifest with {} files from {}",
            manifest.files.len(),
            path.display()
        );

        Ok(manifest)
    }

    /// Load a manifest from a directory (using default filename).
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or parsed.
    pub fn load_from_directory(dir: &Path) -> Result<Self> {
        Self::load(&dir.join(DEFAULT_MANIFEST_FILE))
    }

    /// Save the manifest to a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;

        fs::write(path, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        debug!(
            "Saved checksum manifest with {} files to {}",
            self.files.len(),
            path.display()
        );

        Ok(())
    }

    /// Save the manifest to a directory (using default filename).
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save_to_directory(&self, dir: &Path) -> Result<()> {
        self.save(&dir.join(DEFAULT_MANIFEST_FILE))
    }
}

// =============================================================================
// Verification Result
// =============================================================================

/// Result of verifying a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVerificationResult {
    /// File name.
    pub file_name: String,

    /// Full path to the file.
    pub path: PathBuf,

    /// Whether the file passed verification.
    pub passed: bool,

    /// Expected checksum from manifest.
    pub expected_checksum: String,

    /// Actual computed checksum (if file exists).
    pub actual_checksum: Option<String>,

    /// Expected file size.
    pub expected_size: u64,

    /// Actual file size (if file exists).
    pub actual_size: Option<u64>,

    /// Error message if verification failed.
    pub error: Option<String>,

    /// Time taken to verify (in milliseconds).
    pub duration_ms: u64,
}

impl FileVerificationResult {
    /// Create a passed result.
    fn passed(
        file_name: String,
        path: PathBuf,
        checksum: String,
        size: u64,
        duration_ms: u64,
    ) -> Self {
        Self {
            file_name,
            path,
            passed: true,
            expected_checksum: checksum.clone(),
            actual_checksum: Some(checksum),
            expected_size: size,
            actual_size: Some(size),
            error: None,
            duration_ms,
        }
    }

    /// Create a failed result due to checksum mismatch.
    fn checksum_mismatch(
        file_name: String,
        path: PathBuf,
        expected: String,
        actual: String,
        expected_size: u64,
        actual_size: u64,
        duration_ms: u64,
    ) -> Self {
        Self {
            file_name,
            path,
            passed: false,
            expected_checksum: expected,
            actual_checksum: Some(actual),
            expected_size,
            actual_size: Some(actual_size),
            error: Some("Checksum mismatch".to_string()),
            duration_ms,
        }
    }

    /// Create a failed result due to missing file.
    fn missing_file(
        file_name: String,
        path: PathBuf,
        expected_checksum: String,
        expected_size: u64,
    ) -> Self {
        Self {
            file_name,
            path,
            passed: false,
            expected_checksum,
            actual_checksum: None,
            expected_size,
            actual_size: None,
            error: Some("File not found".to_string()),
            duration_ms: 0,
        }
    }

    /// Create a failed result due to size mismatch.
    fn size_mismatch(
        file_name: String,
        path: PathBuf,
        expected_checksum: String,
        expected_size: u64,
        actual_size: u64,
    ) -> Self {
        Self {
            file_name,
            path,
            passed: false,
            expected_checksum,
            actual_checksum: None,
            expected_size,
            actual_size: Some(actual_size),
            error: Some(format!(
                "Size mismatch: expected {expected_size} bytes, got {actual_size} bytes"
            )),
            duration_ms: 0,
        }
    }

    /// Create a failed result due to an error.
    const fn error(
        file_name: String,
        path: PathBuf,
        expected_checksum: String,
        expected_size: u64,
        error: String,
    ) -> Self {
        Self {
            file_name,
            path,
            passed: false,
            expected_checksum,
            actual_checksum: None,
            expected_size,
            actual_size: None,
            error: Some(error),
            duration_ms: 0,
        }
    }
}

/// Overall result of verifying multiple files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Total number of files verified.
    pub total_files: usize,

    /// Number of files that passed verification.
    pub passed: usize,

    /// Number of files that failed verification.
    pub failed: usize,

    /// Number of files in manifest not found on disk.
    pub missing: usize,

    /// Number of files on disk not in manifest.
    pub extra_files: usize,

    /// Total bytes verified.
    pub total_bytes: u64,

    /// Total duration of verification (in seconds).
    pub duration_secs: f64,

    /// Individual file results.
    pub file_results: Vec<FileVerificationResult>,

    /// List of extra files found on disk but not in manifest.
    pub extra_file_names: Vec<String>,

    /// Whether all verifications passed.
    pub success: bool,
}

impl VerificationResult {
    /// Create a new verification result builder.
    const fn new() -> Self {
        Self {
            total_files: 0,
            passed: 0,
            failed: 0,
            missing: 0,
            extra_files: 0,
            total_bytes: 0,
            duration_secs: 0.0,
            file_results: Vec::new(),
            extra_file_names: Vec::new(),
            success: true,
        }
    }

    /// Add a file result.
    fn add_result(&mut self, result: FileVerificationResult) {
        self.total_files += 1;

        if result.passed {
            self.passed += 1;
            if let Some(size) = result.actual_size {
                self.total_bytes += size;
            }
        } else {
            self.failed += 1;
            self.success = false;
            if result
                .error
                .as_ref()
                .is_some_and(|e| e.contains("not found"))
            {
                self.missing += 1;
            }
        }

        self.file_results.push(result);
    }

    /// Add an extra file (on disk but not in manifest).
    fn add_extra_file(&mut self, file_name: String) {
        self.extra_files += 1;
        self.extra_file_names.push(file_name);
    }

    /// Get failed file results.
    #[must_use]
    pub fn get_failures(&self) -> Vec<&FileVerificationResult> {
        self.file_results.iter().filter(|r| !r.passed).collect()
    }

    /// Get passed file results.
    #[must_use]
    pub fn get_passed(&self) -> Vec<&FileVerificationResult> {
        self.file_results.iter().filter(|r| r.passed).collect()
    }
}

// =============================================================================
// Verification Progress
// =============================================================================

/// Progress information during verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationProgress {
    /// Current file index (1-based).
    pub current_file: usize,

    /// Total number of files to verify.
    pub total_files: usize,

    /// Name of the current file being verified.
    pub current_file_name: String,

    /// Number of files verified so far.
    pub verified: usize,

    /// Number of files that passed so far.
    pub passed: usize,

    /// Number of files that failed so far.
    pub failed: usize,

    /// Bytes verified so far.
    pub bytes_verified: u64,

    /// Total bytes to verify.
    pub total_bytes: u64,

    /// Elapsed time in seconds.
    pub elapsed_secs: f64,
}

impl VerificationProgress {
    /// Calculate progress percentage.
    #[must_use]
    pub fn percentage(&self) -> f64 {
        if self.total_files == 0 {
            return 100.0;
        }
        (self.verified as f64 / self.total_files as f64) * 100.0
    }
}

// =============================================================================
// Integrity Verifier
// =============================================================================

/// Options for integrity verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationOptions {
    /// Whether to check for extra files not in the manifest.
    pub check_extra_files: bool,

    /// Whether to fail fast on first error.
    pub fail_fast: bool,

    /// Whether to verify file sizes before computing checksums.
    pub verify_sizes_first: bool,

    /// Chunk size for reading files (in bytes).
    pub chunk_size: usize,

    /// File extensions to check for extra files (empty = all files).
    pub file_extensions: Vec<String>,
}

impl Default for VerificationOptions {
    fn default() -> Self {
        Self {
            check_extra_files: true,
            fail_fast: false,
            verify_sizes_first: true,
            chunk_size: DEFAULT_CHUNK_SIZE,
            file_extensions: vec!["mp3".to_string(), "m4a".to_string()],
        }
    }
}

impl VerificationOptions {
    /// Create options for strict verification.
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            check_extra_files: true,
            fail_fast: false,
            verify_sizes_first: true,
            chunk_size: DEFAULT_CHUNK_SIZE,
            file_extensions: Vec::new(), // Check all files
        }
    }

    /// Create options for quick verification.
    #[must_use]
    pub fn quick() -> Self {
        Self {
            check_extra_files: false,
            fail_fast: true,
            verify_sizes_first: true,
            chunk_size: DEFAULT_CHUNK_SIZE * 2,
            file_extensions: vec!["mp3".to_string()],
        }
    }
}

/// File integrity verifier.
///
/// Provides methods for verifying files against checksum manifests.
pub struct IntegrityVerifier {
    options: VerificationOptions,
}

impl Default for IntegrityVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegrityVerifier {
    /// Create a new verifier with default options.
    #[must_use]
    pub fn new() -> Self {
        Self {
            options: VerificationOptions::default(),
        }
    }

    /// Create a verifier with custom options.
    #[must_use]
    pub const fn with_options(options: VerificationOptions) -> Self {
        Self { options }
    }

    /// Compute the SHA-256 checksum of a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn compute_checksum(&self, path: &Path) -> Result<String> {
        let file = File::open(path).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; self.options.chunk_size];

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                Error::FileSystem(FileSystemError::ReadFailed {
                    path: path.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Verify a single file against an expected checksum.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn verify_file(
        &self,
        path: &Path,
        expected: &FileChecksum,
    ) -> Result<FileVerificationResult> {
        let file_name = expected.file_name.clone();
        let start = Instant::now();

        // Check if file exists
        if !path.exists() {
            return Ok(FileVerificationResult::missing_file(
                file_name,
                path.to_path_buf(),
                expected.checksum.clone(),
                expected.size_bytes,
            ));
        }

        // Get file size
        let metadata = fs::metadata(path).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })?;
        let actual_size = metadata.len();

        // Optionally check size first
        if self.options.verify_sizes_first && actual_size != expected.size_bytes {
            return Ok(FileVerificationResult::size_mismatch(
                file_name,
                path.to_path_buf(),
                expected.checksum.clone(),
                expected.size_bytes,
                actual_size,
            ));
        }

        // Compute checksum
        let actual_checksum = self.compute_checksum(path)?;
        let duration_ms = start.elapsed().as_millis() as u64;

        if actual_checksum == expected.checksum {
            Ok(FileVerificationResult::passed(
                file_name,
                path.to_path_buf(),
                actual_checksum,
                actual_size,
                duration_ms,
            ))
        } else {
            Ok(FileVerificationResult::checksum_mismatch(
                file_name,
                path.to_path_buf(),
                expected.checksum.clone(),
                actual_checksum,
                expected.size_bytes,
                actual_size,
                duration_ms,
            ))
        }
    }

    /// Verify all files in a manifest against a directory.
    ///
    /// # Arguments
    ///
    /// * `directory` - Directory containing the files to verify
    /// * `manifest` - Manifest containing expected checksums
    /// * `progress_callback` - Optional callback for progress updates
    ///
    /// # Errors
    ///
    /// Returns an error if verification fails unexpectedly.
    pub fn verify_directory<F>(
        &self,
        directory: &Path,
        manifest: &ChecksumManifest,
        mut progress_callback: Option<F>,
    ) -> Result<VerificationResult>
    where
        F: FnMut(&VerificationProgress),
    {
        let start = Instant::now();
        let mut result = VerificationResult::new();

        // Calculate total bytes
        let total_bytes: u64 = manifest.files.values().map(|f| f.size_bytes).sum();

        // Initialize progress
        let mut progress = VerificationProgress {
            current_file: 0,
            total_files: manifest.files.len(),
            current_file_name: String::new(),
            verified: 0,
            passed: 0,
            failed: 0,
            bytes_verified: 0,
            total_bytes,
            elapsed_secs: 0.0,
        };

        info!(
            "Starting verification of {} files in {}",
            manifest.files.len(),
            directory.display()
        );

        // Verify each file in the manifest
        for (index, (file_name, expected)) in manifest.files.iter().enumerate() {
            progress.current_file = index + 1;
            progress.current_file_name.clone_from(file_name);
            progress.elapsed_secs = start.elapsed().as_secs_f64();

            if let Some(ref mut cb) = progress_callback {
                cb(&progress);
            }

            let file_path = directory.join(file_name);

            match self.verify_file(&file_path, expected) {
                Ok(file_result) => {
                    if file_result.passed {
                        progress.passed += 1;
                        progress.bytes_verified += expected.size_bytes;
                    } else {
                        progress.failed += 1;

                        if self.options.fail_fast {
                            result.add_result(file_result);
                            result.duration_secs = start.elapsed().as_secs_f64();
                            return Ok(result);
                        }
                    }
                    result.add_result(file_result);
                }
                Err(e) => {
                    let file_result = FileVerificationResult::error(
                        file_name.clone(),
                        file_path,
                        expected.checksum.clone(),
                        expected.size_bytes,
                        e.to_string(),
                    );
                    progress.failed += 1;
                    result.add_result(file_result);

                    if self.options.fail_fast {
                        result.duration_secs = start.elapsed().as_secs_f64();
                        return Ok(result);
                    }
                }
            }

            progress.verified += 1;
        }

        // Check for extra files if enabled
        if self.options.check_extra_files {
            self.check_extra_files(directory, manifest, &mut result)?;
        }

        result.duration_secs = start.elapsed().as_secs_f64();

        info!(
            "Verification complete: {} passed, {} failed, {} extra files in {:.2}s",
            result.passed, result.failed, result.extra_files, result.duration_secs
        );

        // Final progress update
        progress.elapsed_secs = result.duration_secs;
        if let Some(ref mut cb) = progress_callback {
            cb(&progress);
        }

        Ok(result)
    }

    /// Check for extra files in the directory not present in the manifest.
    fn check_extra_files(
        &self,
        directory: &Path,
        manifest: &ChecksumManifest,
        result: &mut VerificationResult,
    ) -> Result<()> {
        let entries = fs::read_dir(directory).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: directory.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            // Skip manifest file itself
            if file_name == DEFAULT_MANIFEST_FILE {
                continue;
            }

            // Skip files with extensions not in our list (if specified)
            if !self.options.file_extensions.is_empty() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase)
                    .unwrap_or_default();

                if !self.options.file_extensions.iter().any(|e| e == &ext) {
                    continue;
                }
            }

            // Check if file is in manifest
            if !manifest.files.contains_key(file_name) {
                debug!("Extra file found: {}", file_name);
                result.add_extra_file(file_name.to_string());
            }
        }

        Ok(())
    }

    /// Create a manifest from all files in a directory.
    ///
    /// # Arguments
    ///
    /// * `directory` - Directory to scan
    /// * `progress_callback` - Optional callback for progress updates
    ///
    /// # Errors
    ///
    /// Returns an error if files cannot be read.
    pub fn create_manifest_from_directory<F>(
        &self,
        directory: &Path,
        mut progress_callback: Option<F>,
    ) -> Result<ChecksumManifest>
    where
        F: FnMut(&VerificationProgress),
    {
        let mut manifest = ChecksumManifest::with_description(format!(
            "Generated from directory: {}",
            directory.display()
        ));

        // Collect files to process
        let mut files_to_process: Vec<(String, PathBuf, u64)> = Vec::new();

        let entries = fs::read_dir(directory).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: directory.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Skip manifest file itself
            if file_name == DEFAULT_MANIFEST_FILE {
                continue;
            }

            // Check file extension filter
            if !self.options.file_extensions.is_empty() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase)
                    .unwrap_or_default();

                if !self.options.file_extensions.iter().any(|e| e == &ext) {
                    continue;
                }
            }

            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            files_to_process.push((file_name, path, size));
        }

        let total_files = files_to_process.len();
        let total_bytes: u64 = files_to_process.iter().map(|(_, _, s)| s).sum();

        let mut progress = VerificationProgress {
            current_file: 0,
            total_files,
            current_file_name: String::new(),
            verified: 0,
            passed: 0,
            failed: 0,
            bytes_verified: 0,
            total_bytes,
            elapsed_secs: 0.0,
        };

        let start = Instant::now();

        info!(
            "Creating manifest for {} files in {}",
            total_files,
            directory.display()
        );

        for (index, (file_name, path, size)) in files_to_process.iter().enumerate() {
            progress.current_file = index + 1;
            progress.current_file_name.clone_from(file_name);
            progress.elapsed_secs = start.elapsed().as_secs_f64();

            if let Some(ref mut cb) = progress_callback {
                cb(&progress);
            }

            match self.compute_checksum(path) {
                Ok(checksum) => {
                    let file_checksum = FileChecksum::new(file_name.clone(), checksum, *size);
                    manifest.add_file(file_checksum);
                    progress.passed += 1;
                    progress.bytes_verified += size;
                }
                Err(e) => {
                    warn!("Failed to compute checksum for {}: {}", file_name, e);
                    progress.failed += 1;
                }
            }

            progress.verified += 1;
        }

        info!(
            "Manifest created with {} files in {:.2}s",
            manifest.len(),
            start.elapsed().as_secs_f64()
        );

        Ok(manifest)
    }
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Compute the SHA-256 checksum of a file.
///
/// This is a convenience function that creates a temporary verifier.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn compute_file_checksum(path: &Path) -> Result<String> {
    IntegrityVerifier::new().compute_checksum(path)
}

/// Verify a directory against a manifest file.
///
/// This is a convenience function that loads the manifest and verifies files.
///
/// # Errors
///
/// Returns an error if verification fails.
pub fn verify_directory(directory: &Path) -> Result<VerificationResult> {
    let manifest = ChecksumManifest::load_from_directory(directory)?;
    let verifier = IntegrityVerifier::new();
    verifier.verify_directory(directory, &manifest, None::<fn(&VerificationProgress)>)
}

/// Create and save a manifest for a directory.
///
/// # Errors
///
/// Returns an error if the manifest cannot be created or saved.
pub fn create_and_save_manifest(directory: &Path) -> Result<ChecksumManifest> {
    let verifier = IntegrityVerifier::new();
    let manifest =
        verifier.create_manifest_from_directory(directory, None::<fn(&VerificationProgress)>)?;
    manifest.save_to_directory(directory)?;
    Ok(manifest)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut file = File::create(&path).expect("create file");
        file.write_all(content).expect("write content");
        path
    }

    #[test]
    fn test_file_checksum_new() {
        let checksum = FileChecksum::new("test.mp3".to_string(), "abc123".to_string(), 1000);

        assert_eq!(checksum.file_name, "test.mp3");
        assert_eq!(checksum.checksum, "abc123");
        assert_eq!(checksum.size_bytes, 1000);
        assert!(checksum.computed_at > 0);
    }

    #[test]
    fn test_manifest_new() {
        let manifest = ChecksumManifest::new();

        assert_eq!(manifest.version, MANIFEST_VERSION);
        assert!(manifest.files.is_empty());
        assert!(manifest.created_at > 0);
    }

    #[test]
    fn test_manifest_add_file() {
        let mut manifest = ChecksumManifest::new();
        let checksum = FileChecksum::new("test.mp3".to_string(), "abc123".to_string(), 1000);

        manifest.add_file(checksum);

        assert_eq!(manifest.len(), 1);
        assert!(manifest.get_file("test.mp3").is_some());
    }

    #[test]
    fn test_manifest_remove_file() {
        let mut manifest = ChecksumManifest::new();
        let checksum = FileChecksum::new("test.mp3".to_string(), "abc123".to_string(), 1000);
        manifest.add_file(checksum);

        let removed = manifest.remove_file("test.mp3");

        assert!(removed.is_some());
        assert!(manifest.is_empty());
    }

    #[test]
    fn test_manifest_merge() {
        let mut manifest1 = ChecksumManifest::new();
        manifest1.add_file(FileChecksum::new(
            "file1.mp3".to_string(),
            "aaa".to_string(),
            100,
        ));

        let mut manifest2 = ChecksumManifest::new();
        manifest2.add_file(FileChecksum::new(
            "file2.mp3".to_string(),
            "bbb".to_string(),
            200,
        ));

        manifest1.merge(&manifest2);

        assert_eq!(manifest1.len(), 2);
        assert!(manifest1.get_file("file1.mp3").is_some());
        assert!(manifest1.get_file("file2.mp3").is_some());
    }

    #[test]
    fn test_manifest_save_load() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let manifest_path = temp_dir.path().join("checksums.json");

        let mut manifest = ChecksumManifest::with_description("Test manifest");
        manifest.add_file(FileChecksum::new(
            "song.mp3".to_string(),
            "abc123".to_string(),
            5000,
        ));

        manifest.save(&manifest_path).expect("save manifest");

        let loaded = ChecksumManifest::load(&manifest_path).expect("load manifest");

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.description, Some("Test manifest".to_string()));
        assert!(loaded.get_file("song.mp3").is_some());
    }

    #[test]
    fn test_compute_checksum() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let content = b"Hello, World!";
        let file_path = create_test_file(temp_dir.path(), "test.txt", content);

        let verifier = IntegrityVerifier::new();
        let checksum = verifier
            .compute_checksum(&file_path)
            .expect("compute checksum");

        // SHA-256 of "Hello, World!" is known
        assert_eq!(checksum.len(), 64); // 64 hex chars

        // Verify same content produces same checksum
        let checksum2 = verifier
            .compute_checksum(&file_path)
            .expect("compute checksum");
        assert_eq!(checksum, checksum2);
    }

    #[test]
    fn test_verify_file_passed() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let content = b"Test content for verification";
        let file_path = create_test_file(temp_dir.path(), "test.mp3", content);

        let verifier = IntegrityVerifier::new();
        let actual_checksum = verifier
            .compute_checksum(&file_path)
            .expect("compute checksum");

        let expected = FileChecksum::new(
            "test.mp3".to_string(),
            actual_checksum,
            content.len() as u64,
        );

        let result = verifier
            .verify_file(&file_path, &expected)
            .expect("verify file");

        assert!(result.passed);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_verify_file_checksum_mismatch() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let content = b"Test content";
        let file_path = create_test_file(temp_dir.path(), "test.mp3", content);

        let verifier = IntegrityVerifier::new();

        let expected = FileChecksum::new(
            "test.mp3".to_string(),
            "wrongchecksum".to_string(),
            content.len() as u64,
        );

        let result = verifier
            .verify_file(&file_path, &expected)
            .expect("verify file");

        assert!(!result.passed);
        assert!(result.error.as_ref().unwrap().contains("Checksum mismatch"));
    }

    #[test]
    fn test_verify_file_missing() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("nonexistent.mp3");

        let verifier = IntegrityVerifier::new();

        let expected = FileChecksum::new("nonexistent.mp3".to_string(), "abc123".to_string(), 1000);

        let result = verifier
            .verify_file(&file_path, &expected)
            .expect("verify file");

        assert!(!result.passed);
        assert!(result.error.as_ref().unwrap().contains("not found"));
    }

    #[test]
    fn test_verify_file_size_mismatch() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let content = b"Short";
        let file_path = create_test_file(temp_dir.path(), "test.mp3", content);

        let verifier = IntegrityVerifier::new();

        let expected = FileChecksum::new(
            "test.mp3".to_string(),
            "doesn't matter".to_string(),
            1000, // Wrong size
        );

        let result = verifier
            .verify_file(&file_path, &expected)
            .expect("verify file");

        assert!(!result.passed);
        assert!(result.error.as_ref().unwrap().contains("Size mismatch"));
    }

    #[test]
    fn test_verify_directory() {
        let temp_dir = TempDir::new().expect("create temp dir");

        // Create test files
        let content1 = b"File one content";
        let content2 = b"File two content";
        create_test_file(temp_dir.path(), "song1.mp3", content1);
        create_test_file(temp_dir.path(), "song2.mp3", content2);

        let verifier = IntegrityVerifier::new();

        // Create manifest with correct checksums
        let checksum1 = verifier
            .compute_checksum(&temp_dir.path().join("song1.mp3"))
            .unwrap();
        let checksum2 = verifier
            .compute_checksum(&temp_dir.path().join("song2.mp3"))
            .unwrap();

        let mut manifest = ChecksumManifest::new();
        manifest.add_file(FileChecksum::new(
            "song1.mp3".to_string(),
            checksum1,
            content1.len() as u64,
        ));
        manifest.add_file(FileChecksum::new(
            "song2.mp3".to_string(),
            checksum2,
            content2.len() as u64,
        ));

        let result = verifier
            .verify_directory(
                temp_dir.path(),
                &manifest,
                None::<fn(&VerificationProgress)>,
            )
            .expect("verify directory");

        assert!(result.success);
        assert_eq!(result.passed, 2);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn test_verify_directory_with_extra_files() {
        let temp_dir = TempDir::new().expect("create temp dir");

        // Create test files
        create_test_file(temp_dir.path(), "song1.mp3", b"Content");
        create_test_file(temp_dir.path(), "song2.mp3", b"Extra file"); // Extra

        let verifier = IntegrityVerifier::new();
        let checksum = verifier
            .compute_checksum(&temp_dir.path().join("song1.mp3"))
            .unwrap();

        // Manifest only has song1.mp3
        let mut manifest = ChecksumManifest::new();
        manifest.add_file(FileChecksum::new("song1.mp3".to_string(), checksum, 7));

        let result = verifier
            .verify_directory(
                temp_dir.path(),
                &manifest,
                None::<fn(&VerificationProgress)>,
            )
            .expect("verify directory");

        assert_eq!(result.extra_files, 1);
        assert!(result.extra_file_names.contains(&"song2.mp3".to_string()));
    }

    #[test]
    fn test_create_manifest_from_directory() {
        let temp_dir = TempDir::new().expect("create temp dir");

        create_test_file(temp_dir.path(), "song1.mp3", b"Content 1");
        create_test_file(temp_dir.path(), "song2.mp3", b"Content 2");
        create_test_file(temp_dir.path(), "readme.txt", b"Not audio"); // Should be ignored

        let verifier = IntegrityVerifier::new();
        let manifest = verifier
            .create_manifest_from_directory(temp_dir.path(), None::<fn(&VerificationProgress)>)
            .expect("create manifest");

        assert_eq!(manifest.len(), 2);
        assert!(manifest.get_file("song1.mp3").is_some());
        assert!(manifest.get_file("song2.mp3").is_some());
        assert!(manifest.get_file("readme.txt").is_none());
    }

    #[test]
    fn test_verification_options_default() {
        let options = VerificationOptions::default();

        assert!(options.check_extra_files);
        assert!(!options.fail_fast);
        assert!(options.verify_sizes_first);
    }

    #[test]
    fn test_verification_options_strict() {
        let options = VerificationOptions::strict();

        assert!(options.check_extra_files);
        assert!(options.file_extensions.is_empty());
    }

    #[test]
    fn test_verification_options_quick() {
        let options = VerificationOptions::quick();

        assert!(!options.check_extra_files);
        assert!(options.fail_fast);
    }

    #[test]
    fn test_verification_result_get_failures() {
        let mut result = VerificationResult::new();

        result.add_result(FileVerificationResult::passed(
            "good.mp3".to_string(),
            PathBuf::from("/good.mp3"),
            "abc".to_string(),
            100,
            10,
        ));

        result.add_result(FileVerificationResult::missing_file(
            "bad.mp3".to_string(),
            PathBuf::from("/bad.mp3"),
            "xyz".to_string(),
            200,
        ));

        let failures = result.get_failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].file_name, "bad.mp3");
    }

    #[test]
    fn test_verification_progress_percentage() {
        let progress = VerificationProgress {
            current_file: 3,
            total_files: 10,
            current_file_name: "test.mp3".to_string(),
            verified: 5,
            passed: 4,
            failed: 1,
            bytes_verified: 5000,
            total_bytes: 10000,
            elapsed_secs: 1.5,
        };

        assert!((progress.percentage() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_convenience_compute_file_checksum() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let content = b"Test content";
        let file_path = create_test_file(temp_dir.path(), "test.txt", content);

        let checksum = compute_file_checksum(&file_path).expect("compute checksum");

        assert_eq!(checksum.len(), 64);
    }

    #[test]
    fn test_manifest_from_transfer_result() {
        let transfer_result = TransferResult {
            total_files: 2,
            files_transferred: 2,
            files_skipped: 0,
            files_failed: 0,
            bytes_transferred: 2000,
            bytes_skipped: 0,
            duration_secs: 1.0,
            average_speed_bps: 2000.0,
            transferred_files: vec![
                TransferredFile {
                    source: PathBuf::from("/src/song1.mp3"),
                    destination: PathBuf::from("/dst/song1.mp3"),
                    size_bytes: 1000,
                    checksum: Some("aaa111".to_string()),
                    duration_secs: 0.5,
                    skipped: false,
                },
                TransferredFile {
                    source: PathBuf::from("/src/song2.mp3"),
                    destination: PathBuf::from("/dst/song2.mp3"),
                    size_bytes: 1000,
                    checksum: Some("bbb222".to_string()),
                    duration_secs: 0.5,
                    skipped: false,
                },
            ],
            failed_transfers: vec![],
            was_cancelled: false,
            success: true,
        };

        let manifest = ChecksumManifest::from_transfer_result(&transfer_result);

        assert_eq!(manifest.len(), 2);
        assert!(manifest.get_file("song1.mp3").is_some());
        assert!(manifest.get_file("song2.mp3").is_some());
        assert_eq!(manifest.get_file("song1.mp3").unwrap().checksum, "aaa111");
    }
}
