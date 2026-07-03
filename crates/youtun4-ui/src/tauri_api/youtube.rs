//! `YouTube` URL Validation API and `YouTube` Download API.

use crate::types::{DownloadProgress, DownloadResult, TaskId, YouTubeUrlValidation};

use super::task::cancel_task;
use super::{invoke, listen_to_event};

/// Validate a `YouTube` URL and extract playlist information.
///
/// This function validates whether a given URL is a valid `YouTube` playlist URL
/// and extracts the playlist ID if valid. It supports multiple URL formats:
///
/// - Standard playlist URLs: `https://www.youtube.com/playlist?list=PLxxxxxxxx`
/// - Watch URLs with playlist: `https://www.youtube.com/watch?v=xxx&list=PLxxxxxxxx`
/// - Short URLs with playlist: `https://youtu.be/xxx?list=PLxxxxxxxx`
///
/// Returns a `YouTubeUrlValidation` object containing validation result and details.
pub async fn validate_youtube_playlist_url(url: &str) -> Result<YouTubeUrlValidation, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        url: &'a str,
    }

    invoke("validate_youtube_playlist_url", Args { url }).await
}

/// Check if a URL is a valid `YouTube` playlist URL.
///
/// This is a simpler version that just returns true/false.
pub async fn is_valid_youtube_playlist_url(url: &str) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        url: &'a str,
    }

    invoke("is_valid_youtube_playlist_url", Args { url }).await
}

/// Extract the playlist ID from a `YouTube` URL.
///
/// Returns the playlist ID if the URL is valid, or an error message if not.
pub async fn extract_youtube_playlist_id(url: &str) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        url: &'a str,
    }

    invoke("extract_youtube_playlist_id", Args { url }).await
}

/// Event names for `YouTube` download events.
pub mod youtube_events {
    /// Event emitted when a download starts.
    pub const DOWNLOAD_STARTED: &str = "youtube-download-started";
    /// Event emitted for download progress updates.
    pub const DOWNLOAD_PROGRESS: &str = "youtube-download-progress";
    /// Event emitted when a download completes successfully.
    pub const DOWNLOAD_COMPLETED: &str = "youtube-download-completed";
    /// Event emitted when a download fails.
    pub const DOWNLOAD_FAILED: &str = "youtube-download-failed";
    /// Event emitted when a download is cancelled.
    pub const DOWNLOAD_CANCELLED: &str = "youtube-download-cancelled";
}

/// Check if yt-dlp is available on the system.
///
/// Returns the version string if yt-dlp is found, or an error if not.
pub async fn check_yt_dlp_available() -> Result<String, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("check_yt_dlp_available", Args {}).await
}

/// Download a `YouTube` playlist to a local directory.
///
/// Returns the task ID that can be used to track the download.
pub async fn download_youtube_playlist(
    url: &str,
    output_dir: &str,
    audio_quality: Option<&str>,
    embed_thumbnail: Option<bool>,
) -> Result<TaskId, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        url: &'a str,
        output_dir: &'a str,
        audio_quality: Option<&'a str>,
        embed_thumbnail: Option<bool>,
    }

    invoke(
        "download_youtube_playlist",
        Args {
            url,
            output_dir,
            audio_quality,
            embed_thumbnail,
        },
    )
    .await
}

/// Download a `YouTube` playlist directly to a local playlist folder.
///
/// Returns the task ID that can be used to track the download.
pub async fn download_youtube_to_playlist(
    url: &str,
    playlist_name: &str,
) -> Result<TaskId, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        url: &'a str,
        playlist_name: &'a str,
    }

    invoke("download_youtube_to_playlist", Args { url, playlist_name }).await
}

/// Cancel a running download task.
///
/// Returns `true` if the task was successfully cancelled.
pub async fn cancel_download(task_id: TaskId) -> Result<bool, String> {
    cancel_task(task_id).await
}

/// Listen to `YouTube` download started events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_started<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(TaskId) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_STARTED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(task_id) = serde_wasm_bindgen::from_value::<TaskId>(payload)
        {
            handler(task_id);
        }
    })
    .await
}

/// Listen to `YouTube` download progress events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_progress<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DownloadProgress) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_PROGRESS, move |value| {
        // The payload is wrapped in an event object with a "payload" field
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(progress) = serde_wasm_bindgen::from_value::<DownloadProgress>(payload)
        {
            handler(progress);
        }
    })
    .await
}

/// Listen to `YouTube` download completed events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_completed<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DownloadResult) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_COMPLETED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<DownloadResult>(payload)
        {
            handler(result);
        }
    })
    .await
}

/// Listen to `YouTube` download failed events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_failed<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DownloadResult) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_FAILED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<DownloadResult>(payload)
        {
            handler(result);
        }
    })
    .await
}

/// Listen to `YouTube` download cancelled events.
///
/// Returns a function to stop listening.
pub async fn listen_to_download_cancelled<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(DownloadResult) + 'static,
{
    listen_to_event(youtube_events::DOWNLOAD_CANCELLED, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(result) = serde_wasm_bindgen::from_value::<DownloadResult>(payload)
        {
            handler(result);
        }
    })
    .await
}
