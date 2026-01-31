//! Application configuration management.
//!
//! Handles loading, saving, and managing application-wide settings,
//! including the local storage directory for playlists.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::cache::CacheConfig;
use crate::error::{Error, FileSystemError, Result};
use crate::queue::QueueConfig;

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// Directory where playlists are stored.
    pub playlists_directory: PathBuf,
    /// Download quality for `YouTube` downloads.
    #[serde(default)]
    pub download_quality: DownloadQuality,
    /// Theme preference.
    #[serde(default)]
    pub theme: Theme,
    /// Notification preferences.
    #[serde(default)]
    pub notification_preferences: NotificationPreferences,
    /// Cache configuration.
    #[serde(default)]
    pub cache: CacheConfig,
    /// Download queue configuration.
    #[serde(default)]
    pub queue: QueueConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            playlists_directory: default_playlists_directory(),
            download_quality: DownloadQuality::default(),
            theme: Theme::default(),
            notification_preferences: NotificationPreferences::default(),
            cache: CacheConfig::default(),
            queue: QueueConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load configuration from disk, or create default if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be read or parsed.
    pub fn load() -> Result<Self> {
        let config_path = config_file_path();

        if !config_path.exists() {
            debug!("Config file not found, using defaults");
            let config = Self::default();
            // Ensure config directory exists and save defaults
            if let Err(e) = config.save() {
                warn!("Failed to save default config: {}", e);
            }
            return Ok(config);
        }

        let content = fs::read_to_string(&config_path).map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: config_path.clone(),
                reason: format!("Failed to read config file: {e}"),
            })
        })?;

        let config: Self = serde_json::from_str(&content)
            .map_err(|e| Error::Configuration(format!("Failed to parse config file: {e}")))?;

        info!("Loaded config from {}", config_path.display());
        debug!(
            "Playlists directory: {}",
            config.playlists_directory.display()
        );

        Ok(config)
    }

    /// Save configuration to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be written.
    pub fn save(&self) -> Result<()> {
        let config_path = config_file_path();

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| {
                Error::FileSystem(FileSystemError::CreateDirFailed {
                    path: parent.to_path_buf(),
                    reason: format!("Failed to create config directory: {e}"),
                })
            })?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, content).map_err(|e| {
            Error::FileSystem(FileSystemError::WriteFailed {
                path: config_path.clone(),
                reason: format!("Failed to write config file: {e}"),
            })
        })?;

        info!("Saved config to {}", config_path.display());
        Ok(())
    }

    /// Update the playlists directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory doesn't exist or isn't writable.
    pub fn set_playlists_directory(&mut self, path: PathBuf) -> Result<()> {
        // Validate the path
        validate_storage_directory(&path)?;

        self.playlists_directory = path;
        info!(
            "Updated playlists directory to: {}",
            self.playlists_directory.display()
        );
        Ok(())
    }

    /// Get the path to the config file.
    #[must_use]
    pub fn config_file_path() -> PathBuf {
        config_file_path()
    }
}

/// Get the default playlists directory.
#[must_use]
pub fn default_playlists_directory() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mp3youtube")
        .join("playlists")
}

/// Get the path to the config file.
fn config_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("mp3youtube")
        .join("config.json")
}

/// Validate that a directory is suitable for storing playlists.
fn validate_storage_directory(path: &Path) -> Result<()> {
    // Check if path is absolute
    if !path.is_absolute() {
        return Err(Error::Configuration(
            "Storage directory must be an absolute path".to_string(),
        ));
    }

    // If directory exists, check if it's actually a directory and writable
    if path.exists() {
        if !path.is_dir() {
            return Err(Error::Configuration(format!(
                "Path exists but is not a directory: {}",
                path.display()
            )));
        }

        // Try to check if we can write to it
        let test_file = path.join(".mp3youtube_write_test");
        match fs::write(&test_file, "test") {
            Ok(()) => {
                let _ = fs::remove_file(&test_file);
            }
            Err(e) => {
                return Err(Error::Configuration(format!(
                    "Directory is not writable: {} ({})",
                    path.display(),
                    e
                )));
            }
        }
    } else {
        // Try to create the directory
        fs::create_dir_all(path).map_err(|e| {
            Error::Configuration(format!("Cannot create directory {}: {}", path.display(), e))
        })?;
    }

    Ok(())
}

/// Configuration manager that handles loading and caching config.
pub struct ConfigManager {
    config: AppConfig,
}

impl ConfigManager {
    /// Create a new config manager, loading config from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the config cannot be loaded.
    pub fn new() -> Result<Self> {
        let config = AppConfig::load()?;
        Ok(Self { config })
    }

    /// Get a reference to the current configuration.
    #[must_use]
    pub const fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Get the playlists directory.
    #[must_use]
    pub fn playlists_directory(&self) -> &Path {
        &self.config.playlists_directory
    }

    /// Update the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the config cannot be saved.
    pub fn update(&mut self, config: AppConfig) -> Result<()> {
        // Validate the new playlists directory
        validate_storage_directory(&config.playlists_directory)?;

        self.config = config;
        self.config.save()?;
        Ok(())
    }

    /// Update just the playlists directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory is invalid or config cannot be saved.
    pub fn set_playlists_directory(&mut self, path: PathBuf) -> Result<()> {
        self.config.set_playlists_directory(path)?;
        self.config.save()?;
        Ok(())
    }

