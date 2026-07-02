use std::path::Path;

use crate::error::Result;

use super::model::{DownloadResult, PlaylistInfo, ProgressCallback};

/// `YouTube` downloader trait for testability.
#[cfg_attr(test, mockall::automock)]
pub trait YouTubeDownloader: Send + Sync {
    /// Parse a `YouTube` URL and extract playlist information.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or not a playlist.
    fn parse_playlist_url(&self, url: &str) -> Result<PlaylistInfo>;

    /// Download all videos from a playlist as MP3 files.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails.
    fn download_playlist(
        &self,
        playlist_info: &PlaylistInfo,
        output_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<Vec<DownloadResult>>;
}
