use serde::{Deserialize, Serialize};

/// Download quality setting for `YouTube` downloads.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DownloadQuality {
    /// Low quality (128 kbps).
    Low,
    /// Medium quality (192 kbps).
    #[default]
    Medium,
    /// High quality (320 kbps or best available).
    High,
}

impl std::fmt::Display for DownloadQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low (128 kbps)"),
            Self::Medium => write!(f, "Medium (192 kbps)"),
            Self::High => write!(f, "High (320 kbps)"),
        }
    }
}

/// Theme setting for the application.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Dark theme (default).
    #[default]
    Dark,
    /// Light theme.
    Light,
    /// Follow system preference.
    System,
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dark => write!(f, "Dark"),
            Self::Light => write!(f, "Light"),
            Self::System => write!(f, "System"),
        }
    }
}

/// Notification preferences for the application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool represents an independent notification toggle"
)]
pub struct NotificationPreferences {
    /// Show notifications for download completion.
    #[serde(default = "default_true")]
    pub download_complete: bool,
    /// Show notifications for sync completion.
    #[serde(default = "default_true")]
    pub sync_complete: bool,
    /// Show notifications for errors.
    #[serde(default = "default_true")]
    pub errors: bool,
    /// Show notifications for device connections.
    #[serde(default = "default_true")]
    pub device_connected: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            download_complete: true,
            sync_complete: true,
            errors: true,
            device_connected: true,
        }
    }
}

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    /// Directory where playlists are stored.
    pub playlists_directory: String,
    /// Download quality for `YouTube` downloads.
    #[serde(default)]
    pub download_quality: DownloadQuality,
    /// Theme preference.
    #[serde(default)]
    pub theme: Theme,
    /// Notification preferences.
    #[serde(default)]
    pub notification_preferences: NotificationPreferences,
}
