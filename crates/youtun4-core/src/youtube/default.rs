use std::path::Path;

use tracing::{debug, info};

use crate::error::Result;

use super::downloader::YouTubeDownloader;
use super::model::{DownloadResult, DownloadStatus, PlaylistInfo, ProgressCallback};
use super::progress::DownloadProgressTracker;
use super::url::{extract_playlist_id, sanitize_filename};

/// Default `YouTube` downloader implementation.
/// Note: This is a placeholder that will need a proper `YouTube` downloading library.
#[derive(Debug)]
pub struct DefaultYouTubeDownloader;

impl DefaultYouTubeDownloader {
    /// Create a new downloader.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for DefaultYouTubeDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTubeDownloader for DefaultYouTubeDownloader {
    fn parse_playlist_url(&self, url: &str) -> Result<PlaylistInfo> {
        let playlist_id = extract_playlist_id(url)?;

        // TODO: Implement actual YouTube API/scraping to get playlist info
        // For now, return a placeholder that will be implemented with rustube or similar
        info!("Parsing playlist: {}", playlist_id);

        Ok(PlaylistInfo {
            id: playlist_id,
            title: "Placeholder".to_string(),
            video_count: 0,
            videos: vec![],
            thumbnail_url: None,
        })
    }

    fn download_playlist(
        &self,
        playlist_info: &PlaylistInfo,
        output_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<Vec<DownloadResult>> {
        info!(
            "Downloading playlist '{}' to {}",
            playlist_info.title,
            output_dir.display()
        );

        let mut results = Vec::new();
        let total_videos = playlist_info.videos.len();
        let mut tracker = DownloadProgressTracker::new(total_videos);

        for (index, video) in playlist_info.videos.iter().enumerate() {
            if let Some(ref callback) = progress {
                callback(tracker.create_progress(
                    index + 1,
                    &video.title,
                    0.0,
                    DownloadStatus::Starting,
                    0,
                    None,
                ));
            }

            // TODO: Implement actual download logic with rustube
            debug!("Would download: {} ({})", video.title, video.id);

            let filename = format!("{}.mp3", sanitize_filename(&video.title));
            let output_path = output_dir.join(&filename);

            tracker.video_failed();

            // Placeholder result
            results.push(DownloadResult {
                video: video.clone(),
                success: false,
                output_path: Some(output_path),
                error: Some("Download not yet implemented".to_string()),
            });
        }

        Ok(results)
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

    #[test]
    fn test_default_downloader_creation() {
        let downloader = DefaultYouTubeDownloader::new();
        let result =
            downloader.parse_playlist_url("https://www.youtube.com/playlist?list=PLtest123");
        result.unwrap();
    }
}