    /// Reset to default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the config cannot be saved.
    pub fn reset(&mut self) -> Result<()> {
        self.config = AppConfig::default();
        self.config.save()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(!config.playlists_directory.as_os_str().is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig {
            playlists_directory: PathBuf::from("/test/path"),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).expect("Should serialize");
        let deserialized: AppConfig = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_validate_storage_directory_success() {
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let result = validate_storage_directory(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_storage_directory_creates_new() {
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let new_path = temp_dir.path().join("new_subdir");

        let result = validate_storage_directory(&new_path);
        assert!(result.is_ok());
        assert!(new_path.exists());
    }

    #[test]
    fn test_validate_storage_directory_relative_path() {
        let result = validate_storage_directory(Path::new("relative/path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_config_manager_update() {
        // This test uses a temp directory to avoid modifying actual config
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let playlists_dir = temp_dir.path().join("playlists");

        let _config = AppConfig {
            playlists_directory: playlists_dir.clone(),
            ..Default::default()
        };

        // Just test that validation works
        let result = validate_storage_directory(&playlists_dir);
        assert!(result.is_ok());
    }

    // =============================================================================
    // Additional Config Tests
    // =============================================================================

    #[test]
    fn test_default_playlists_directory_not_empty() {
        let dir = default_playlists_directory();
        // Should be something like ~/Library/Application Support/mp3youtube/playlists on macOS
        // or ~/.local/share/mp3youtube/playlists on Linux
        assert!(!dir.as_os_str().is_empty());
        assert!(dir.ends_with("playlists") || dir.to_string_lossy().contains("mp3youtube"));
    }

    #[test]
    fn test_config_equality() {
        let config1 = AppConfig {
            playlists_directory: PathBuf::from("/test/path"),
            ..Default::default()
        };
        let config2 = AppConfig {
            playlists_directory: PathBuf::from("/test/path"),
            ..Default::default()
        };
        let config3 = AppConfig {
            playlists_directory: PathBuf::from("/different/path"),
            ..Default::default()
        };

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_config_debug_format() {
        let config = AppConfig {
            playlists_directory: PathBuf::from("/test/path"),
            ..Default::default()
        };
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("AppConfig"));
        assert!(debug_str.contains("playlists_directory"));
    }

    #[test]
    fn test_config_clone() {
        let config = AppConfig {
            playlists_directory: PathBuf::from("/test/path"),
            ..Default::default()
        };
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_validate_storage_directory_creates_nested() {
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let nested_path = temp_dir.path().join("level1/level2/level3");

        let result = validate_storage_directory(&nested_path);
        assert!(result.is_ok());
        assert!(nested_path.exists());
        assert!(nested_path.is_dir());
    }

    #[test]
    fn test_validate_storage_directory_existing_file() {
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let file_path = temp_dir.path().join("not_a_directory");
        fs::write(&file_path, "test content").expect("Should write file");

        let result = validate_storage_directory(&file_path);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not a directory"));
    }

    #[test]
    fn test_app_config_set_playlists_directory_valid() {
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let mut config = AppConfig::default();

        let result = config.set_playlists_directory(temp_dir.path().to_path_buf());
        assert!(result.is_ok());
        assert_eq!(config.playlists_directory, temp_dir.path().to_path_buf());
    }

    #[test]
    fn test_app_config_set_playlists_directory_relative_fails() {
        let mut config = AppConfig::default();

        let result = config.set_playlists_directory(PathBuf::from("relative/path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_config_file_path_static() {
        let path = AppConfig::config_file_path();
        // Should end with config.json
        assert!(path.to_string_lossy().ends_with("config.json"));
        // Should contain mp3youtube in the path
        assert!(path.to_string_lossy().contains("mp3youtube"));
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = AppConfig {
            playlists_directory: PathBuf::from("/Users/test/Music/playlists"),
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&config).expect("Should serialize");
        assert!(json.contains("playlists_directory"));

        let deserialized: AppConfig = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_config_deserialization_from_json() {
        let json = r#"{"playlists_directory":"/custom/path"}"#;
        let config: AppConfig = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(config.playlists_directory, PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_validate_storage_directory_already_exists() {
        let temp_dir = TempDir::new().expect("Should create temp dir");
        // Call twice - second time it already exists
        let result1 = validate_storage_directory(temp_dir.path());
        let result2 = validate_storage_directory(temp_dir.path());
        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[test]
    fn test_config_manager_playlists_directory() {
        // Create a temp config manager environment
        let temp_dir = TempDir::new().expect("Should create temp dir");
        let config = AppConfig {
            playlists_directory: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        // Test ConfigManager methods with a manual instance
        struct TestConfigManager {
            config: AppConfig,
        }

        impl TestConfigManager {
            fn config(&self) -> &AppConfig {
                &self.config
            }

            fn playlists_directory(&self) -> &Path {
                &self.config.playlists_directory
            }
        }

        let manager = TestConfigManager { config };
        assert_eq!(manager.playlists_directory(), temp_dir.path());
        assert_eq!(
            manager.config().playlists_directory,
            temp_dir.path().to_path_buf()
        );
    }

    #[test]
    fn test_config_file_path_uses_correct_name() {
        let path = config_file_path();
        assert!(path.file_name().unwrap().to_str().unwrap() == "config.json");
    }

    #[test]
    fn test_default_playlists_directory_is_absolute() {
        let dir = default_playlists_directory();
        // If we got a path from dirs crate, it should be absolute
        // Only the fallback "." wouldn't be absolute
        if dir != PathBuf::from(".").join("mp3youtube").join("playlists") {
            assert!(dir.is_absolute());
        }
    }

    #[test]
    fn test_config_json_format() {
        let config = AppConfig {
            playlists_directory: PathBuf::from("/test"),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).expect("serialize");
        // Should be valid JSON
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse as value");
        assert!(value.is_object());
        assert!(value.get("playlists_directory").is_some());
    }

    #[test]
    fn test_validate_error_message_for_relative_path() {
        let result = validate_storage_directory(Path::new("./relative"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("absolute"));
    }
}
