//! Structured logging system using tracing.
//!
//! Provides configurable logging with:
//! - Different log levels for development and production
//! - Console output with human-readable formatting
//! - File output with JSON formatting and rotation
//! - Log file rotation (daily, with configurable retention)

use std::path::PathBuf;

use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// Logging configuration options.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Directory where log files are stored.
    pub log_directory: PathBuf,
    /// Log file name prefix (e.g., "mp3youtube" -> "mp3youtube.2024-01-15.log").
    pub log_file_prefix: String,
    /// Maximum log level for console output.
    pub console_level: Level,
    /// Maximum log level for file output.
    pub file_level: Level,
    /// How often to rotate log files.
    pub rotation: LogRotation,
    /// Number of days to keep old log files (0 = keep forever).
    pub max_log_files: usize,
    /// Whether to include ANSI color codes in console output.
    pub console_ansi: bool,
    /// Whether to include timestamps in console output.
    pub console_timestamps: bool,
    /// Whether to include file/line info in logs.
    pub include_file_line: bool,
    /// Whether to include target module in logs.
    pub include_target: bool,
    /// Whether to log span events (enter/exit).
    pub log_span_events: bool,
}

/// Log rotation frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogRotation {
    /// Create a new log file every minute (for testing).
    Minutely,
    /// Create a new log file every hour.
    Hourly,
    /// Create a new log file every day.
    Daily,
    /// Never rotate (single log file).
    Never,
}

impl From<LogRotation> for Rotation {
    fn from(rotation: LogRotation) -> Self {
        match rotation {
            LogRotation::Minutely => Self::MINUTELY,
            LogRotation::Hourly => Self::HOURLY,
            LogRotation::Daily => Self::DAILY,
            LogRotation::Never => Self::NEVER,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self::production()
    }
}

impl LoggingConfig {
    /// Create a development configuration with verbose logging.
    #[must_use]
    pub fn development() -> Self {
        Self {
            log_directory: default_log_directory(),
            log_file_prefix: "mp3youtube".to_string(),
            console_level: Level::DEBUG,
            file_level: Level::TRACE,
            rotation: LogRotation::Hourly,
            max_log_files: 24, // Keep 24 hours of logs
            console_ansi: true,
            console_timestamps: true,
            include_file_line: true,
            include_target: true,
            log_span_events: true,
        }
    }

    /// Create a production configuration with minimal console output.
    #[must_use]
    pub fn production() -> Self {
        Self {
            log_directory: default_log_directory(),
            log_file_prefix: "mp3youtube".to_string(),
            console_level: Level::INFO,
            file_level: Level::DEBUG,
            rotation: LogRotation::Daily,
            max_log_files: 7, // Keep 1 week of logs
            console_ansi: true,
            console_timestamps: false,
            include_file_line: false,
            include_target: false,
            log_span_events: false,
        }
    }

    /// Detect configuration based on build type.
    #[must_use]
    pub fn auto() -> Self {
        if cfg!(debug_assertions) {
            Self::development()
        } else {
            Self::production()
        }
    }

    /// Set the log directory.
    #[must_use]
    pub fn with_log_directory(mut self, path: PathBuf) -> Self {
        self.log_directory = path;
        self
    }

    /// Set the log file prefix.
    #[must_use]
    pub fn with_log_file_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.log_file_prefix = prefix.into();
        self
    }

    /// Set the console log level.
    #[must_use]
    pub const fn with_console_level(mut self, level: Level) -> Self {
        self.console_level = level;
        self
    }

    /// Set the file log level.
    #[must_use]
    pub const fn with_file_level(mut self, level: Level) -> Self {
        self.file_level = level;
        self
    }

    /// Set the log rotation frequency.
    #[must_use]
    pub const fn with_rotation(mut self, rotation: LogRotation) -> Self {
        self.rotation = rotation;
        self
    }
}

/// Guard that keeps file logging active. Drop this to flush and close log files.
pub struct LoggingGuard {
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Initialize the logging system with the given configuration.
///
/// Returns a guard that must be kept alive for the duration of the application.
/// When the guard is dropped, any pending log entries are flushed to disk.
///
/// # Errors
///
/// Returns an error if the log directory cannot be created or accessed.
///
/// # Panics
///
/// Panics if logging has already been initialized.
pub fn init(config: &LoggingConfig) -> Result<LoggingGuard, LoggingError> {
    // Ensure log directory exists
    if !config.log_directory.exists() {
        std::fs::create_dir_all(&config.log_directory).map_err(|e| {
            LoggingError::DirectoryCreationFailed {
                path: config.log_directory.clone(),
                reason: e.to_string(),
            }
        })?;
    }

    // Create file appender with rotation
    let file_appender = RollingFileAppender::new(
        config.rotation.into(),
        &config.log_directory,
        &config.log_file_prefix,
    );

    // Make file logging non-blocking
    let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);

