use serde::{Deserialize, Serialize};

use crate::error::{DownloadError, Error, Result};

/// Result of `YouTube` URL validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeUrlValidation {
    /// Whether the URL is valid.
    pub is_valid: bool,
    /// The extracted playlist ID (if valid).
    pub playlist_id: Option<String>,
    /// The normalized/canonical URL.
    pub normalized_url: Option<String>,
    /// Error message if validation failed.
    pub error_message: Option<String>,
    /// The URL type detected.
    pub url_type: YouTubeUrlType,
}

/// Type of `YouTube` URL detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum YouTubeUrlType {
    /// Standard playlist URL (youtube.com/playlist?list=...)
    Playlist,
    /// Watch URL with playlist parameter (youtube.com/watch?v=...&list=...)
    WatchWithPlaylist,
    /// Single video URL without playlist
    SingleVideo,
    /// Short URL (youtu.be/...)
    ShortUrl,
    /// Invalid or unrecognized URL
    #[default]
    Invalid,
}

impl YouTubeUrlValidation {
    /// Create a successful validation result.
    #[must_use]
    pub const fn valid(
        playlist_id: String,
        url_type: YouTubeUrlType,
        normalized_url: String,
    ) -> Self {
        Self {
            is_valid: true,
            playlist_id: Some(playlist_id),
            normalized_url: Some(normalized_url),
            error_message: None,
            url_type,
        }
    }

    /// Create a failed validation result.
    #[must_use]
    pub const fn invalid(error_message: String, url_type: YouTubeUrlType) -> Self {
        Self {
            is_valid: false,
            playlist_id: None,
            normalized_url: None,
            error_message: Some(error_message),
            url_type,
        }
    }
}

/// Validate a `YouTube` URL and extract playlist information.
///
/// This function performs comprehensive validation of `YouTube` URLs and extracts
/// playlist IDs when present. It supports multiple URL formats and provides
/// detailed error messages.
///
/// # Supported URL Formats
///
/// - `https://www.youtube.com/playlist?list=PLxxxxxxxx` - Standard playlist URL
/// - `https://youtube.com/playlist?list=PLxxxxxxxx` - Without www
/// - `https://www.youtube.com/watch?v=xxxxx&list=PLxxxxxxxx` - Watch with playlist
/// - `https://youtu.be/xxxxx?list=PLxxxxxxxx` - Short URL with playlist
/// - `http://` variants are also accepted
///
/// # Examples
///
/// ```rust
/// use youtun4_core::youtube::validate_youtube_url;
///
/// // Valid playlist URL
/// let result = validate_youtube_url("https://www.youtube.com/playlist?list=PLrAXtmErZgOei");
/// assert!(result.is_valid);
/// assert_eq!(result.playlist_id, Some("PLrAXtmErZgOei".to_string()));
///
/// // Invalid URL
/// let result = validate_youtube_url("https://example.com");
/// assert!(!result.is_valid);
/// assert!(result.error_message.is_some());
/// ```
#[must_use]
pub fn validate_youtube_url(url: &str) -> YouTubeUrlValidation {
    let url = url.trim();

    // Check for empty URL
    if url.is_empty() {
        return YouTubeUrlValidation::invalid(
            "URL cannot be empty".to_string(),
            YouTubeUrlType::Invalid,
        );
    }

    // Check for basic URL format (must start with http:// or https://)
    let url_lower = url.to_lowercase();
    if !url_lower.starts_with("http://") && !url_lower.starts_with("https://") {
        return YouTubeUrlValidation::invalid(
            "URL must start with http:// or https://".to_string(),
            YouTubeUrlType::Invalid,
        );
    }

    // Check if it's a YouTube URL
    let is_youtube_domain = url_lower.contains("youtube.com") || url_lower.contains("youtu.be");
    if !is_youtube_domain {
        return YouTubeUrlValidation::invalid(
            "URL must be a YouTube URL (youtube.com or youtu.be)".to_string(),
            YouTubeUrlType::Invalid,
        );
    }

    // Determine URL type and extract playlist ID
    let url_type = detect_url_type(url);

    // Extract playlist ID based on URL type
    if let Some(playlist_id) = extract_playlist_id_internal(url) {
        // Validate playlist ID format
        if let Err(validation_error) = validate_playlist_id_format(&playlist_id) {
            return YouTubeUrlValidation::invalid(validation_error, url_type);
        }

        // Generate normalized URL
        let normalized = format!("https://www.youtube.com/playlist?list={playlist_id}");

        YouTubeUrlValidation::valid(playlist_id, url_type, normalized)
    } else {
        let error_msg = match url_type {
            YouTubeUrlType::SingleVideo => {
                "URL is a single video, not a playlist. Add a playlist to the URL or use a playlist URL.".to_string()
            }
            YouTubeUrlType::ShortUrl => {
                "Short URL does not contain a playlist. Use a playlist URL instead.".to_string()
            }
            YouTubeUrlType::Playlist | YouTubeUrlType::WatchWithPlaylist | YouTubeUrlType::Invalid => "URL does not contain a valid playlist ID".to_string(),
        };
        YouTubeUrlValidation::invalid(error_msg, url_type)
    }
}

