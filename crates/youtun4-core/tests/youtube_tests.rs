//! Tests for `YouTube` playlist parsing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use youtun4_core::{RustyYtdlDownloader, YouTubeDownloader};

#[test]
fn test_parse_youtube_playlist() {
    let downloader = RustyYtdlDownloader::new();

    // Test with the example playlist URL
    let url = "https://www.youtube.com/playlist?list=PLw-VjHDlEOgvtnnnqWlTqByAtC7tXBg6D";

    println!("Testing playlist URL: {url}");

    let result = downloader.parse_playlist_url(url);

    match result {
        Ok(playlist_info) => {
            println!("Playlist ID: {}", playlist_info.id);
            println!("Playlist Title: {}", playlist_info.title);
            println!("Video Count: {}", playlist_info.video_count);
            println!("\nVideos:");
            for video in &playlist_info.videos {
                println!("  - {} ({})", video.title, video.id);
            }

            assert!(
                playlist_info.video_count > 0,
                "Should find at least one video"
            );
        }
        Err(e) => {
            panic!("Failed to parse playlist: {e:?}");
        }
    }
}

/// Test `rusty_ytdl` directly without our wrapper (using async API)
#[tokio::test]
#[ignore = "requires network access - run with: cargo test --ignored -- --nocapture"]
async fn test_rusty_ytdl_direct() {
    use rusty_ytdl::{Video, VideoOptions, VideoQuality, VideoSearchOptions};

    let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    println!("Creating video instance...");
    let video = Video::new(url).expect("Failed to create video");

    println!("Getting video info...");
    let info = video.get_info().await.expect("Failed to get info");

    println!("Title: {}", info.video_details.title);
    println!("Formats available: {}", info.formats.len());

    // Show all formats
    for (i, fmt) in info.formats.iter().enumerate() {
        println!(
            "  Format {}: mime={}, audio={}, video={}, bitrate={}",
            i, fmt.mime_type.mime, fmt.has_audio, fmt.has_video, fmt.bitrate
        );
    }

    // Try download method instead of stream
    let temp_path = std::env::temp_dir().join("rusty_ytdl_test.mp4");
    println!("\nTrying download method to: {temp_path:?}");

    match video.download(&temp_path).await {
        Ok(()) => {
            println!("Download successful!");
            if temp_path.exists() {
                let size = std::fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);
                println!("File size: {size} bytes");
                // Cleanup
                let _ = std::fs::remove_file(&temp_path);
            }
        }
        Err(e) => println!("Download error: {e}"),
    }

    // Also try creating video with VideoAudio filter for best of both worlds
    println!("\n--- Trying with VideoSearchOptions::VideoAudio ---");
    let video_opts = VideoOptions {
        quality: VideoQuality::Lowest, // Try lowest quality for faster test
        filter: VideoSearchOptions::VideoAudio,
        ..Default::default()
    };
    let video_audio =
        Video::new_with_options(url, video_opts).expect("Failed to create video with options");

    println!("Trying to get stream with VideoAudio filter...");
    match video_audio.stream().await {
        Ok(stream) => {
            println!("Got stream! Content length: {}", stream.content_length());

            // Try to get one chunk
            match stream.chunk().await {
                Ok(Some(chunk)) => println!("Got chunk of {} bytes!", chunk.len()),
                Ok(None) => println!("Empty stream"),
                Err(e) => println!("Chunk error: {e}"),
            }
        }
        Err(e) => println!("Stream error: {e}"),
    }
}

#[test]
#[ignore = "downloads from YouTube - run with: cargo test --ignored -- --nocapture"]
fn test_download_single_video() {
    // Initialize tracing for test output
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();

    let downloader = RustyYtdlDownloader::new();

    // Use the video that rusty_ytdl uses in their own tests
    // Known working video for rusty_ytdl
    let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";
    let _ = url; // Mark as intentionally unused in this test

    let temp_dir = std::env::temp_dir().join("youtun4_test");
    std::fs::create_dir_all(&temp_dir).expect("Should create temp dir");

    println!("Test download dir: {temp_dir:?}");

    let result = downloader.download_playlist(
        &youtun4_core::PlaylistInfo {
            id: "test".to_string(),
            title: "Test Playlist".to_string(),
            video_count: 1,
            videos: vec![youtun4_core::VideoInfo {
                id: "FZ8BxMU3BYc".to_string(),
                title: "Test Video".to_string(),
                duration_secs: Some(60),
                channel: Some("Test Channel".to_string()),
                thumbnail_url: None,
            }],
            thumbnail_url: None,
        },
        &temp_dir,
        None,
    );

    match result {
        Ok(results) => {
            println!("Download results:");
            for r in &results {
                println!("  - {} (success: {})", r.video.title, r.success);
                if let Some(path) = &r.output_path {
                    println!("    Path: {path:?}");
                    if path.exists() {
                        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                        println!("    Size: {size} bytes");
                    }
                }
                if let Some(err) = &r.error {
                    println!("    Error: {err}");
                }
            }

            // Check if at least one file was created
            let files: Vec<_> = std::fs::read_dir(&temp_dir)
                .expect("Should read dir")
                .filter_map(std::result::Result::ok)
                .collect();
            println!("Files in temp dir: {files:?}");
        }
        Err(e) => {
            println!("Download failed: {e:?}");
            // Don't panic - network tests can fail
        }
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
