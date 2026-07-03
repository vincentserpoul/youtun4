use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::{Error, FileSystemError, Result};

/// Check if a file is an audio file based on extension.
#[must_use]
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "mp3" | "m4a" | "mp4" | "wav" | "flac" | "ogg" | "aac"
            )
        })
}

/// Validate a playlist name.
///
/// # Errors
///
/// Returns an error if the name is empty, too long, contains invalid characters,
/// or is a reserved name.
pub fn validate_playlist_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Playlist(crate::error::PlaylistError::InvalidName {
            name: name.to_string(),
            reason: "Playlist name cannot be empty".to_string(),
        }));
    }

    if name.len() > 255 {
        return Err(Error::Playlist(crate::error::PlaylistError::InvalidName {
            name: name.to_string(),
            reason: "Playlist name too long".to_string(),
        }));
    }

    // Check for invalid characters
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
    if name.chars().any(|c| invalid_chars.contains(&c)) {
        return Err(Error::Playlist(crate::error::PlaylistError::InvalidName {
            name: name.to_string(),
            reason: "Playlist name contains invalid characters".to_string(),
        }));
    }

    // Check for reserved names (Windows compatibility)
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&name.to_uppercase().as_str()) {
        return Err(Error::Playlist(crate::error::PlaylistError::InvalidName {
            name: name.to_string(),
            reason: "Playlist name is reserved".to_string(),
        }));
    }

    Ok(())
}

/// Clear all non-hidden contents of a directory.
pub(super) fn clear_directory(path: &Path) -> Result<()> {
    let entries = fs::read_dir(path).map_err(|e| {
        Error::FileSystem(FileSystemError::ReadFailed {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            Error::FileSystem(FileSystemError::ReadFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

        let entry_path = entry.path();
        let file_name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Skip hidden files and system files
        if file_name.starts_with('.') || file_name.eq_ignore_ascii_case("System Volume Information")
        {
            continue;
        }

        if entry_path.is_dir() {
            fs::remove_dir_all(&entry_path).map_err(|e| {
                Error::FileSystem(FileSystemError::DeleteFailed {
                    path: entry_path.clone(),
                    reason: e.to_string(),
                })
            })?;
        } else {
            fs::remove_file(&entry_path).map_err(|e| {
                Error::FileSystem(FileSystemError::DeleteFailed {
                    path: entry_path.clone(),
                    reason: e.to_string(),
                })
            })?;
        }
    }

    Ok(())
}

/// Copy contents of one directory to another.
pub(super) fn copy_directory_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in WalkDir::new(src)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let src_path = entry.path();
        let file_name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Skip metadata file
        if file_name == "playlist.json" {
            continue;
        }

        let dst_path = dst.join(file_name);

        if src_path.is_file() {
            fs::copy(src_path, &dst_path).map_err(|e| {
                Error::FileSystem(FileSystemError::CopyFailed {
                    source_path: src_path.to_path_buf(),
                    destination: dst_path.clone(),
                    reason: e.to_string(),
                })
            })?;
        } else if src_path.is_dir() {
            fs::create_dir_all(&dst_path).map_err(|e| {
                Error::FileSystem(FileSystemError::CreateDirFailed {
                    path: dst_path.clone(),
                    reason: e.to_string(),
                })
            })?;
            copy_directory_contents(src_path, &dst_path)?;
        }
    }

    Ok(())
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
    fn test_validate_playlist_name_empty() {
        let result = validate_playlist_name("");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_playlist_name_invalid_chars() {
        let invalid_names = ["test/name", "test\\name", "test:name", "test*name"];
        for name in invalid_names {
            let result = validate_playlist_name(name);
            assert!(result.is_err(), "Name '{name}' should be invalid");
        }
    }

    #[test]
    fn test_validate_playlist_name_reserved() {
        let result = validate_playlist_name("CON");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_audio_file() {
        assert!(is_audio_file(Path::new("song.mp3")));
        assert!(is_audio_file(Path::new("song.MP3")));
        assert!(is_audio_file(Path::new("song.m4a")));
        assert!(is_audio_file(Path::new("song.flac")));
        assert!(!is_audio_file(Path::new("song.txt")));
        assert!(!is_audio_file(Path::new("song")));
    }

    #[test]
    fn test_validate_playlist_name_valid() {
        validate_playlist_name("My Playlist").unwrap();
        validate_playlist_name("playlist-2024").unwrap();
        validate_playlist_name("Rock & Roll").unwrap();
    }

    #[test]
    fn test_validate_playlist_name_too_long() {
        let long_name = "a".repeat(300);
        let result = validate_playlist_name(&long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_audio_file_various_extensions() {
        // Supported audio formats
        assert!(is_audio_file(Path::new("file.mp3")));
        assert!(is_audio_file(Path::new("file.m4a")));
        assert!(is_audio_file(Path::new("file.mp4"))); // mp4 can contain audio
        assert!(is_audio_file(Path::new("file.wav")));
        assert!(is_audio_file(Path::new("file.flac")));
        assert!(is_audio_file(Path::new("file.ogg")));
        assert!(is_audio_file(Path::new("file.aac")));

        // Case insensitive
        assert!(is_audio_file(Path::new("file.MP3")));
        assert!(is_audio_file(Path::new("file.FLAC")));
        assert!(is_audio_file(Path::new("file.M4A")));

        // Not supported
        assert!(!is_audio_file(Path::new("file.txt")));
        assert!(!is_audio_file(Path::new("file.jpg")));
        assert!(!is_audio_file(Path::new("file.wma"))); // not in current supported list
        assert!(!is_audio_file(Path::new("file.opus"))); // not in current supported list
        assert!(!is_audio_file(Path::new("file")));
    }
}
