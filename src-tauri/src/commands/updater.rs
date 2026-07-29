//! Application self-update commands.
//!
//! Backed by `tauri-plugin-updater`: the app fetches the signed `latest.json`
//! manifest published alongside each GitHub release, verifies it against the
//! public key in `tauri.conf.json`, then downloads and installs the bundle
//! matching the running target.
//!
//! Self-update is a desktop-only feature — mobile builds are updated through
//! the app stores. Rather than dropping the commands from the IPC surface on
//! mobile (which would make the frontend fail with "command not found"), the
//! mobile builds compile stubs that report the feature as unsupported.

use serde::Serialize;
use tauri::AppHandle;

/// Metadata about an available update.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    /// Version offered by the update manifest.
    pub version: String,
    /// Version currently running.
    pub current_version: String,
    /// Release notes, when the manifest provides them.
    pub notes: Option<String>,
    /// Publication date (`YYYY-MM-DD`), when the manifest provides it.
    pub date: Option<String>,
}

#[cfg(desktop)]
mod imp {
    use super::UpdateInfo;

    use serde::Serialize;
    use tauri::AppHandle;
    use tauri_plugin_updater::{Update, UpdaterExt};
    use tracing::info;

    use crate::commands::error::emit_or_log;

    /// Event emitted when the update package starts downloading.
    const EVENT_DOWNLOAD_STARTED: &str = "updater-download-started";
    /// Event emitted as the update package downloads.
    const EVENT_DOWNLOAD_PROGRESS: &str = "updater-download-progress";
    /// Event emitted once the update package has finished downloading.
    const EVENT_DOWNLOAD_FINISHED: &str = "updater-download-finished";

    /// Minimum bytes between progress events.
    ///
    /// The plugin invokes the progress callback once per HTTP chunk, which
    /// would flood the IPC channel with thousands of events for a bundle of
    /// tens of megabytes.
    const PROGRESS_INTERVAL_BYTES: u64 = 256 * 1024;

    /// Download progress for the update package.
    #[derive(Debug, Clone, Copy, Serialize)]
    struct UpdateProgress {
        /// Bytes downloaded so far.
        downloaded: u64,
        /// Total bytes, when the server reports a content length.
        total: Option<u64>,
    }

    /// Ask the configured endpoint whether a newer version is published.
    async fn fetch_update(app: &AppHandle) -> Result<Option<Update>, String> {
        app.updater()
            .map_err(|e| format!("Updater unavailable: {e}"))?
            .check()
            .await
            .map_err(|e| format!("Update check failed: {e}"))
    }

    /// Check whether an update is available.
    pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
        let Some(update) = fetch_update(&app).await? else {
            info!("No update available");
            return Ok(None);
        };

        info!(
            version = %update.version,
            current = %update.current_version,
            "Update available"
        );

        Ok(Some(UpdateInfo {
            version: update.version,
            current_version: update.current_version,
            notes: update.body,
            date: update.date.map(|d| d.date().to_string()),
        }))
    }

    /// Download and install the available update, then restart.
    ///
    /// The endpoint is queried again rather than reusing the result of an
    /// earlier `check_for_update`, so the package that gets installed is
    /// always the one currently published.
    pub async fn install_update(app: AppHandle) -> Result<(), String> {
        let Some(update) = fetch_update(&app).await? else {
            return Err("No update available".to_string());
        };

        info!(version = %update.version, "Installing update");
        emit_or_log(&app, EVENT_DOWNLOAD_STARTED, &update.version);

        let progress_app = app.clone();
        let finish_app = app.clone();
        let mut downloaded: u64 = 0;
        let mut last_emitted: u64 = 0;

        update
            .download_and_install(
                move |chunk_len, total| {
                    // usize -> u64 is lossless on every supported target; the
                    // fallback keeps the cast lint-clean without a panic path.
                    let chunk = u64::try_from(chunk_len).unwrap_or(u64::MAX);
                    downloaded = downloaded.saturating_add(chunk);

                    let is_complete = total == Some(downloaded);
                    if is_complete
                        || downloaded.saturating_sub(last_emitted) >= PROGRESS_INTERVAL_BYTES
                    {
                        last_emitted = downloaded;
                        emit_or_log(
                            &progress_app,
                            EVENT_DOWNLOAD_PROGRESS,
                            &UpdateProgress { downloaded, total },
                        );
                    }
                },
                move || emit_or_log(&finish_app, EVENT_DOWNLOAD_FINISHED, &()),
            )
            .await
            .map_err(|e| format!("Update installation failed: {e}"))?;

        info!("Update installed, restarting");

        // Diverges: the process is replaced. On Windows the NSIS installer
        // already terminated the app before this point.
        app.restart();
    }
}

#[cfg(not(desktop))]
mod imp {
    use super::UpdateInfo;

    use tauri::AppHandle;

    /// Message returned by both commands on platforms without self-update.
    const UNSUPPORTED: &str = "Self-update is not available on this platform; \
        install updates through the app store.";

    /// Stub for platforms where updates are managed by an app store.
    #[allow(
        clippy::unused_async,
        reason = "mirrors the desktop signature so the IPC surface is identical"
    )]
    pub async fn check_for_update(_app: AppHandle) -> Result<Option<UpdateInfo>, String> {
        Err(UNSUPPORTED.to_string())
    }

    /// Stub for platforms where updates are managed by an app store.
    #[allow(
        clippy::unused_async,
        reason = "mirrors the desktop signature so the IPC surface is identical"
    )]
    pub async fn install_update(_app: AppHandle) -> Result<(), String> {
        Err(UNSUPPORTED.to_string())
    }
}

/// Version of the running application.
#[tauri::command]
#[allow(
    clippy::unused_async,
    reason = "Tauri IPC commands in this crate are uniformly async"
)]
pub async fn get_app_version(app: AppHandle) -> Result<String, String> {
    Ok(app.package_info().version.to_string())
}

/// Check whether a newer version is available.
///
/// Returns `None` when the running version is already the published one.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    imp::check_for_update(app).await
}

/// Download and install the available update, then restart the app.
///
/// Progress is reported on the `updater-download-started`,
/// `updater-download-progress` and `updater-download-finished` events.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    imp::install_update(app).await
}
