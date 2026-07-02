use serde::{Deserialize, Serialize};

/// Information about a `YouTube` video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    /// Video ID.
    pub id: String,
    /// Video title.
    pub title: String,
    /// Video duration in seconds.
    pub duration_secs: Option<u64>,
    /// Channel/uploader name.
    pub channel: Option<String>,
    /// Thumbnail URL for the video.
    pub thumbnail_url: Option<String>,
}

/// Information about a `YouTube` playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistInfo {
    /// Playlist ID.
    pub id: String,
    /// Playlist title.
    pub title: String,
    /// Number of videos in the playlist.
    pub video_count: usize,
    /// Videos in the playlist.
    pub videos: Vec<VideoInfo>,
    /// Thumbnail URL for the playlist (or first video's thumbnail).
    pub thumbnail_url: Option<String>,
}

/// Progress callback for download operations.
pub type ProgressCallback = Box<dyn Fn(DownloadProgress) + Send + Sync>;

/// Download progress information.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Current video index (1-based).
    pub current_index: usize,
    /// Total number of videos.
    pub total_videos: usize,
    /// Current video title.
    pub current_title: String,
    /// Download progress for current video (0.0 - 1.0).
    pub current_progress: f64,
    /// Overall progress (0.0 - 1.0).
    pub overall_progress: f64,
    /// Status message.
    pub status: DownloadStatus,
    /// Bytes downloaded for the current file.
    pub current_bytes: u64,
    /// Total bytes for the current file (if known).
    pub current_total_bytes: Option<u64>,
    /// Total bytes downloaded across all files.
    pub total_bytes_downloaded: u64,
    /// Download speed in bytes per second.
    pub download_speed_bps: f64,
    /// Estimated time remaining in seconds.
    pub estimated_remaining_secs: Option<f64>,
    /// Elapsed time in seconds since download started.
    pub elapsed_secs: f64,
    /// Number of videos completed successfully.
    pub videos_completed: usize,
    /// Number of videos skipped (already exist).
    pub videos_skipped: usize,
    /// Number of videos that failed.
    pub videos_failed: usize,
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self {
            current_index: 0,
            total_videos: 0,
            current_title: String::new(),
            current_progress: 0.0,
            overall_progress: 0.0,
            status: DownloadStatus::Starting,
            current_bytes: 0,
            current_total_bytes: None,
            total_bytes_downloaded: 0,
            download_speed_bps: 0.0,
            estimated_remaining_secs: None,
            elapsed_secs: 0.0,
            videos_completed: 0,
            videos_skipped: 0,
            videos_failed: 0,
        }
    }
}

impl DownloadProgress {
    /// Create a new download progress instance.
    #[must_use]
    pub fn new(total_videos: usize) -> Self {
        Self {
            total_videos,
            ..Default::default()
        }
    }

    /// Calculate overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    #[allow(clippy::float_arithmetic, reason = "acceptable for display formatting")]
    pub fn overall_progress_percent(&self) -> f64 {
        self.overall_progress * 100.0
    }

    /// Calculate current video progress as a percentage (0.0 - 100.0).
    #[must_use]
    #[allow(clippy::float_arithmetic, reason = "acceptable for display formatting")]
    pub fn current_progress_percent(&self) -> f64 {
        self.current_progress * 100.0
    }

    /// Format the download speed as a human-readable string.
    #[must_use]
    pub fn formatted_speed(&self) -> String {
        format_bytes_per_second(self.download_speed_bps)
    }

    /// Format the estimated time remaining as a human-readable string.
    #[must_use]
    pub fn formatted_eta(&self) -> Option<String> {
        self.estimated_remaining_secs.map(format_duration)
    }

    /// Format the elapsed time as a human-readable string.
    #[must_use]
    pub fn formatted_elapsed(&self) -> String {
        format_duration(self.elapsed_secs)
    }
}

/// Format bytes per second as a human-readable string.
#[allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "acceptable for display formatting"
)]
fn format_bytes_per_second(bps: f64) -> String {
    if bps < 1024.0 {
        format!("{bps:.0} B/s")
    } else if bps < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bps / 1024.0)
    } else {
        format!("{:.1} MB/s", bps / (1024.0 * 1024.0))
    }
}

/// Format duration in seconds as a human-readable string.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "acceptable for display formatting"
)]
fn format_duration(secs: f64) -> String {
    let total_secs = secs as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Download status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    /// Starting download.
    Starting,
    /// Downloading audio.
    Downloading,
    /// Converting to MP3.
    Converting,
    /// Completed successfully.
    Completed,
    /// Failed with error message.
    Failed(String),
    /// Skipped (e.g., already exists).
    Skipped,
}

