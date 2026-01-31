//! Integrity verification commands.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info};
use youtun4_core::Error;
use youtun4_core::integrity::{
    ChecksumManifest, FileChecksum, IntegrityVerifier, VerificationOptions, VerificationProgress,
    VerificationResult,
};

use super::error::map_err;

/// Event names for integrity verification events.
pub mod integrity_events {
    /// Event emitted for verification progress updates.
    pub const VERIFICATION_PROGRESS: &str = "integrity-verification-progress";
    /// Event emitted when verification completes.
    pub const VERIFICATION_COMPLETED: &str = "integrity-verification-completed";
}

/// Create a checksum manifest for a directory.
#[tauri::command]
pub async fn create_checksum_manifest(directory: String) -> std::result::Result<usize, String> {
    info!("Creating checksum manifest for directory: {}", directory);

    let path = PathBuf::from(&directory);
    let verifier = IntegrityVerifier::new();

    let manifest = verifier
        .create_manifest_from_directory(&path, None::<fn(&VerificationProgress)>)
        .map_err(map_err)?;

    let file_count = manifest.len();
    manifest.save_to_directory(&path).map_err(map_err)?;

    info!("Created checksum manifest with {} files", file_count);
    Ok(file_count)
}

/// Load a checksum manifest from a directory.
#[tauri::command]
pub async fn load_checksum_manifest(
    directory: String,
) -> std::result::Result<ChecksumManifest, String> {
    debug!("Loading checksum manifest from: {}", directory);

    let path = PathBuf::from(&directory);
    ChecksumManifest::load_from_directory(&path).map_err(map_err)
}

/// Check if a checksum manifest exists in a directory.
#[tauri::command]
pub async fn has_checksum_manifest(directory: String) -> std::result::Result<bool, String> {
    let path = PathBuf::from(&directory).join(youtun4_core::integrity::DEFAULT_MANIFEST_FILE);
    Ok(path.exists())
}

/// Verify all files in a directory against a checksum manifest.
#[tauri::command]
pub async fn verify_directory_integrity(
    app: AppHandle,
    directory: String,
    check_extra_files: bool,
) -> std::result::Result<VerificationResult, String> {
    info!("Verifying integrity of directory: {}", directory);

    let path = PathBuf::from(&directory);
    let manifest = ChecksumManifest::load_from_directory(&path).map_err(map_err)?;

    let options = VerificationOptions {
        check_extra_files,
        ..Default::default()
    };

    let verifier = IntegrityVerifier::with_options(options);

    let app_handle = app.clone();
    let progress_callback = move |progress: &VerificationProgress| {
        if let Err(e) = app_handle.emit(integrity_events::VERIFICATION_PROGRESS, progress) {
            error!("Failed to emit verification-progress event: {}", e);
        }
    };

    let result = verifier
        .verify_directory(&path, &manifest, Some(progress_callback))
        .map_err(map_err)?;

    if let Err(e) = app.emit(integrity_events::VERIFICATION_COMPLETED, &result) {
        error!("Failed to emit verification-completed event: {}", e);
    }

    info!(
        "Verification complete: {} passed, {} failed, {} extra files",
        result.passed, result.failed, result.extra_files
    );

    Ok(result)
}

/// Verify a single file against an expected checksum.
#[tauri::command]
pub async fn verify_file_checksum(
    file_path: String,
    expected_checksum: String,
    expected_size: u64,
) -> std::result::Result<bool, String> {
    debug!("Verifying file checksum: {}", file_path);

    let path = PathBuf::from(&file_path);
    let verifier = IntegrityVerifier::new();

    let expected = FileChecksum::new(
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string(),
        expected_checksum,
        expected_size,
    );

    let result = verifier.verify_file(&path, &expected).map_err(map_err)?;
    Ok(result.passed)
}

/// Add or update a file in a checksum manifest.
#[tauri::command]
pub async fn update_manifest_file(
    file_path: String,
    manifest_dir: String,
) -> std::result::Result<(), String> {
    info!("Updating manifest for file: {}", file_path);

    let path = PathBuf::from(&file_path);
    let manifest_path = PathBuf::from(&manifest_dir);

    let mut manifest = match ChecksumManifest::load_from_directory(&manifest_path) {
        Ok(m) => m,
        Err(_) => ChecksumManifest::new(),
    };

    let verifier = IntegrityVerifier::new();
    let checksum = verifier.compute_checksum(&path).map_err(map_err)?;

    let metadata = std::fs::metadata(&path).map_err(|e| {
        map_err(Error::FileSystem(
            youtun4_core::error::FileSystemError::ReadFailed {
                path: path.clone(),
                reason: e.to_string(),
            },
        ))
    })?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let file_checksum = FileChecksum::new(file_name, checksum, metadata.len());
    manifest.add_file(file_checksum);

    manifest
        .save_to_directory(&manifest_path)
        .map_err(map_err)?;

    info!("Manifest updated successfully");
    Ok(())
}

/// Remove a file from a checksum manifest.
#[tauri::command]
pub async fn remove_from_manifest(
    file_name: String,
    manifest_dir: String,
) -> std::result::Result<bool, String> {
    debug!("Removing file from manifest: {}", file_name);

    let manifest_path = PathBuf::from(&manifest_dir);
    let mut manifest = ChecksumManifest::load_from_directory(&manifest_path).map_err(map_err)?;

    let removed = manifest.remove_file(&file_name).is_some();

    manifest
        .save_to_directory(&manifest_path)
        .map_err(map_err)?;

    info!("Removed file '{}' from manifest: {}", file_name, removed);
    Ok(removed)
}

/// Get verification options presets.
#[tauri::command]
pub fn get_default_verification_options() -> VerificationOptions {
    VerificationOptions::default()
}

/// Get strict verification options.
#[tauri::command]
pub const fn get_strict_verification_options() -> VerificationOptions {
    VerificationOptions::strict()
}

/// Get quick verification options.
#[tauri::command]
pub fn get_quick_verification_options() -> VerificationOptions {
    VerificationOptions::quick()
}
