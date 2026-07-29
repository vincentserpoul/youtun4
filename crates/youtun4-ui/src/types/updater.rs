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
    fn percent_reaches_hundred() {
        let progress = UpdateProgress {
            downloaded: 2048,
            total: Some(2048),
        };
        assert_eq!(progress.percent(), Some(100));
    }
}
