use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::error::Result;

use super::downloader::YouTubeDownloader;
use super::model::{DownloadResult, PlaylistInfo, ProgressCallback};
use super::rusty_ytdl::{RustyYtdlConfig, RustyYtdlDownloader};

// ============================================================================
// Legacy YtDlpDownloader - kept for backwards compatibility but deprecated
// ============================================================================

/// Configuration for the yt-dlp downloader (deprecated - use `RustyYtdlConfig`).
#[deprecated(
    since = "0.2.0",
    note = "Use RustyYtdlConfig and RustyYtdlDownloader instead"
)]
#[derive(Debug, Clone)]
pub struct YtDlpConfig {
    /// Path to yt-dlp executable. If None, searches PATH.
    pub yt_dlp_path: Option<PathBuf>,
    /// Audio quality/bitrate for MP3 conversion (e.g., "192" for 192kbps).
    pub audio_quality: String,
    /// Whether to embed thumbnail in MP3.
    pub embed_thumbnail: bool,
    /// Whether to add metadata to MP3.
    pub add_metadata: bool,
    /// Download timeout in seconds per video.
    pub timeout_secs: u64,
    /// Number of retries for failed downloads.
    pub retries: u32,
}

#[allow(deprecated, reason = "implementing Default for deprecated type")]
impl Default for YtDlpConfig {
    fn default() -> Self {
        Self {
            yt_dlp_path: None,
            audio_quality: "192".to_string(),
            embed_thumbnail: true,
            add_metadata: true,
            timeout_secs: 300,
            retries: 3,
        }
    }
}

/// `YouTube` downloader using yt-dlp subprocess (deprecated - use `RustyYtdlDownloader`).
#[deprecated(
    since = "0.2.0",
    note = "Use RustyYtdlDownloader instead - it requires no external dependencies"
)]
#[derive(Debug)]
pub struct YtDlpDownloader {
    inner: RustyYtdlDownloader,
}

#[allow(deprecated, reason = "implementing methods for deprecated type")]
impl YtDlpDownloader {
    /// Create a new downloader with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RustyYtdlDownloader::new(),
        }
    }

    /// Create a new downloader with custom configuration.
    #[must_use]
    pub fn with_config(config: &YtDlpConfig) -> Self {
        let rusty_config = RustyYtdlConfig {
            timeout_secs: config.timeout_secs,
            retries: config.retries,
        };
        Self {
            inner: RustyYtdlDownloader::with_config(rusty_config),
        }
    }

    /// Get the cancel flag for external cancellation control.
    #[must_use]
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.inner.cancel_flag()
    }

    /// Cancel any ongoing download operation.
    pub fn cancel(&self) {
        self.inner.cancel();
    }

    /// Reset the cancel flag.
    pub fn reset_cancel(&self) {
        self.inner.reset_cancel();
    }
}

#[allow(deprecated, reason = "implementing Default for deprecated type")]
impl Default for YtDlpDownloader {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(deprecated, reason = "implementing trait for deprecated type")]
impl YouTubeDownloader for YtDlpDownloader {
    fn parse_playlist_url(&self, url: &str) -> Result<PlaylistInfo> {
        self.inner.parse_playlist_url(url)
    }

    fn download_playlist(
        &self,
        playlist_info: &PlaylistInfo,
        output_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<Vec<DownloadResult>> {
        self.inner
            .download_playlist(playlist_info, output_dir, progress)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;

    #[allow(
        deprecated,
        reason = "testing deprecated API for backwards compatibility"
    )]
    mod yt_dlp_downloader_tests {
        use super::*;

        #[test]
        fn test_yt_dlp_downloader_creation() {
            // YtDlpDownloader is now a thin wrapper - just test it creates
            let _downloader = YtDlpDownloader::new();
        }

        #[test]
        fn test_yt_dlp_downloader_with_config() {
            let config = YtDlpConfig {
                yt_dlp_path: Some(PathBuf::from("/custom/yt-dlp")),
                audio_quality: "320".to_string(),
                embed_thumbnail: false,
                add_metadata: true,
                timeout_secs: 600,
                retries: 5,
            };
            let _downloader = YtDlpDownloader::with_config(&config);
        }

        #[test]
        fn test_yt_dlp_downloader_cancel_flag() {
            let downloader = YtDlpDownloader::new();
            let flag = downloader.cancel_flag();

            // Initially not cancelled
            assert!(!flag.load(Ordering::SeqCst));

            // Set cancel
            downloader.cancel();
            assert!(flag.load(Ordering::SeqCst));

            // Reset cancel
            downloader.reset_cancel();
            assert!(!flag.load(Ordering::SeqCst));
        }

        #[test]
        fn test_yt_dlp_downloader_cancel_flag_shared() {
            let downloader = YtDlpDownloader::new();
            let flag = downloader.cancel_flag();

            // Cancel via the shared flag
            flag.store(true, Ordering::SeqCst);

            // Verify the cancellation is visible via a new flag request
            let flag2 = downloader.cancel_flag();
            assert!(flag2.load(Ordering::SeqCst));
        }

        #[test]
        fn test_yt_dlp_config_default() {
            let config = YtDlpConfig::default();

            assert!(config.yt_dlp_path.is_none());
            assert_eq!(config.audio_quality, "192");
            assert!(config.embed_thumbnail);
            assert!(config.add_metadata);
            assert_eq!(config.timeout_secs, 300);
            assert_eq!(config.retries, 3);
        }

        #[test]
        fn test_yt_dlp_downloader_default() {
            let _downloader = YtDlpDownloader::default();
        }

        #[test]
        fn test_yt_dlp_downloader_implements_youtube_downloader_trait() {
            // Verify that YtDlpDownloader implements YouTubeDownloader
            fn assert_youtube_downloader<T: YouTubeDownloader>() {}
            assert_youtube_downloader::<YtDlpDownloader>();
        }

        #[test]
        fn test_yt_dlp_downloader_parse_invalid_url() {
            let downloader = YtDlpDownloader::new();

            // Invalid URL should fail
            let result = downloader.parse_playlist_url("https://example.com");
            result.unwrap_err();
        }

        #[test]
        fn test_yt_dlp_downloader_parse_single_video_url() {
            let downloader = YtDlpDownloader::new();

            // Single video URL (no playlist) should fail
            let result =
                downloader.parse_playlist_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            result.unwrap_err();
        }
    }
}
