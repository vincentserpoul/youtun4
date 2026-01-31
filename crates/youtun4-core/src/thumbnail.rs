//! Thumbnail management for playlists and videos.
//!
//! This module provides functionality to:
//! - Fetch thumbnails from `YouTube` URLs
//! - Cache thumbnails locally
//! - Generate playlist thumbnails from first video
//!
//! Thumbnails are cached using the cache module infrastructure.

use std::time::Duration;

use tracing::{debug, info, warn};

use crate::cache::CacheManager;
use crate::error::{Error, Result};

/// Default timeout for thumbnail fetch requests.
pub const DEFAULT_FETCH_TIMEOUT_SECS: u64 = 30;

/// Thumbnail manager for fetching and caching playlist/video thumbnails.
pub struct ThumbnailManager<'a> {
    cache: &'a mut CacheManager,
    timeout: Duration,
}

impl<'a> ThumbnailManager<'a> {
    /// Create a new thumbnail manager with the given cache.
    pub const fn new(cache: &'a mut CacheManager) -> Self {
        Self {
            cache,
            timeout: Duration::from_secs(DEFAULT_FETCH_TIMEOUT_SECS),
        }
    }

    /// Set the fetch timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Fetch a thumbnail from URL and cache it.
    ///
    /// Returns the cached thumbnail data if successful.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the thumbnail (video ID or playlist ID)
    /// * `url` - URL to fetch the thumbnail from
    ///
    /// # Errors
    ///
    /// Returns an error if the fetch fails or caching fails.
    pub fn fetch_and_cache(&mut self, id: &str, url: &str) -> Result<Vec<u8>> {
        // Check if already cached
        if let Ok(Some(data)) = self.cache.get_thumbnail(id) {
            debug!("Thumbnail {} found in cache", id);
            return Ok(data);
        }

        info!("Fetching thumbnail for {} from {}", id, url);

        // Fetch the thumbnail
        let data = fetch_thumbnail_data(url, self.timeout)?;

        // Cache it
        self.cache.put_thumbnail(id, &data)?;

        info!("Cached thumbnail for {} ({} bytes)", id, data.len());
        Ok(data)
    }

    /// Get a cached thumbnail without fetching.
    ///
    /// Returns `None` if not cached.
    pub fn get_cached(&mut self, id: &str) -> Result<Option<Vec<u8>>> {
        self.cache.get_thumbnail(id)
    }

    /// Check if a thumbnail is cached.
    #[must_use]
    pub fn is_cached(&self, id: &str) -> bool {
        self.cache.has_thumbnail(id)
    }

    /// Get the path to a cached thumbnail file.
    ///
    /// Returns the path to the thumbnail file if it exists in cache.
    #[must_use]
    pub fn get_thumbnail_path(&self, id: &str) -> Option<std::path::PathBuf> {
        if self.cache.has_thumbnail(id) {
            let cache_dir = self.cache.cache_dir();
            Some(cache_dir.join("thumbnails").join(format!("thumb_{id}.jpg")))
        } else {
            None
        }
    }
}

/// Fetch thumbnail data from a URL.
///
/// # Errors
///
/// Returns an error if the fetch fails.
fn fetch_thumbnail_data(url: &str, timeout: Duration) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| Error::network_error(format!("Failed to create HTTP client: {e}")))?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| Error::network_error(format!("Failed to fetch thumbnail: {e}")))?;

    // Check content type
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("image/") {
        warn!("Unexpected content type for thumbnail: {}", content_type);
    }

    // Read the response body
    let data = response
        .bytes()
        .map_err(|e| Error::network_error(format!("Failed to read thumbnail data: {e}")))?;

    if data.is_empty() {
        return Err(Error::network_error("Empty thumbnail data"));
    }

    Ok(data.to_vec())
}

/// Generate a `YouTube` thumbnail URL for a video ID.
///
/// Returns the high-quality thumbnail URL (hqdefault).
#[must_use]
pub fn youtube_thumbnail_url(video_id: &str) -> String {
    // YouTube thumbnail URL format:
    // https://img.youtube.com/vi/{VIDEO_ID}/{QUALITY}.jpg
    // Quality options: default, mqdefault, hqdefault, sddefault, maxresdefault
    format!("https://img.youtube.com/vi/{video_id}/hqdefault.jpg")
}

/// Generate a high-resolution `YouTube` thumbnail URL for a video ID.
///
/// Note: maxresdefault is not always available for all videos.
#[must_use]
pub fn youtube_thumbnail_url_maxres(video_id: &str) -> String {
    format!("https://img.youtube.com/vi/{video_id}/maxresdefault.jpg")
}

/// Get the best thumbnail URL for a playlist.
///
/// If the playlist has its own thumbnail, use that.
/// Otherwise, use the first video's thumbnail.
#[must_use]
pub fn get_playlist_thumbnail_url(
    playlist_thumbnail: Option<&str>,
    first_video_id: Option<&str>,
) -> Option<String> {
    playlist_thumbnail
        .map(String::from)
        .or_else(|| first_video_id.map(youtube_thumbnail_url))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_thumbnail_url() {
        let url = youtube_thumbnail_url("dQw4w9WgXcQ");
        assert_eq!(url, "https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg");
    }

    #[test]
    fn test_youtube_thumbnail_url_maxres() {
        let url = youtube_thumbnail_url_maxres("dQw4w9WgXcQ");
        assert_eq!(
            url,
            "https://img.youtube.com/vi/dQw4w9WgXcQ/maxresdefault.jpg"
        );
    }

    #[test]
    fn test_get_playlist_thumbnail_url_with_playlist_thumbnail() {
        let url = get_playlist_thumbnail_url(Some("https://example.com/thumb.jpg"), Some("abc123"));
        assert_eq!(url, Some("https://example.com/thumb.jpg".to_string()));
    }

    #[test]
    fn test_get_playlist_thumbnail_url_fallback_to_video() {
        let url = get_playlist_thumbnail_url(None, Some("abc123"));
        assert_eq!(
            url,
            Some("https://img.youtube.com/vi/abc123/hqdefault.jpg".to_string())
        );
    }

    #[test]
    fn test_get_playlist_thumbnail_url_none() {
        let url = get_playlist_thumbnail_url(None, None);
        assert_eq!(url, None);
    }
}