/// Detect the type of `YouTube` URL.
fn detect_url_type(url: &str) -> YouTubeUrlType {
    let url_lower = url.to_lowercase();

    if url_lower.contains("youtu.be/") {
        if url_lower.contains("list=") {
            YouTubeUrlType::WatchWithPlaylist
        } else {
            YouTubeUrlType::ShortUrl
        }
    } else if url_lower.contains("/playlist") {
        YouTubeUrlType::Playlist
    } else if url_lower.contains("/watch") {
        if url_lower.contains("list=") {
            YouTubeUrlType::WatchWithPlaylist
        } else {
            YouTubeUrlType::SingleVideo
        }
    } else {
        YouTubeUrlType::Invalid
    }
}

/// Extract playlist ID from URL (internal implementation).
fn extract_playlist_id_internal(url: &str) -> Option<String> {
    let url_lower = url.to_lowercase();

    // Find list= parameter (case-insensitive search)
    if let Some(list_pos) = url_lower.find("list=") {
        let start = list_pos + 5;
        let rest = &url[start..];

        // Extract until next & or # or end of string
        let end = rest.find(['&', '#']).unwrap_or(rest.len());
        let playlist_id = rest[..end].trim();

        if !playlist_id.is_empty() {
            return Some(playlist_id.to_string());
        }
    }

    None
}

/// Validate playlist ID format.
///
/// `YouTube` playlist IDs have specific formats:
/// - User-created playlists: Start with "PL" followed by alphanumeric characters
/// - Watch Later: "WL"
/// - Liked Videos: "LL"
/// - Mix playlists: Start with "RD"
/// - Album playlists: Start with "`OLAK5uy`_"
fn validate_playlist_id_format(playlist_id: &str) -> std::result::Result<(), String> {
    // Check minimum length
    if playlist_id.len() < 2 {
        return Err("Playlist ID is too short".to_string());
    }

    // Check maximum length (YouTube playlist IDs are typically under 50 chars)
    if playlist_id.len() > 64 {
        return Err("Playlist ID is too long".to_string());
    }

    // Check for valid characters (alphanumeric, underscore, hyphen)
    if !playlist_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Playlist ID contains invalid characters".to_string());
    }

    // Known valid playlist ID prefixes
    let valid_prefixes = ["PL", "UU", "LL", "WL", "RD", "OLAK5uy_", "FL"];

    // Check if it matches a known prefix or is alphanumeric (for edge cases)
    let has_valid_prefix = valid_prefixes
        .iter()
        .any(|prefix| playlist_id.starts_with(prefix));

    // Allow any valid-looking alphanumeric ID (YouTube may have other formats)
    if !has_valid_prefix
        && !playlist_id
            .chars()
            .next()
            .is_some_and(char::is_alphanumeric)
    {
        return Err("Playlist ID has an invalid format".to_string());
    }

    Ok(())
}

