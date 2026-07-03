//! Playlist API.

use crate::types::{
    FolderStatistics, FolderValidationResult, Mp3Metadata, PlaylistMetadata, SavedPlaylistMetadata,
    TrackInfo,
};

use super::invoke;

/// List all playlists.
pub async fn list_playlists() -> Result<Vec<PlaylistMetadata>, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    invoke("list_playlists", Args {}).await
}

/// Create a new playlist.
pub async fn create_playlist(name: &str, source_url: Option<&str>) -> Result<String, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        name: &'a str,
        source_url: Option<&'a str>,
        thumbnail_url: Option<&'a str>,
    }

    invoke(
        "create_playlist",
        Args {
            name,
            source_url,
            thumbnail_url: None,
        },
    )
    .await
}

/// Delete a playlist.
pub async fn delete_playlist(name: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("delete_playlist", Args { name }).await
}

/// Sync a playlist to a device.
pub async fn sync_playlist(playlist_name: &str, device_mount_point: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        playlist_name: &'a str,
        device_mount_point: &'a str,
    }

    invoke(
        "sync_playlist",
        Args {
            playlist_name,
            device_mount_point,
        },
    )
    .await
}

/// Get tracks for a playlist with MP3 metadata.
pub async fn get_playlist_tracks(name: &str) -> Result<Vec<TrackInfo>, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_tracks", Args { name }).await
}

/// Get tracks for a playlist without metadata extraction (faster).
pub async fn get_playlist_tracks_fast(name: &str) -> Result<Vec<TrackInfo>, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_tracks_fast", Args { name }).await
}

/// Extract MP3 metadata (ID3 tags) from a single file.
///
/// Returns metadata including title, artist, album, duration, track number, etc.
/// If the file has no tags or is not a valid MP3, returns empty metadata.
pub async fn extract_track_metadata(path: &str) -> Result<Mp3Metadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        path: &'a str,
    }

    invoke("extract_track_metadata", Args { path }).await
}

/// Get detailed metadata for a specific playlist.
///
/// Returns playlist metadata including name, source URL, creation time,
/// modification time, track count, and total size.
pub async fn get_playlist_details(name: &str) -> Result<PlaylistMetadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_details", Args { name }).await
}

/// Validate a playlist folder structure.
///
/// Checks if the folder exists, has valid metadata, and contains audio files.
/// Returns a validation result with details about any issues found.
pub async fn validate_playlist_folder(name: &str) -> Result<FolderValidationResult, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("validate_playlist_folder", Args { name }).await
}

/// Get statistics about a playlist folder.
///
/// Returns information about file counts, sizes, and metadata status.
pub async fn get_playlist_statistics(name: &str) -> Result<FolderStatistics, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_statistics", Args { name }).await
}

/// Get the folder path for a playlist.
///
/// Returns the absolute path to the playlist folder on the filesystem.
pub async fn get_playlist_folder_path(name: &str) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_folder_path", Args { name }).await
}

/// Open the playlist folder in the system file manager.
///
/// This invokes the Tauri command that uses the opener plugin to open the folder.
pub async fn open_playlist_folder(name: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("open_playlist_folder", Args { name }).await
}

/// Repair a playlist folder by fixing common issues.
///
/// Currently this creates missing metadata files and fixes corrupted metadata.
/// Returns a list of repairs that were made.
pub async fn repair_playlist_folder(name: &str) -> Result<Vec<String>, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("repair_playlist_folder", Args { name }).await
}

/// Import an existing folder as a playlist.
///
/// Creates metadata for a folder that already contains audio files.
/// The folder must be in the playlists directory.
pub async fn import_playlist_folder(
    folder_name: &str,
    source_url: Option<&str>,
) -> Result<String, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        folder_name: &'a str,
        source_url: Option<&'a str>,
    }

    invoke(
        "import_playlist_folder",
        Args {
            folder_name,
            source_url,
        },
    )
    .await
}

/// Rename a playlist.
///
/// This renames the playlist folder and updates any metadata as needed.
pub async fn rename_playlist(old_name: &str, new_name: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        old_name: &'a str,
        new_name: &'a str,
    }

    invoke("rename_playlist", Args { old_name, new_name }).await
}

/// Check if a playlist exists.
pub async fn playlist_exists(name: &str) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("playlist_exists", Args { name }).await
}

/// Ensure a playlist folder has proper structure.
///
/// Creates the metadata file if it doesn't exist.
pub async fn ensure_playlist_structure(name: &str) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("ensure_playlist_structure", Args { name }).await
}

/// Get the saved metadata for a playlist.
///
/// Returns the raw metadata stored in playlist.json, including title,
/// description, source URL, timestamps, track count, and total size.
pub async fn get_playlist_saved_metadata(name: &str) -> Result<SavedPlaylistMetadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("get_playlist_saved_metadata", Args { name }).await
}

/// Update playlist metadata.
///
/// Updates the playlist.json file with new metadata values.
/// Pass `None` for fields that should not be changed.
///
/// # Arguments
///
/// * `name` - Playlist name
/// * `title` - New title (None to keep existing, Some("") to clear)
/// * `description` - New description (None to keep existing, Some("") to clear)
/// * `source_url` - New source URL (None to keep existing, Some(None) to clear, Some(Some(url)) to set)
pub async fn update_playlist_metadata(
    name: &str,
    title: Option<&str>,
    description: Option<&str>,
    source_url: Option<Option<&str>>,
) -> Result<SavedPlaylistMetadata, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        name: &'a str,
        title: Option<&'a str>,
        description: Option<&'a str>,
        source_url: Option<Option<&'a str>>,
    }

    invoke(
        "update_playlist_metadata",
        Args {
            name,
            title,
            description,
            source_url,
        },
    )
    .await
}

/// Refresh the cached track count and total size for a playlist.
///
/// Scans the playlist folder and updates the `track_count` and `total_size_bytes`
/// fields in the metadata file.
pub async fn refresh_playlist_stats(name: &str) -> Result<SavedPlaylistMetadata, String> {
    #[derive(serde::Serialize)]
    struct Args<'a> {
        name: &'a str,
    }

    invoke("refresh_playlist_stats", Args { name }).await
}