/// Result of downloading a single video.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// Video info.
    pub video: VideoInfo,
    /// Whether the download was successful.
    pub success: bool,
    /// Output file path (if successful).
    pub output_path: Option<std::path::PathBuf>,
    /// Error message (if failed).
    pub error: Option<String>,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    // =========================================================================
    // Download Status Tests
    // =========================================================================

    #[test]
    fn test_download_status_equality() {
        assert_eq!(DownloadStatus::Starting, DownloadStatus::Starting);
        assert_eq!(
            DownloadStatus::Failed("error".to_string()),
            DownloadStatus::Failed("error".to_string())
        );
        assert_ne!(DownloadStatus::Starting, DownloadStatus::Downloading);
    }

    // =========================================================================
    // Download Progress and Result Tests
    // =========================================================================

    mod download_progress_tests {
        use super::*;

        #[test]
        fn test_download_progress_struct() {
            let progress = DownloadProgress {
                current_index: 5,
                total_videos: 10,
                current_title: "Test Song".to_string(),
                current_progress: 0.75,
                overall_progress: 0.45,
                status: DownloadStatus::Downloading,
                current_bytes: 1024,
                current_total_bytes: Some(2048),
                total_bytes_downloaded: 5000,
                download_speed_bps: 102_400.0,
                estimated_remaining_secs: Some(30.0),
                elapsed_secs: 10.5,
                videos_completed: 3,
                videos_skipped: 1,
                videos_failed: 0,
            };

            assert_eq!(progress.current_index, 5);
            assert_eq!(progress.total_videos, 10);
            assert_eq!(progress.current_title, "Test Song");
            assert!((progress.current_progress - 0.75).abs() < f64::EPSILON);
            assert!((progress.overall_progress - 0.45).abs() < f64::EPSILON);
            assert_eq!(progress.status, DownloadStatus::Downloading);
            assert_eq!(progress.current_bytes, 1024);
            assert_eq!(progress.current_total_bytes, Some(2048));
            assert_eq!(progress.total_bytes_downloaded, 5000);
            assert!((progress.download_speed_bps - 102_400.0).abs() < f64::EPSILON);
            assert_eq!(progress.estimated_remaining_secs, Some(30.0));
            assert!((progress.elapsed_secs - 10.5).abs() < f64::EPSILON);
            assert_eq!(progress.videos_completed, 3);
            assert_eq!(progress.videos_skipped, 1);
            assert_eq!(progress.videos_failed, 0);
        }

        #[test]
        fn test_download_progress_formatting() {
            let progress = DownloadProgress {
                current_index: 1,
                total_videos: 5,
                current_title: "Test".to_string(),
                current_progress: 0.5,
                overall_progress: 0.1,
                status: DownloadStatus::Downloading,
                current_bytes: 0,
                current_total_bytes: None,
                total_bytes_downloaded: 0,
                download_speed_bps: 1_536_000.0,       // ~1.5 MB/s
                estimated_remaining_secs: Some(150.0), // 2:30
                elapsed_secs: 75.0,                    // 1:15
                videos_completed: 0,
                videos_skipped: 0,
                videos_failed: 0,
            };

            assert_eq!(progress.formatted_speed(), "1.5 MB/s");
            assert_eq!(progress.formatted_eta(), Some("2:30".to_string()));
            assert_eq!(progress.formatted_elapsed(), "1:15");
            assert!((progress.overall_progress_percent() - 10.0).abs() < f64::EPSILON);
            assert!((progress.current_progress_percent() - 50.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_download_progress_default() {
            let progress = DownloadProgress::default();

            assert_eq!(progress.current_index, 0);
            assert_eq!(progress.total_videos, 0);
            assert!(progress.current_title.is_empty());
            assert!((progress.current_progress - 0.0).abs() < f64::EPSILON);
            assert!((progress.overall_progress - 0.0).abs() < f64::EPSILON);
            assert_eq!(progress.status, DownloadStatus::Starting);
            assert_eq!(progress.current_bytes, 0);
            assert_eq!(progress.current_total_bytes, None);
            assert!(progress.download_speed_bps.abs() < f64::EPSILON);
        }

        #[test]
        fn test_download_progress_new() {
            let progress = DownloadProgress::new(10);

            assert_eq!(progress.total_videos, 10);
            assert_eq!(progress.current_index, 0);
        }

        #[test]
        fn test_download_result_success() {
            let video = VideoInfo {
                id: "test123".to_string(),
                title: "Test Video".to_string(),
                duration_secs: Some(180),
                channel: Some("Test Channel".to_string()),
                thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            };

            let result = DownloadResult {
                video,
                success: true,
                output_path: Some(PathBuf::from("/output/test.mp3")),
                error: None,
            };

            assert!(result.success);
            assert!(result.output_path.is_some());
            assert!(result.error.is_none());
        }

        #[test]
        fn test_download_result_failure() {
            let video = VideoInfo {
                id: "test123".to_string(),
                title: "Test Video".to_string(),
                duration_secs: None,
                channel: None,
                thumbnail_url: None,
            };

            let result = DownloadResult {
                video,
                success: false,
                output_path: None,
                error: Some("Download failed".to_string()),
            };

            assert!(!result.success);
            assert!(result.output_path.is_none());
            assert!(result.error.is_some());
        }

        #[test]
        fn test_video_info_clone() {
            let video = VideoInfo {
                id: "test123".to_string(),
                title: "Test Video".to_string(),
                duration_secs: Some(180),
                channel: Some("Test Channel".to_string()),
                thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            };

            let cloned = video.clone();
            assert_eq!(video.id, cloned.id);
            assert_eq!(video.title, cloned.title);
            assert_eq!(video.duration_secs, cloned.duration_secs);
            assert_eq!(video.channel, cloned.channel);
            assert_eq!(video.thumbnail_url, cloned.thumbnail_url);
        }

        #[test]
        fn test_playlist_info_struct() {
            let videos = vec![
                VideoInfo {
                    id: "vid1".to_string(),
                    title: "Video 1".to_string(),
                    duration_secs: Some(120),
                    channel: None,
                    thumbnail_url: None,
                },
                VideoInfo {
                    id: "vid2".to_string(),
                    title: "Video 2".to_string(),
                    duration_secs: Some(240),
                    channel: None,
                    thumbnail_url: None,
                },
            ];

            let playlist = PlaylistInfo {
                id: "PLtest123".to_string(),
                title: "Test Playlist".to_string(),
                video_count: 2,
                videos,
                thumbnail_url: Some("https://example.com/playlist-thumb.jpg".to_string()),
            };

            assert_eq!(playlist.id, "PLtest123");
            assert_eq!(playlist.title, "Test Playlist");
            assert_eq!(playlist.video_count, 2);
            assert_eq!(playlist.videos.len(), 2);
            assert!(playlist.thumbnail_url.is_some());
        }
    }

    // =========================================================================
    // Format Helper Tests
    // =========================================================================

    mod format_helper_tests {
        use super::*;

        #[test]
        fn test_format_bytes_per_second_bytes() {
            assert_eq!(format_bytes_per_second(0.0), "0 B/s");
            assert_eq!(format_bytes_per_second(512.0), "512 B/s");
            assert_eq!(format_bytes_per_second(1023.0), "1023 B/s");
        }

        #[test]
        fn test_format_bytes_per_second_kilobytes() {
            assert_eq!(format_bytes_per_second(1024.0), "1.0 KB/s");
            assert_eq!(format_bytes_per_second(1536.0), "1.5 KB/s");
            assert_eq!(format_bytes_per_second(102_400.0), "100.0 KB/s");
        }

        #[test]
        fn test_format_bytes_per_second_megabytes() {
            assert_eq!(format_bytes_per_second(1_048_576.0), "1.0 MB/s");
            assert_eq!(format_bytes_per_second(1_572_864.0), "1.5 MB/s");
            assert_eq!(format_bytes_per_second(10_485_760.0), "10.0 MB/s");
        }

        #[test]
        fn test_format_duration_seconds() {
            assert_eq!(format_duration(0.0), "0:00");
            assert_eq!(format_duration(30.0), "0:30");
            assert_eq!(format_duration(59.0), "0:59");
        }

        #[test]
        fn test_format_duration_minutes() {
            assert_eq!(format_duration(60.0), "1:00");
            assert_eq!(format_duration(90.0), "1:30");
            assert_eq!(format_duration(150.0), "2:30");
            assert_eq!(format_duration(3599.0), "59:59");
        }

        #[test]
        fn test_format_duration_hours() {
            assert_eq!(format_duration(3600.0), "1:00:00");
            assert_eq!(format_duration(3660.0), "1:01:00");
            assert_eq!(format_duration(3661.0), "1:01:01");
            assert_eq!(format_duration(7200.0), "2:00:00");
        }
    }
}
