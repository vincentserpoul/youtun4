//! Application self-update API.

use crate::types::{UpdateInfo, UpdateProgress};

use super::{invoke, listen_to_event};

/// Event names for self-update events.
pub mod updater_events {
    /// Event emitted when the update package starts downloading.
    pub const DOWNLOAD_STARTED: &str = "updater-download-started";
    /// Event emitted as the update package downloads.
    pub const DOWNLOAD_PROGRESS: &str = "updater-download-progress";
    /// Event emitted once the update package has finished downloading.
    pub const DOWNLOAD_FINISHED: &str = "updater-download-finished";
}

/// Get the version of the running application.
pub async fn get_app_version() -> Result<String, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("get_app_version", Args {}).await
}

/// Check whether a newer version is available.
///
/// Returns `None` when the running version is already the published one.
pub async fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("check_for_update", Args {}).await
}

/// Download and install the available update, then restart the app.
///
/// On success the process is replaced, so this future normally never
/// resolves — treat a returned `Ok` as "the restart is imminent".
pub async fn install_update() -> Result<(), String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("install_update", Args {}).await
}

/// Listen to update download progress events.
///
/// Returns a function to stop listening.
pub async fn listen_to_update_progress<F>(handler: F) -> Result<js_sys::Function, String>
where
    F: Fn(UpdateProgress) + 'static,
{
    listen_to_event(updater_events::DOWNLOAD_PROGRESS, move |value| {
        if let Ok(payload) =
            js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("payload"))
            && let Ok(progress) = serde_wasm_bindgen::from_value::<UpdateProgress>(payload)
        {
            handler(progress);
        }
    })
    .await
}
