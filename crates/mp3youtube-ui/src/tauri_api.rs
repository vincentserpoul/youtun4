//! Tauri API bindings for WASM.
//!
//! This module provides functions to call Tauri commands from the frontend.

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::types::{DeviceInfo, PlaylistMetadata};

#[wasm_bindgen]
extern "C" {
    /// The global Tauri invoke function (Tauri 2.x API).
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> Result<js_sys::Promise, JsValue>;
}

/// Check if the Tauri API is available.
fn is_tauri_available() -> bool {
    let window = web_sys::window();
    if window.is_none() {
        return false;
    }

    let window = window.expect("window exists");
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"));

    tauri.is_ok() && !tauri.expect("tauri ok").is_undefined()
}

/// Call a Tauri command with the given arguments.
async fn invoke<T: serde::de::DeserializeOwned>(
    cmd: &str,
    args: impl serde::Serialize,
) -> Result<T, String> {
    if !is_tauri_available() {
        return Err("Tauri API not available - are you running in a Tauri app?".to_string());
    }

    let args_value = serde_wasm_bindgen::to_value(&args).map_err(|e| {
        let msg = format!("Failed to serialize args: {e}");
        leptos::logging::error!("{}", msg);
        msg
    })?;

    leptos::logging::log!("Invoking Tauri command: {}", cmd);

    let promise = tauri_invoke(cmd, args_value).map_err(|e| {
        let msg = e
            .as_string()
            .unwrap_or_else(|| "Failed to invoke Tauri command".to_string());
        leptos::logging::error!("Invoke error: {}", msg);
        msg
    })?;

    let result = JsFuture::from(promise).await.map_err(|e| {
        let msg = e
            .as_string()
            .unwrap_or_else(|| "Unknown error from Tauri command".to_string());
        leptos::logging::error!("Promise error: {}", msg);
        msg
    })?;

    leptos::logging::log!("Tauri command {} completed", cmd);

    serde_wasm_bindgen::from_value(result).map_err(|e| {
        let msg = format!("Failed to deserialize result: {e}");
        leptos::logging::error!("{}", msg);
        msg
    })
}

/// List all detected USB devices.
pub async fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("list_devices", Args {}).await
}

/// List all playlists.
pub async fn list_playlists() -> Result<Vec<PlaylistMetadata>, String> {
    #[derive(serde::Serialize)]
    struct Args {}

    invoke("list_playlists", Args {}).await
}

/// Create a new playlist.
pub async fn create_playlist(name: &str, source_url: Option<&str>) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
        source_url: Option<&'a str>,
    }

    invoke("create_playlist", Args { name, source_url }).await
}

/// Delete a playlist.
pub async fn delete_playlist(name: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("delete_playlist", Args { name }).await
}

/// Sync a playlist to a device.
pub async fn sync_playlist(playlist_name: &str, device_mount_point: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        playlist_name: &'a str,
        device_mount_point: &'a str,
    }

    invoke(
        "sync_playlist",
        Args {
            playlist_name,
            device_mount_point,
        },
    )
    .await
}
