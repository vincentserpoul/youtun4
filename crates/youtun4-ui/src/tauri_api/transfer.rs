//! File Transfer API.

use crate::types::{TransferOptions, TransferProgress, TransferResult};

use super::{invoke, listen_to_event};

/// Event name for transfer progress updates.
pub const TRANSFER_PROGRESS_EVENT: &str = "transfer-progress";

/// Sync a playlist to a device with progress tracking.
///
/// This enhanced sync operation provides:
/// - Chunked file transfers for better performance
/// - Progress callbacks via Tauri events
/// - Optional integrity verification
/// - Detailed transfer statistics
///
/// Subscribe to "transfer-progress" events to receive progress updates.
pub async fn sync_playlist_with_progress(
    playlist_name: &str,
    device_mount_point: &str,
    verify_integrity: bool,
    skip_existing: bool,
) -> Result<TransferResult, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        playlist_name: &'a str,
        device_mount_point: &'a str,
        verify_integrity: bool,
        skip_existing: bool,
    }

    invoke(
        "sync_playlist_with_progress",
        Args {
            playlist_name,
            device_mount_point,
            verify_integrity,
            skip_existing,
        },
    )
    .await
}

/// Get default transfer options.
pub async fn get_default_transfer_options() -> Result<TransferOptions, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("get_default_transfer_options", Args {}).await
}

/// Get fast transfer options (optimized for speed, no verification).
pub async fn get_fast_transfer_options() -> Result<TransferOptions, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("get_fast_transfer_options", Args {}).await
}

/// Get reliable transfer options (full integrity verification).
pub async fn get_reliable_transfer_options() -> Result<TransferOptions, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("get_reliable_transfer_options", Args {}).await
}

/// Transfer specific files to a device.
///
/// Subscribe to "transfer-progress" events to receive progress updates.
pub async fn transfer_files_to_device(
    source_files: Vec<String>,
    device_mount_point: &str,
    options: &TransferOptions,
) -> Result<TransferResult, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        source_files: Vec<String>,
        device_mount_point: &'a str,
        options: &'a TransferOptions,
    }

    invoke(
        "transfer_files_to_device",
        Args {
            source_files,
            device_mount_point,
            options,
        },
    )
    .await
}

/// Compute the SHA-256 checksum of a file.
pub async fn compute_file_checksum(file_path: &str) -> Result<String, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        file_path: &'a str,
    }

    invoke("compute_file_checksum", Args { file_path }).await
}

/// Verify integrity of a transferred file by comparing checksums.
///
/// Returns `true` if source and destination have matching checksums.
pub async fn verify_file_integrity(
    source_path: &str,
    destination_path: &str,
) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        source_path: &'a str,
        destination_path: &'a str,
    }

    invoke(
        "verify_file_integrity",
        Args {
            source_path,
            destination_path,
        },
    )
    .await
}

/// Listen to transfer progress events.
///
/// Returns a function to stop listening.
pub async fn listen_to_transfer_progress<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(TransferProgress) + 'static,
{
    listen_to_event(TRANSFER_PROGRESS_EVENT, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(progress) = serde_wasm_bindgen::from_value::<TransferProgress>(payload)
        {
            handler(progress);
        }
    })
    .await
}
