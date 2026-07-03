use serde::{Deserialize, Serialize};

// =============================================================================
// Notification Types
// =============================================================================

/// Type of notification to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NotificationType {
    /// Informational message.
    #[default]
    Info,
    /// Success message.
    Success,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

impl std::fmt::Display for NotificationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Success => write!(f, "success"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A toast notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Unique identifier for this notification.
    pub id: u64,
    /// The notification type.
    pub notification_type: NotificationType,
    /// The main message to display.
    pub message: String,
    /// Optional title/heading for the notification.
    pub title: Option<String>,
    /// Duration in milliseconds before auto-dismiss (None = manual dismiss only).
    pub duration_ms: Option<u64>,
}

impl Notification {
    /// Create a new notification with a unique ID.
    #[must_use]
    pub fn new(notification_type: NotificationType, message: impl Into<String>) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);

        Self {
            id: COUNTER.fetch_add(1, Ordering::Relaxed),
            notification_type,
            message: message.into(),
            title: None,
            duration_ms: Some(5000), // Default 5 seconds
        }
    }

    /// Create an info notification.
    #[must_use]
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(NotificationType::Info, message)
    }

    /// Create a success notification.
    #[must_use]
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(NotificationType::Success, message)
    }

    /// Create a warning notification.
    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(NotificationType::Warning, message)
    }

    /// Create an error notification.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        let mut notification = Self::new(NotificationType::Error, message);
        notification.duration_ms = Some(8000); // Errors stay longer
        notification
    }

    /// Set the title for this notification.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the duration for this notification.
    #[must_use]
    pub const fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Make this notification persist until manually dismissed.
    #[must_use]
    pub const fn persistent(mut self) -> Self {
        self.duration_ms = None;
        self
    }
}