/// Parse a `YouTube` playlist URL and extract the playlist ID.
///
/// Supports the following URL formats:
/// - `https://www.youtube.com/playlist?list=PLxxxxxxxx`
/// - `https://youtube.com/playlist?list=PLxxxxxxxx`
/// - `https://www.youtube.com/watch?v=xxxxx&list=PLxxxxxxxx`
/// - `https://youtu.be/xxxxx?list=PLxxxxxxxx`
///
/// # Errors
///
/// Returns an error if the URL is not a valid `YouTube` playlist URL.
///
/// # Panics
///
/// Panics if the URL validation reports valid but has no playlist ID (should never happen).
#[allow(
    clippy::expect_used,
    reason = "valid URL is guaranteed to have playlist ID"
)]
pub fn extract_playlist_id(url: &str) -> Result<String> {
    let validation = validate_youtube_url(url);

    if validation.is_valid {
        Ok(validation
            .playlist_id
            .expect("Valid URL should have playlist ID"))
    } else {
        let error_message = validation
            .error_message
            .unwrap_or_else(|| "Invalid URL".to_string());

        // Determine error type based on URL type
        match validation.url_type {
            YouTubeUrlType::Invalid => Err(Error::Download(DownloadError::InvalidUrl {
                url: url.to_string(),
                reason: error_message,
            })),
            YouTubeUrlType::SingleVideo | YouTubeUrlType::ShortUrl => {
                Err(Error::Download(DownloadError::NotAPlaylist {
                    url: url.to_string(),
                }))
            }
            YouTubeUrlType::Playlist | YouTubeUrlType::WatchWithPlaylist => {
                Err(Error::Download(DownloadError::NotAPlaylist {
                    url: url.to_string(),
                }))
            }
        }
    }
}

