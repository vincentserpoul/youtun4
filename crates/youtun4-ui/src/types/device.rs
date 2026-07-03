use serde::{Deserialize, Serialize};

use crate::utils::format_bytes;

/// Information about a detected device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Device name/identifier.
    pub name: String,
    /// Mount point path as string.
    pub mount_point: String,
    /// Total capacity in bytes.
    pub total_bytes: u64,
    /// Available space in bytes.
    pub available_bytes: u64,
    /// File system type (e.g., FAT32, exFAT).
    pub file_system: String,
    /// Whether the device is removable.
    pub is_removable: bool,
}

impl DeviceInfo {
    /// Returns the used space in bytes.
    #[must_use]
    pub const fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }

    /// Returns the usage percentage (0.0 - 100.0).
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for display formatting"
    )]
    pub fn usage_percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes() as f64 / self.total_bytes as f64) * 100.0
    }
}

// =============================================================================
// Capacity Check Types
// =============================================================================

/// Warning level for capacity checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CapacityWarningLevel {
    /// Sufficient space available.
    #[default]
    Ok,
    /// Space is limited (usage will be high after sync).
    Warning,
    /// Insufficient space for sync.
    Critical,
}

impl std::fmt::Display for CapacityWarningLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "Ok"),
            Self::Warning => write!(f, "Warning"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

/// Result of checking device capacity for a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityCheckResult {
    /// Whether the playlist(s) can fit on the device.
    pub can_fit: bool,
    /// Total bytes required for the sync operation.
    pub required_bytes: u64,
    /// Available bytes on the device.
    pub available_bytes: u64,
    /// Total device capacity in bytes.
    pub total_bytes: u64,
    /// Device usage percentage after sync (0.0 - 100.0).
    pub usage_after_sync_percent: f64,
    /// Warning level based on available space.
    pub warning_level: CapacityWarningLevel,
    /// Human-readable message about the capacity status.
    pub message: String,
}

impl CapacityCheckResult {
    /// Format required bytes as a human-readable string.
    #[must_use]
    pub fn formatted_required(&self) -> String {
        format_bytes(self.required_bytes)
    }

    /// Format available bytes as a human-readable string.
    #[must_use]
    pub fn formatted_available(&self) -> String {
        format_bytes(self.available_bytes)
    }

    /// Get the remaining bytes after sync (can be negative if insufficient).
    #[must_use]
    pub const fn remaining_after_sync(&self) -> i64 {
        self.available_bytes as i64 - self.required_bytes as i64
    }

    /// Format remaining bytes after sync as a human-readable string.
    #[must_use]
    #[allow(clippy::cast_sign_loss, reason = "sign is checked before cast")]
    pub fn formatted_remaining(&self) -> String {
        let remaining = self.remaining_after_sync();
        if remaining >= 0 {
            format_bytes(remaining as u64)
        } else {
            format!("-{}", format_bytes((-remaining) as u64))
        }
    }
}
