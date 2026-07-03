//! Configuration API.

use crate::types::AppConfig;

use super::invoke;

/// Get the current application configuration.
pub async fn get_config() -> Result<AppConfig, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("get_config", Args {}).await
}

/// Update the application configuration.
pub async fn update_config(config: &AppConfig) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        config: &'a AppConfig,
    }

    invoke("update_config", Args { config }).await
}

/// Get the current playlists storage directory.
pub async fn get_storage_directory() -> Result<String, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("get_storage_directory", Args {}).await
}

/// Set the playlists storage directory.
pub async fn set_storage_directory(path: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        path: &'a str,
    }

    invoke("set_storage_directory", Args { path }).await
}

/// Get the default storage directory.
pub async fn get_default_storage_directory() -> Result<String, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("get_default_storage_directory", Args {}).await
}