/// Sanitize a string for use as a filename.
#[must_use]
pub fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];

    let sanitized: String = name
        .chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
        .collect();

    // Trim whitespace and dots from ends
    let trimmed = sanitized.trim().trim_matches('.');

    // Limit length (leaving room for extension)
    if trimmed.len() > 200 {
        trimmed[..200].to_string()
    } else {
        trimmed.to_string()
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

    // =========================================================================
    // URL Validation Tests
    // =========================================================================

    mod validate_youtube_url_tests {
        use super::*;

        #[test]
        fn test_valid_standard_playlist_url() {
            let url = "https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(
                result.playlist_id,
                Some("PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf".to_string())
            );
            assert_eq!(result.url_type, YouTubeUrlType::Playlist);
            assert!(result.error_message.is_none());
            assert!(result.normalized_url.is_some());
        }

        #[test]
        fn test_valid_playlist_url_without_www() {
            let url = "https://youtube.com/playlist?list=PLtest123abc";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123abc".to_string()));
        }

        #[test]
        fn test_valid_watch_url_with_playlist() {
            let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOtest";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLrAXtmErZgOtest".to_string()));
            assert_eq!(result.url_type, YouTubeUrlType::WatchWithPlaylist);
        }

        #[test]
        fn test_valid_watch_url_list_first() {
            let url = "https://www.youtube.com/watch?list=PLrAXtmErZgOtest&v=dQw4w9WgXcQ";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLrAXtmErZgOtest".to_string()));
        }

        #[test]
        fn test_valid_short_url_with_playlist() {
            let url = "https://youtu.be/dQw4w9WgXcQ?list=PLrAXtmErZgOtest";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLrAXtmErZgOtest".to_string()));
        }

        #[test]
        fn test_valid_http_url() {
            let url = "http://www.youtube.com/playlist?list=PLtest123";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123".to_string()));
        }

        #[test]
        fn test_valid_mixed_case_list_parameter() {
            let url = "https://www.youtube.com/playlist?LIST=PLtest123";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123".to_string()));
        }

        #[test]
        fn test_valid_user_uploads_playlist() {
            let url = "https://www.youtube.com/playlist?list=UUxxxxxxxxxxxxxxxxxxxxxxxx";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(
                result.playlist_id,
                Some("UUxxxxxxxxxxxxxxxxxxxxxxxx".to_string())
            );
        }

        #[test]
        fn test_valid_mix_playlist() {
            let url = "https://www.youtube.com/watch?v=abc&list=RDxxxxxxxx";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("RDxxxxxxxx".to_string()));
        }

        #[test]
        fn test_invalid_empty_url() {
            let result = validate_youtube_url("");

            assert!(!result.is_valid);
            assert_eq!(result.url_type, YouTubeUrlType::Invalid);
            assert!(result.error_message.unwrap().contains("empty"));
        }

        #[test]
        fn test_invalid_whitespace_only_url() {
            let result = validate_youtube_url("   ");

            assert!(!result.is_valid);
            assert!(result.error_message.unwrap().contains("empty"));
        }

        #[test]
        fn test_invalid_not_youtube() {
            let url = "https://www.vimeo.com/video/123";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert_eq!(result.url_type, YouTubeUrlType::Invalid);
            assert!(result.error_message.unwrap().contains("YouTube"));
        }

        #[test]
        fn test_invalid_no_protocol() {
            let url = "www.youtube.com/playlist?list=PLtest123";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert!(result.error_message.unwrap().contains("http"));
        }

        #[test]
        fn test_invalid_single_video_no_playlist() {
            let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert_eq!(result.url_type, YouTubeUrlType::SingleVideo);
            assert!(result.error_message.unwrap().contains("single video"));
        }

        #[test]
        fn test_invalid_short_url_no_playlist() {
            let url = "https://youtu.be/dQw4w9WgXcQ";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert_eq!(result.url_type, YouTubeUrlType::ShortUrl);
        }

        #[test]
        fn test_invalid_empty_list_parameter() {
            let url = "https://www.youtube.com/playlist?list=";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
        }

        #[test]
        fn test_invalid_playlist_id_too_short() {
            let url = "https://www.youtube.com/playlist?list=X";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert!(result.error_message.unwrap().contains("too short"));
        }

        #[test]
        fn test_invalid_playlist_id_special_chars() {
            let url = "https://www.youtube.com/playlist?list=PL<script>alert(1)</script>";
            let result = validate_youtube_url(url);

            assert!(!result.is_valid);
            assert!(result.error_message.unwrap().contains("invalid characters"));
        }

        #[test]
        fn test_url_with_trailing_whitespace() {
            let url = "  https://www.youtube.com/playlist?list=PLtest123  ";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123".to_string()));
        }

        #[test]
        fn test_url_with_hash_fragment() {
            let url = "https://www.youtube.com/playlist?list=PLtest123#section";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(result.playlist_id, Some("PLtest123".to_string()));
        }

        #[test]
        fn test_normalized_url_format() {
            let url = "https://youtube.com/watch?v=abc&list=PLtest123";
            let result = validate_youtube_url(url);

            assert!(result.is_valid);
            assert_eq!(
                result.normalized_url,
                Some("https://www.youtube.com/playlist?list=PLtest123".to_string())
            );
        }
    }

    // =========================================================================
    // Playlist ID Validation Tests
    // =========================================================================

    mod playlist_id_format_tests {
        use super::*;

        #[test]
        fn test_valid_pl_prefix() {
            validate_playlist_id_format("PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf").unwrap();
        }

        #[test]
        fn test_valid_uu_prefix() {
            validate_playlist_id_format("UUxxxxxxxxxxxxxxxxxxxxxxxx").unwrap();
        }

        #[test]
        fn test_valid_rd_prefix() {
            validate_playlist_id_format("RDxxxxxxxx").unwrap();
        }

        #[test]
        fn test_valid_olak_prefix() {
            validate_playlist_id_format("OLAK5uy_xxxxxxxxxxxxxxxxx").unwrap();
        }

        #[test]
        fn test_invalid_too_short() {
            assert!(validate_playlist_id_format("X").is_err());
        }

        #[test]
        fn test_invalid_too_long() {
            let long_id = "PL".to_string() + &"x".repeat(100);
            assert!(validate_playlist_id_format(&long_id).is_err());
        }

        #[test]
        fn test_invalid_special_characters() {
            assert!(validate_playlist_id_format("PL<>test").is_err());
            assert!(validate_playlist_id_format("PL test").is_err());
            assert!(validate_playlist_id_format("PL@test").is_err());
        }

        #[test]
        fn test_valid_with_underscore_and_hyphen() {
            validate_playlist_id_format("PLtest_123-abc").unwrap();
        }
    }

    // =========================================================================
    // URL Type Detection Tests
    // =========================================================================

    mod url_type_detection_tests {
        use super::*;

        #[test]
        fn test_detect_playlist_type() {
            assert_eq!(
                detect_url_type("https://www.youtube.com/playlist?list=PLtest"),
                YouTubeUrlType::Playlist
            );
        }

        #[test]
        fn test_detect_watch_with_playlist_type() {
            assert_eq!(
                detect_url_type("https://www.youtube.com/watch?v=abc&list=PLtest"),
                YouTubeUrlType::WatchWithPlaylist
            );
        }

        #[test]
        fn test_detect_single_video_type() {
            assert_eq!(
                detect_url_type("https://www.youtube.com/watch?v=abc"),
                YouTubeUrlType::SingleVideo
            );
        }

        #[test]
        fn test_detect_short_url_type() {
            assert_eq!(
                detect_url_type("https://youtu.be/abc"),
                YouTubeUrlType::ShortUrl
            );
        }

        #[test]
        fn test_detect_short_url_with_playlist() {
            assert_eq!(
                detect_url_type("https://youtu.be/abc?list=PLtest"),
                YouTubeUrlType::WatchWithPlaylist
            );
        }

        #[test]
        fn test_detect_invalid_type() {
            assert_eq!(
                detect_url_type("https://youtube.com/channel/abc"),
                YouTubeUrlType::Invalid
            );
        }
    }

    // =========================================================================
    // Extract Playlist ID Tests (backward compatibility)
    // =========================================================================

    #[test]
    fn test_extract_playlist_id_standard_url() {
        let url = "https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf";
        let result = extract_playlist_id(url);
        assert!(result.is_ok());
        assert_eq!(
            result.expect("Should have ID"),
            "PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf"
        );
    }

    #[test]
    fn test_extract_playlist_id_watch_url_with_list() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOtest";
        let result = extract_playlist_id(url);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should have ID"), "PLrAXtmErZgOtest");
    }

    #[test]
    fn test_extract_playlist_id_no_playlist() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        let result = extract_playlist_id(url);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Download(DownloadError::NotAPlaylist { .. }))
        ));
    }

    #[test]
    fn test_extract_playlist_id_not_youtube() {
        let url = "https://www.vimeo.com/video/123";
        let result = extract_playlist_id(url);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Download(DownloadError::InvalidUrl { .. }))
        ));
    }

    // =========================================================================
    // Sanitize Filename Tests
    // =========================================================================

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(sanitize_filename("Hello World"), "Hello World");
    }

    #[test]
    fn test_sanitize_filename_invalid_chars() {
        assert_eq!(sanitize_filename("Hello/World"), "Hello_World");
        assert_eq!(sanitize_filename("Test:File"), "Test_File");
        assert_eq!(sanitize_filename("A*B?C"), "A_B_C");
    }

    #[test]
    fn test_sanitize_filename_trim() {
        assert_eq!(sanitize_filename("  Hello  "), "Hello");
        assert_eq!(sanitize_filename("...test..."), "test");
    }

    #[test]
    fn test_sanitize_filename_long_name() {
        let long_name = "a".repeat(300);
        let result = sanitize_filename(&long_name);
        assert_eq!(result.len(), 200);
    }

    // =========================================================================
    // YouTubeUrlValidation Struct Tests
    // =========================================================================

    #[test]
    fn test_youtube_url_validation_valid() {
        let validation = YouTubeUrlValidation::valid(
            "PLtest123".to_string(),
            YouTubeUrlType::Playlist,
            "https://www.youtube.com/playlist?list=PLtest123".to_string(),
        );

        assert!(validation.is_valid);
        assert_eq!(validation.playlist_id, Some("PLtest123".to_string()));
        assert!(validation.error_message.is_none());
    }

    #[test]
    fn test_youtube_url_validation_invalid() {
        let validation =
            YouTubeUrlValidation::invalid("Test error".to_string(), YouTubeUrlType::Invalid);

        assert!(!validation.is_valid);
        assert!(validation.playlist_id.is_none());
        assert_eq!(validation.error_message, Some("Test error".to_string()));
    }

    // =========================================================================
    // Default Trait Tests
    // =========================================================================

    #[test]
    fn test_youtube_url_type_default() {
        let url_type = YouTubeUrlType::default();
        assert_eq!(url_type, YouTubeUrlType::Invalid);
    }
}
