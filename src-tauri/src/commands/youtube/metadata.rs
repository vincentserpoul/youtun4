use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info};
use youtun4_core::time::unix_timestamp_secs;
use youtun4_core::youtube::PlaylistInfo;

/// Audio file extensions recognized for track counting.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "mp4", "wav", "flac", "ogg", "aac", "webm", "opus",
];

/// Update playlist.json with source URL and thumbnail before download.
pub(super) fn update_playlist_metadata_before_download(
    playlist_json_path: &Path,
    source_url: &str,
    playlist_info: &PlaylistInfo,
) {
    if !playlist_json_path.exists() {
        return;
    }

    let content = match fs::read_to_string(playlist_json_path) {
        Ok(c) => c,
        Err(e) => {
            debug!(
                "Could not read playlist metadata at {}: {e}",
                playlist_json_path.display()
            );
            return;
        }
    };
    let Ok(mut metadata) = serde_json::from_str::<serde_json::Value>(&content) else {
        debug!(
            "Could not parse playlist metadata at {}",
            playlist_json_path.display()
        );
        return;
    };
    let Some(obj) = metadata.as_object_mut() else {
        debug!(
            "Playlist metadata is not a JSON object at {}",
            playlist_json_path.display()
        );
        return;
    };

    obj.insert("source_url".to_string(), serde_json::json!(source_url));

    if let Some(thumb) = &playlist_info.thumbnail_url {
        obj.insert("thumbnail_url".to_string(), serde_json::json!(thumb));
    }

    match serde_json::to_string_pretty(&metadata) {
        Ok(updated) => {
            if let Err(e) = fs::write(playlist_json_path, updated) {
                debug!(
                    "Could not write playlist metadata at {}: {e}",
                    playlist_json_path.display()
                );
            }
        }
        Err(e) => {
            debug!("Could not serialize playlist metadata: {e}");
        }
    }
}

/// Count audio files and calculate total size in a directory.
fn count_audio_files(dir: &Path) -> (usize, u64) {
    let mut track_count = 0usize;
    let mut total_size = 0u64;

    let Ok(entries) = fs::read_dir(dir) else {
        return (track_count, total_size);
    };

    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        if AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            track_count += 1;
            if let Ok(meta) = fs::metadata(&path) {
                total_size += meta.len();
            }
        }
    }

    (track_count, total_size)
}

/// Build track metadata from download results.
fn build_tracks_metadata(
    results: &[youtun4_core::youtube::DownloadResult],
) -> Vec<serde_json::Value> {
    let now = unix_timestamp_secs();

    results
        .iter()
        .filter(|r| r.success && r.output_path.is_some())
        .map(|r| {
            let file_name = r
                .output_path
                .as_ref()
                .and_then(|p: &PathBuf| p.file_name())
                .and_then(|n: &std::ffi::OsStr| n.to_str())
                .unwrap_or("")
                .to_string();

            serde_json::json!({
                "file_name": file_name,
                "video_id": r.video.id,
                "source_url": format!("https://www.youtube.com/watch?v={}", r.video.id),
                "title": r.video.title,
                "channel": r.video.channel,
                "duration_secs": r.video.duration_secs,
                "thumbnail_url": r.video.thumbnail_url,
                "downloaded_at": now
            })
        })
        .collect()
}

/// Update playlist.json with track count, size, and metadata after download.
pub(super) fn update_playlist_metadata_after_download(
    playlist_json_path: &Path,
    output_path: &Path,
    results: &[youtun4_core::youtube::DownloadResult],
) {
    let Ok(content) = fs::read_to_string(playlist_json_path) else {
        return;
    };
    let Ok(mut metadata) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };
    let Some(obj) = metadata.as_object_mut() else {
        return;
    };

    let (track_count, total_size) = count_audio_files(output_path);

    obj.insert("track_count".to_string(), serde_json::json!(track_count));
    obj.insert(
        "total_size_bytes".to_string(),
        serde_json::json!(total_size),
    );
    obj.insert(
        "modified_at".to_string(),
        serde_json::json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        ),
    );

    let tracks_metadata = build_tracks_metadata(results);
    obj.insert("tracks".to_string(), serde_json::json!(tracks_metadata));

    if let Ok(updated) = serde_json::to_string_pretty(&metadata) {
        let _ = fs::write(playlist_json_path, updated);
    }

    info!(
        "Updated playlist.json: {} tracks, {} bytes, {} track metadata entries",
        track_count,
        total_size,
        tracks_metadata.len()
    );
}
