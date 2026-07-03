//! Tauri API bindings for WASM.
//!
//! This module provides functions to call Tauri commands from the frontend.

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

mod config;
mod device;
mod playlist;
mod sync;
mod task;
mod transfer;
mod youtube;

pub use config::*;
pub use device::*;
pub use playlist::*;
pub use sync::*;
pub use task::*;
pub use transfer::*;
pub use youtube::*;

pub use device::device_events;
pub use sync::{sync_events, sync_orchestrator_events};
pub use youtube::youtube_events;

#[wasm_bindgen]
extern "C" {
    /// The global Tauri invoke function (Tauri 2.x API).
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> Result<js_sys::Promise, JsValue>;

    /// Listen to Tauri events (Tauri 2.x API).
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    fn tauri_listen(
        event: &str,
        handler: &Closure<dyn Fn(JsValue)>,
    ) -> Result<js_sys::Promise, JsValue>;
}

/// A handle for an event listener that can be used to unlisten.
#[wasm_bindgen]
extern "C" {
    /// Unlisten function returned by `tauri_listen`.
    pub type UnlistenFn;

    #[wasm_bindgen(method, structural, js_name = "call")]
    fn call(this: &UnlistenFn);
}

/// Listen to a Tauri event.
///
/// Returns a closure that can be called to stop listening.
pub async fn listen_to_event<F>(event: &str, handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(JsValue) + 'static,
{
    if !is_tauri_available() {
        return Err("Tauri API not available".to_string());
    }

    let closure = Closure::new(handler);
    let promise = tauri_listen(event, &closure).map_err(|e| {
        e.as_string()
            .unwrap_or_else(|| "Failed to listen to event".to_string())
    })?;

    // Keep the closure alive
    closure.forget();

    let unlisten = JsFuture::from(promise).await.map_err(|e| {
        e.as_string()
            .unwrap_or_else(|| "Failed to set up event listener".to_string())
    })?;

    Ok(unlisten.unchecked_into())
}

/// Check if the Tauri API is available.
#[allow(
    clippy::expect_used,
    reason = "both expect() calls are guarded immediately above by an is_none()/is_ok() check on the same value, so the panic path is unreachable"
)]
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
        let msg = "Tauri API not available - are you running in a Tauri app?";
        leptos::logging::error!("{}", msg);
        return Err(msg.to_string());
    }

    let args_value = serde_wasm_bindgen::to_value(&args).map_err(|e| {
        let msg = format!("Failed to serialize args: {e}");
        leptos::logging::error!("{}", msg);
        msg
    })?;

    leptos::logging::log!("=== INVOKE {} START ===", cmd);

    let promise = tauri_invoke(cmd, args_value).map_err(|e| {
        let msg = e
            .as_string()
            .unwrap_or_else(|| "Failed to invoke Tauri command".to_string());
        leptos::logging::error!("=== INVOKE {} FAILED (tauri_invoke): {} ===", cmd, msg);
        msg
    })?;

    let result = JsFuture::from(promise).await.map_err(|e| {
        let msg = e
            .as_string()
            .unwrap_or_else(|| "Unknown error from Tauri command".to_string());
        leptos::logging::error!("=== INVOKE {} FAILED (promise): {} ===", cmd, msg);
        msg
    })?;

    leptos::logging::log!("=== INVOKE {} GOT RESULT ===", cmd);

    // Log the raw result for debugging
    if let Some(json_str) = js_sys::JSON::stringify(&result)
        .ok()
        .and_then(|s| s.as_string())
    {
        leptos::logging::log!("=== INVOKE {} RAW RESULT: {} ===", cmd, json_str);
    }

    let deserialized: T = serde_wasm_bindgen::from_value(result).map_err(|e| {
        let msg = format!("Failed to deserialize result: {e}");
        leptos::logging::error!("=== INVOKE {} FAILED (deserialize): {} ===", cmd, msg);
        msg
    })?;

    leptos::logging::log!("=== INVOKE {} SUCCESS ===", cmd);
    Ok(deserialized)
}
