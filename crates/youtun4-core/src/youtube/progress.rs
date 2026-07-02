use super::model::{DownloadProgress, DownloadStatus};

/// Progress tracker for monitoring download operations.
///
/// This struct tracks download statistics and calculates metrics like
/// speed and estimated time remaining.
#[derive(Debug)]
pub struct DownloadProgressTracker {
    /// Start time of the download operation.
    start_time: std::time::Instant,
    /// Total number of videos to download.
    pub total_videos: usize,
    /// Number of videos completed.
    pub videos_completed: usize,
    /// Number of videos skipped.
    pub videos_skipped: usize,
    /// Number of videos failed.
    pub videos_failed: usize,
    /// Total bytes downloaded across all files.
    pub total_bytes_downloaded: u64,
    /// Recent download samples for speed calculation (timestamp, bytes).
    speed_samples: Vec<(std::time::Instant, u64)>,
    /// Maximum number of samples to keep for speed averaging.
    max_samples: usize,
}

impl Default for DownloadProgressTracker {
    fn default() -> Self {
        Self::new(0)
    }
}

impl DownloadProgressTracker {
    /// Create a new progress tracker.
    #[must_use]
    pub fn new(total_videos: usize) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            total_videos,
            videos_completed: 0,
            videos_skipped: 0,
            videos_failed: 0,
            total_bytes_downloaded: 0,
            speed_samples: Vec::with_capacity(10),
            max_samples: 10,
        }
    }

    /// Record a progress update with current bytes downloaded.
    pub fn record_progress(&mut self, bytes_downloaded: u64) {
        let now = std::time::Instant::now();
        self.total_bytes_downloaded = bytes_downloaded;
        self.speed_samples.push((now, bytes_downloaded));

        // Keep only the most recent samples
        if self.speed_samples.len() > self.max_samples {
            self.speed_samples.remove(0);
        }
    }

    /// Mark a video as completed.
    pub const fn video_completed(&mut self) {
        self.videos_completed += 1;
    }

    /// Mark a video as skipped.
    pub const fn video_skipped(&mut self) {
        self.videos_skipped += 1;
    }

    /// Mark a video as failed.
    pub const fn video_failed(&mut self) {
        self.videos_failed += 1;
    }

    /// Get elapsed time in seconds.
    #[must_use]
    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Calculate current download speed in bytes per second.
    ///
    /// Uses a sliding window average for smoother speed estimates.
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for speed calculation"
    )]
    pub fn download_speed_bps(&self) -> f64 {
        if self.speed_samples.len() < 2 {
            // Not enough samples, calculate from total
            let elapsed = self.elapsed_secs();
            if elapsed > 0.0 {
                return self.total_bytes_downloaded as f64 / elapsed;
            }
            return 0.0;
        }

        // Calculate speed from the sliding window
        let Some(first) = self.speed_samples.first() else {
            return 0.0;
        };
        let Some(last) = self.speed_samples.last() else {
            return 0.0;
        };

        let time_diff = last.0.duration_since(first.0).as_secs_f64();
        let bytes_diff = last.1.saturating_sub(first.1);

        if time_diff > 0.0 {
            bytes_diff as f64 / time_diff
        } else {
            0.0
        }
    }

    /// Estimate remaining time in seconds based on current progress.
    ///
    /// Returns `None` if there's not enough data to estimate.
    #[must_use]
    #[allow(clippy::float_arithmetic, reason = "acceptable for time estimation")]
    pub fn estimated_remaining_secs(&self, current_progress: f64) -> Option<f64> {
        if current_progress <= 0.0 || current_progress >= 1.0 {
            return None;
        }

        let elapsed = self.elapsed_secs();
        if elapsed < 1.0 {
            return None; // Wait for at least 1 second of data
        }

        // Estimate total time based on current progress
        let total_estimated = elapsed / current_progress;
        let remaining = total_estimated - elapsed;

        (remaining > 0.0).then_some(remaining)
    }

    /// Create a `DownloadProgress` snapshot with current statistics.
    #[must_use]
    #[allow(
        clippy::float_arithmetic,
        clippy::cast_precision_loss,
        reason = "acceptable for progress calculation"
    )]
    pub fn create_progress(
        &self,
        current_index: usize,
        current_title: &str,
        current_progress: f64,
        status: DownloadStatus,
        current_bytes: u64,
        current_total_bytes: Option<u64>,
    ) -> DownloadProgress {
        let overall_progress = if self.total_videos > 0 {
            (current_index.saturating_sub(1) as f64 + current_progress) / self.total_videos as f64
        } else {
            0.0
        };

        DownloadProgress {
            current_index,
            total_videos: self.total_videos,
            current_title: current_title.to_string(),
            current_progress,
            overall_progress,
            status,
            current_bytes,
            current_total_bytes,
            total_bytes_downloaded: self.total_bytes_downloaded,
            download_speed_bps: self.download_speed_bps(),
            estimated_remaining_secs: self.estimated_remaining_secs(overall_progress),
            elapsed_secs: self.elapsed_secs(),
            videos_completed: self.videos_completed,
            videos_skipped: self.videos_skipped,
            videos_failed: self.videos_failed,
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;

    mod progress_tracker_tests {
        use super::*;

        #[test]
        fn test_tracker_creation() {
            let tracker = DownloadProgressTracker::new(10);

            assert_eq!(tracker.total_videos, 10);
            assert_eq!(tracker.videos_completed, 0);
            assert_eq!(tracker.videos_skipped, 0);
            assert_eq!(tracker.videos_failed, 0);
            assert_eq!(tracker.total_bytes_downloaded, 0);
        }

        #[test]
        fn test_tracker_default() {
            let tracker = DownloadProgressTracker::default();

            assert_eq!(tracker.total_videos, 0);
        }

        #[test]
        fn test_tracker_video_counts() {
            let mut tracker = DownloadProgressTracker::new(5);

            tracker.video_completed();
            tracker.video_completed();
            tracker.video_skipped();
            tracker.video_failed();

            assert_eq!(tracker.videos_completed, 2);
            assert_eq!(tracker.videos_skipped, 1);
            assert_eq!(tracker.videos_failed, 1);
        }

        #[test]
        fn test_tracker_elapsed_time() {
            let tracker = DownloadProgressTracker::new(5);

            // Elapsed time should be very small (close to 0)
            let elapsed = tracker.elapsed_secs();
            assert!(elapsed >= 0.0);
            assert!(elapsed < 1.0); // Should be less than 1 second
        }

        #[test]
        fn test_tracker_record_progress() {
            let mut tracker = DownloadProgressTracker::new(5);

            tracker.record_progress(1000);
            assert_eq!(tracker.total_bytes_downloaded, 1000);

            tracker.record_progress(2500);
            assert_eq!(tracker.total_bytes_downloaded, 2500);
        }

        #[test]
        fn test_tracker_create_progress() {
            let tracker = DownloadProgressTracker::new(4);

            let progress = tracker.create_progress(
                2,
                "Test Video",
                0.5,
                DownloadStatus::Downloading,
                512,
                Some(1024),
            );

            assert_eq!(progress.current_index, 2);
            assert_eq!(progress.total_videos, 4);
            assert_eq!(progress.current_title, "Test Video");
            assert!((progress.current_progress - 0.5).abs() < f64::EPSILON);
            assert_eq!(progress.status, DownloadStatus::Downloading);
            assert_eq!(progress.current_bytes, 512);
            assert_eq!(progress.current_total_bytes, Some(1024));
        }

        #[test]
        fn test_tracker_overall_progress_calculation() {
            let tracker = DownloadProgressTracker::new(4);

            // Video 2 at 50% progress: (2-1 + 0.5) / 4 = 0.375
            let progress =
                tracker.create_progress(2, "Test", 0.5, DownloadStatus::Downloading, 0, None);

            assert!((progress.overall_progress - 0.375).abs() < f64::EPSILON);
        }

        #[test]
        fn test_tracker_speed_with_insufficient_samples() {
            let tracker = DownloadProgressTracker::new(1);

            // With no samples, speed should be based on total elapsed
            let speed = tracker.download_speed_bps();
            // Speed could be 0 or very high depending on timing
            assert!(speed >= 0.0);
        }

        #[test]
        fn test_tracker_eta_with_no_progress() {
            let tracker = DownloadProgressTracker::new(5);

            // ETA should be None with 0% progress
            let eta = tracker.estimated_remaining_secs(0.0);
            assert!(eta.is_none());

            // ETA should be None with 100% progress
            let eta = tracker.estimated_remaining_secs(1.0);
            assert!(eta.is_none());
        }
    }
}
