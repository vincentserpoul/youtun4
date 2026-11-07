//! Types for application self-update.

use serde::{Deserialize, Serialize};

/// Metadata about an available application update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Bytes per megabyte, for progress labels.
const BYTES_PER_MB: u64 = 1024 * 1024;

/// Download progress for an update package.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateProgress {
    /// Bytes downloaded so far.
    pub downloaded: u64,
    /// Total bytes, when the server reports a content length.
    pub total: Option<u64>,
}

impl UpdateProgress {
    /// Completion percentage, or `None` while the total size is unknown.
    ///
    /// Integer math throughout: the workspace denies float arithmetic, and a
    /// whole percent is all the progress bar needs.
    #[must_use]
    pub fn percent(&self) -> Option<u64> {
        let total = self.total?;
        if total == 0 {
            return None;
        }
        Some(self.downloaded.saturating_mul(100) / total)
    }

    /// Render the download size as `"12 MB / 34 MB"`, or `"12 MB"` when the
    /// total size is unknown.
    #[must_use]
    pub fn size_label(&self) -> String {
        let downloaded = self.downloaded / BYTES_PER_MB;
        self.total.map_or_else(
            || format!("{downloaded} MB"),
            |total| format!("{downloaded} MB / {} MB", total / BYTES_PER_MB),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_is_none_without_total() {
        let progress = UpdateProgress {
            downloaded: 100,
            total: None,
        };
        assert_eq!(progress.percent(), None);
    }

    #[test]
    fn percent_is_none_for_zero_total() {
        let progress = UpdateProgress {
            downloaded: 0,
            total: Some(0),
        };
        assert_eq!(progress.percent(), None);
    }

    #[test]
    fn percent_rounds_down() {
        let progress = UpdateProgress {
            downloaded: 1,
            total: Some(3),
        };
        assert_eq!(progress.percent(), Some(33));
    }

    #[test]
    fn size_label_without_total() {
        let progress = UpdateProgress {
            downloaded: 12 * BYTES_PER_MB,
            total: None,
        };
        assert_eq!(progress.size_label(), "12 MB");
    }

    #[test]
    fn size_label_with_total() {
        let progress = UpdateProgress {
            downloaded: 12 * BYTES_PER_MB,
            total: Some(34 * BYTES_PER_MB),
        };
        assert_eq!(progress.size_label(), "12 MB / 34 MB");
    }

    #[test]
    fn percent_reaches_hundred() {
        let progress = UpdateProgress {
            downloaded: 2048,
            total: Some(2048),
        };
        assert_eq!(progress.percent(), Some(100));
    }
}