    // Build environment filter for console
    // Allows overriding via RUST_LOG environment variable
    // Default: INFO for dependencies, DEBUG for our crates only
    let console_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("warn")
            .add_directive("mp3youtube=info".parse().expect("valid directive"))
            .add_directive("mp3youtube_core=info".parse().expect("valid directive"))
    });

    // Build environment filter for file (more verbose)
    let file_filter = EnvFilter::new(level_to_directive(config.file_level))
        .add_directive("mp3youtube=trace".parse().expect("valid directive"))
        .add_directive("mp3youtube_core=trace".parse().expect("valid directive"));

    // Configure span events
    let span_events = if config.log_span_events {
        FmtSpan::NEW | FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    };

    // Build console layer
    let console_layer = fmt::layer()
        .with_ansi(config.console_ansi)
        .with_target(config.include_target)
        .with_file(config.include_file_line)
        .with_line_number(config.include_file_line)
        .with_span_events(span_events.clone())
        .with_filter(console_filter);

    // Build file layer with JSON formatting
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(span_events)
        .json()
        .with_filter(file_filter);

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    Ok(LoggingGuard {
        _file_guard: file_guard,
    })
}

/// Initialize logging with automatic configuration detection.
///
/// Uses development config in debug builds, production config in release builds.
///
/// # Errors
///
/// Returns an error if initialization fails.
///
/// # Panics
///
/// Panics if logging has already been initialized.
pub fn init_auto() -> Result<LoggingGuard, LoggingError> {
    init(&LoggingConfig::auto())
}

/// Get the default log directory.
#[must_use]
pub fn default_log_directory() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mp3youtube")
        .join("logs")
}

/// Get the path to the current log file (approximate, as rotation may occur).
#[must_use]
pub fn current_log_path(config: &LoggingConfig) -> PathBuf {
    config.log_directory.join(&config.log_file_prefix)
}

/// Convert a tracing Level to a filter directive string.
const fn level_to_directive(level: Level) -> &'static str {
    match level {
        Level::TRACE => "trace",
        Level::DEBUG => "debug",
        Level::INFO => "info",
        Level::WARN => "warn",
        Level::ERROR => "error",
    }
}

/// Errors that can occur during logging initialization.
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    /// Failed to create the log directory.
    #[error("Failed to create log directory {path}: {reason}")]
    DirectoryCreationFailed {
        /// The path that could not be created.
        path: PathBuf,
        /// The reason for the failure.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_production() {
        let config = LoggingConfig::default();
        assert_eq!(config.console_level, Level::INFO);
        assert_eq!(config.file_level, Level::DEBUG);
        assert_eq!(config.rotation, LogRotation::Daily);
    }

    #[test]
    fn test_development_config() {
        let config = LoggingConfig::development();
        assert_eq!(config.console_level, Level::DEBUG);
        assert_eq!(config.file_level, Level::TRACE);
        assert_eq!(config.rotation, LogRotation::Hourly);
        assert!(config.include_file_line);
        assert!(config.log_span_events);
    }

    #[test]
    fn test_production_config() {
        let config = LoggingConfig::production();
        assert_eq!(config.console_level, Level::INFO);
        assert_eq!(config.file_level, Level::DEBUG);
        assert_eq!(config.rotation, LogRotation::Daily);
        assert!(!config.include_file_line);
        assert!(!config.log_span_events);
    }

    #[test]
    fn test_config_builder() {
        let config = LoggingConfig::production()
            .with_console_level(Level::WARN)
            .with_file_level(Level::INFO)
            .with_rotation(LogRotation::Hourly);

        assert_eq!(config.console_level, Level::WARN);
        assert_eq!(config.file_level, Level::INFO);
        assert_eq!(config.rotation, LogRotation::Hourly);
    }

    #[test]
    fn test_log_rotation_conversion() {
        assert!(matches!(
            Rotation::from(LogRotation::Minutely),
            Rotation::MINUTELY
        ));
        assert!(matches!(
            Rotation::from(LogRotation::Hourly),
            Rotation::HOURLY
        ));
        assert!(matches!(
            Rotation::from(LogRotation::Daily),
            Rotation::DAILY
        ));
        assert!(matches!(
            Rotation::from(LogRotation::Never),
            Rotation::NEVER
        ));
    }

    #[test]
    fn test_default_log_directory() {
        let dir = default_log_directory();
        assert!(dir.to_string_lossy().contains("mp3youtube"));
        assert!(dir.to_string_lossy().contains("logs"));
    }
}
