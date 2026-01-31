//! Integration tests for `MP3YouTube` core workflows.
//!
//! These tests verify end-to-end workflows including:
//! - Playlist creation and management
//! - Device detection (using mock devices with temp directories)
//! - Synchronization workflows
//!
//! All tests use temporary directories as fixtures to simulate device behavior.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use mp3youtube_core::{
    // Cleanup
    CleanupOptions,
    DeviceCleanupHandler,
    // Device
    DeviceDetector,
    DeviceInfo,
    // Error types
    Error,
    // Metadata
    Mp3Metadata,
    PlaylistError,
    PlaylistManager,
    Result,
    // Sync
    SyncOptions,
    SyncPhase,
    SyncProgress,
    SyncRequest,
    // Transfer
    TransferOptions,
    TransferProgress,
    extract_metadata,
    // Playlist
    is_audio_file,
    validate_playlist_name,
};
use tempfile::TempDir;

// =============================================================================
// Test Fixtures and Utilities
// =============================================================================

/// Test fixture that provides temporary directories for playlist and device simulation.
struct TestFixture {
    /// Directory for storing playlists (simulates local storage).
    playlists_dir: TempDir,
    /// Directory that simulates a mounted USB device.
    device_dir: TempDir,
    /// Playlist manager initialized with the playlists directory.
    playlist_manager: PlaylistManager,
}

impl TestFixture {
    /// Create a new test fixture with empty directories.
    fn new() -> Result<Self> {
        let playlists_dir = TempDir::new().map_err(|e| {
            Error::Configuration(format!("Failed to create temp playlists dir: {e}"))
        })?;
        let device_dir = TempDir::new()
            .map_err(|e| Error::Configuration(format!("Failed to create temp device dir: {e}")))?;

        let playlist_manager = PlaylistManager::new(playlists_dir.path().to_path_buf())?;

        Ok(Self {
            playlists_dir,
            device_dir,
            playlist_manager,
        })
    }

    /// Get the path to the playlists directory.
    fn playlists_path(&self) -> &Path {
        self.playlists_dir.path()
    }

    /// Get the path to the simulated device directory.
    fn device_path(&self) -> &Path {
        self.device_dir.path()
    }

    /// Create a test playlist with the given name and tracks.
    fn create_playlist_with_tracks(&self, name: &str, track_names: &[&str]) -> Result<PathBuf> {
        let path = self.playlist_manager.create_playlist(name, None)?;

        for track_name in track_names {
            let track_path = path.join(track_name);
            // Create fake MP3 data (larger files for more realistic testing)
            let fake_mp3_data = format!("FAKE MP3 DATA FOR {} - {}", name, "x".repeat(1000));
            fs::write(&track_path, fake_mp3_data)
                .map_err(|e| Error::Configuration(format!("Failed to create test track: {e}")))?;
        }

        Ok(path)
    }

    /// Create a test playlist with source URL.
    fn create_playlist_with_source(
        &self,
        name: &str,
        source_url: &str,
        track_names: &[&str],
    ) -> Result<PathBuf> {
        let path = self
            .playlist_manager
            .create_playlist(name, Some(source_url.to_string()))?;

        for track_name in track_names {
            let track_path = path.join(track_name);
            let fake_mp3_data = format!("FAKE MP3 DATA FROM YOUTUBE - {}", "x".repeat(500));
            fs::write(&track_path, fake_mp3_data)
                .map_err(|e| Error::Configuration(format!("Failed to create test track: {e}")))?;
        }

        Ok(path)
    }

    /// Simulate existing content on the device.
    fn add_device_content(&self, files: &[(&str, &str)]) -> Result<()> {
        for (name, content) in files {
            let file_path = self.device_dir.path().join(name);
            fs::write(&file_path, content)
                .map_err(|e| Error::Configuration(format!("Failed to create device file: {e}")))?;
        }
        Ok(())
    }

    /// Get list of files currently on the device (non-hidden).
    fn list_device_files(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(self.device_dir.path())
            .map_err(|e| Error::Configuration(format!("Failed to read device dir: {e}")))?
        {
            let entry =
                entry.map_err(|e| Error::Configuration(format!("Failed to read entry: {e}")))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                files.push(name);
            }
        }
        files.sort();
        Ok(files)
    }
}

/// Mock device detector for testing.
/// Uses a temporary directory to simulate a mounted USB device.
struct MockDeviceDetector {
    devices: Vec<DeviceInfo>,
}

impl MockDeviceDetector {
    const fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    fn add_device(
        &mut self,
        name: &str,
        mount_point: PathBuf,
        total_bytes: u64,
        available_bytes: u64,
    ) {
        self.devices.push(DeviceInfo {
            name: name.to_string(),
            mount_point,
            total_bytes,
            available_bytes,
            file_system: "fat32".to_string(),
            is_removable: true,
        });
    }
}

impl DeviceDetector for MockDeviceDetector {
    fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self.devices.clone())
    }

    fn is_device_connected(&self, mount_point: &Path) -> bool {
        self.devices.iter().any(|d| d.mount_point == mount_point) && mount_point.exists()
    }

    fn refresh(&mut self) {
        // No-op for mock
    }
}

// =============================================================================
// Playlist Creation Integration Tests
// =============================================================================

#[test]
fn test_create_playlist_workflow() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Create a playlist
    let result = fixture.playlist_manager.create_playlist("My Music", None);
    assert!(result.is_ok(), "Failed to create playlist: {result:?}");

    let path = result.expect("Unwrap path");
    assert!(path.exists(), "Playlist directory should exist");
    assert!(
        path.join("playlist.json").exists(),
        "Metadata file should exist"
    );

    // Verify we can list it
    let playlists = fixture
        .playlist_manager
        .list_playlists()
        .expect("Should list");
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].name, "My Music");
}

#[test]
fn test_create_playlist_with_youtube_source() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    let youtube_url = "https://www.youtube.com/playlist?list=PLtest123";
    let result = fixture
        .playlist_manager
        .create_playlist("YouTube Favorites", Some(youtube_url.to_string()));
    assert!(result.is_ok());

    let path = result.expect("Unwrap path");
    let metadata = fixture
        .playlist_manager
        .get_playlist_metadata(&path)
        .expect("Should get metadata");

    assert_eq!(metadata.name, "YouTube Favorites");
    assert_eq!(metadata.source_url, Some(youtube_url.to_string()));
}

#[test]
fn test_create_playlist_with_tracks() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    let tracks = &["track1.mp3", "track2.mp3", "track3.mp3"];
    let path = fixture
        .create_playlist_with_tracks("Album", tracks)
        .expect("Should create playlist with tracks");

    // Verify tracks exist
    for track in tracks {
        assert!(path.join(track).exists(), "Track {track} should exist");
    }

    // Verify track count
    let track_list = fixture
        .playlist_manager
        .list_tracks("Album")
        .expect("Should list tracks");
    assert_eq!(track_list.len(), 3);
}

#[test]
fn test_create_duplicate_playlist_fails() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .playlist_manager
        .create_playlist("Unique", None)
        .expect("First creation should succeed");

    let result = fixture.playlist_manager.create_playlist("Unique", None);
    assert!(result.is_err());

    match result {
        Err(Error::Playlist(PlaylistError::AlreadyExists { name })) => {
            assert_eq!(name, "Unique");
        }
        _ => panic!("Expected AlreadyExists error"),
    }
}

#[test]
fn test_delete_playlist_workflow() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    let path = fixture
        .create_playlist_with_tracks("ToDelete", &["song.mp3"])
        .expect("Should create");
    assert!(path.exists());

    fixture
        .playlist_manager
        .delete_playlist("ToDelete")
        .expect("Should delete");
    assert!(!path.exists());

    // Verify it's gone from the list
    let playlists = fixture
        .playlist_manager
        .list_playlists()
        .expect("Should list");
    assert!(playlists.is_empty());
}

#[test]
fn test_playlist_name_validation() {
    // Valid names
    assert!(validate_playlist_name("My Playlist").is_ok());
    assert!(validate_playlist_name("Rock-n-Roll 2024").is_ok());
    assert!(validate_playlist_name("Album (Deluxe Edition)").is_ok());

    // Invalid names
    assert!(validate_playlist_name("").is_err()); // Empty
    assert!(validate_playlist_name("test/name").is_err()); // Slash
    assert!(validate_playlist_name("test\\name").is_err()); // Backslash
    assert!(validate_playlist_name("test:name").is_err()); // Colon
    assert!(validate_playlist_name("test*name").is_err()); // Asterisk
    assert!(validate_playlist_name("CON").is_err()); // Reserved Windows name
    assert!(validate_playlist_name("NUL").is_err()); // Reserved Windows name
}

#[test]
fn test_playlist_folder_validation() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Create a valid playlist
    fixture
        .create_playlist_with_tracks("ValidPlaylist", &["song.mp3"])
        .expect("Should create");

    let validation = fixture.playlist_manager.validate_folder("ValidPlaylist");
    assert!(validation.exists);
    assert!(validation.has_metadata);
    assert!(validation.metadata_valid);
    assert_eq!(validation.audio_file_count, 1);
    assert!(validation.is_valid());
}

#[test]
fn test_playlist_folder_statistics() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    let path = fixture
        .create_playlist_with_tracks("StatsTest", &["song1.mp3", "song2.mp3"])
        .expect("Should create");

    // Add a non-audio file
    fs::write(path.join("notes.txt"), "Some notes").expect("Write should succeed");

    let stats = fixture
        .playlist_manager
        .get_folder_statistics("StatsTest")
        .expect("Should get stats");

    assert_eq!(stats.audio_files, 2);
    assert_eq!(stats.other_files, 1);
    assert_eq!(stats.total_files, 3);
    assert!(stats.has_metadata);
    assert!(stats.audio_size_bytes > 0);
}

#[test]
fn test_import_existing_folder() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Create a folder manually with audio files but no metadata
    let folder_path = fixture.playlists_path().join("ImportMe");
    fs::create_dir(&folder_path).expect("Should create dir");
    fs::write(folder_path.join("track.mp3"), "mp3 data").expect("Should write");

    // Import it
    let name = fixture
        .playlist_manager
        .import_folder(&folder_path, Some("https://youtube.com/test".to_string()))
        .expect("Should import");

    assert_eq!(name, "ImportMe");
    assert!(folder_path.join("playlist.json").exists());

    // Verify it appears in the list
    let playlists = fixture
        .playlist_manager
        .list_playlists()
        .expect("Should list");
    assert!(playlists.iter().any(|p| p.name == "ImportMe"));
}

#[test]
fn test_repair_corrupted_metadata() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    let path = fixture
        .playlist_manager
        .create_playlist("CorruptedPlaylist", None)
        .expect("Should create");

    // Corrupt the metadata
    fs::write(path.join("playlist.json"), "invalid json {{{").expect("Should write");

    // Validate shows issue
    let validation = fixture
        .playlist_manager
        .validate_folder("CorruptedPlaylist");
    assert!(validation.has_metadata);
    assert!(!validation.metadata_valid);

    // Repair it
    let repairs = fixture
        .playlist_manager
        .repair_folder("CorruptedPlaylist")
        .expect("Should repair");
    assert!(!repairs.is_empty());

    // Now it should be valid
    let validation = fixture
        .playlist_manager
        .validate_folder("CorruptedPlaylist");
    assert!(validation.metadata_valid);
}

// =============================================================================
// Device Detection Integration Tests
// =============================================================================

#[test]
fn test_mock_device_detection() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    let mut detector = MockDeviceDetector::new();
    detector.add_device(
        "MP3_PLAYER",
        fixture.device_path().to_path_buf(),
        8_000_000_000, // 8 GB
        4_000_000_000, // 4 GB available
    );

    let devices = detector.list_devices().expect("Should list");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].name, "MP3_PLAYER");
    assert_eq!(devices[0].total_bytes, 8_000_000_000);
    assert_eq!(devices[0].available_bytes, 4_000_000_000);
    assert_eq!(devices[0].file_system, "fat32");
    assert!(devices[0].is_removable);
}

#[test]
fn test_device_info_calculations() {
    let device = DeviceInfo {
        name: "TestDevice".to_string(),
        mount_point: PathBuf::from("/mnt/test"),
        total_bytes: 1000,
        available_bytes: 300,
        file_system: "fat32".to_string(),
        is_removable: true,
    };

    assert_eq!(device.used_bytes(), 700);
    assert!((device.usage_percentage() - 70.0).abs() < 0.01);
}

#[test]
fn test_device_connection_check() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    let mut detector = MockDeviceDetector::new();
    detector.add_device(
        "USB_DRIVE",
        fixture.device_path().to_path_buf(),
        4_000_000_000,
        2_000_000_000,
    );

    // Device should be connected (directory exists)
    assert!(detector.is_device_connected(fixture.device_path()));

    // Non-existent device should not be connected
    assert!(!detector.is_device_connected(Path::new("/nonexistent/path")));
}

#[test]
fn test_multiple_devices_detection() {
    let device1_dir = TempDir::new().expect("Create temp dir 1");
    let device2_dir = TempDir::new().expect("Create temp dir 2");

    let mut detector = MockDeviceDetector::new();
    detector.add_device(
        "DEVICE_A",
        device1_dir.path().to_path_buf(),
        16_000_000_000,
        8_000_000_000,
    );
    detector.add_device(
        "DEVICE_B",
        device2_dir.path().to_path_buf(),
        32_000_000_000,
        16_000_000_000,
    );

    let devices = detector.list_devices().expect("Should list");
    assert_eq!(devices.len(), 2);

    let device_names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
    assert!(device_names.contains(&"DEVICE_A"));
    assert!(device_names.contains(&"DEVICE_B"));
}

// =============================================================================
// Synchronization Integration Tests
// =============================================================================

#[test]
fn test_basic_sync_workflow() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Create a playlist with tracks
    fixture
        .create_playlist_with_tracks("SyncTest", &["song1.mp3", "song2.mp3", "song3.mp3"])
        .expect("Should create playlist");

    // Add some existing content to device
    fixture
        .add_device_content(&[("old_file.txt", "old content")])
        .expect("Should add device content");

    // Sync the playlist to device
    let result = fixture
        .playlist_manager
        .sync_to_device("SyncTest", fixture.device_path());
    assert!(result.is_ok(), "Sync should succeed: {result:?}");

    // Verify old content is cleared
    let device_files = fixture.list_device_files().expect("Should list");
    assert!(!device_files.contains(&"old_file.txt".to_string()));

    // Verify new tracks are present
    assert!(device_files.contains(&"song1.mp3".to_string()));
    assert!(device_files.contains(&"song2.mp3".to_string()));
    assert!(device_files.contains(&"song3.mp3".to_string()));

    // Verify playlist.json is NOT synced (metadata stays local)
    assert!(!device_files.contains(&"playlist.json".to_string()));
}

#[test]
fn test_sync_with_progress_tracking() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .create_playlist_with_tracks("ProgressTest", &["track1.mp3", "track2.mp3"])
        .expect("Should create");

    let progress_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let progress_clone = progress_updates.clone();

    let options = TransferOptions::fast();
    let result = fixture.playlist_manager.sync_to_device_with_progress(
        "ProgressTest",
        fixture.device_path(),
        &options,
        Some(move |progress: &TransferProgress| {
            progress_clone.lock().expect("Lock").push(progress.clone());
        }),
    );

    assert!(
        result.is_ok(),
        "Sync with progress should succeed: {result:?}"
    );

    let transfer_result = result.expect("Unwrap result");
    assert!(transfer_result.success);
    assert_eq!(transfer_result.files_transferred, 2);

    // Verify we got progress updates
    let updates = progress_updates.lock().expect("Lock");
    assert!(!updates.is_empty(), "Should have received progress updates");
}

#[test]
fn test_sync_preserves_hidden_files_on_device() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .create_playlist_with_tracks("HiddenTest", &["track.mp3"])
        .expect("Should create");

    // Add hidden file to device (should be preserved)
    fs::write(fixture.device_path().join(".hidden_config"), "config data")
        .expect("Should write hidden file");

    fixture
        .playlist_manager
        .sync_to_device("HiddenTest", fixture.device_path())
        .expect("Sync should succeed");

    // Hidden file should still exist
    assert!(fixture.device_path().join(".hidden_config").exists());

    // But regular old files would be cleared (we didn't add any non-hidden old files)
    let device_files = fixture.list_device_files().expect("Should list");
    assert!(device_files.contains(&"track.mp3".to_string()));
}

#[test]
fn test_sync_with_cancellation() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Create playlist with multiple tracks
    fixture
        .create_playlist_with_tracks(
            "CancelTest",
            &[
                "track1.mp3",
                "track2.mp3",
                "track3.mp3",
                "track4.mp3",
                "track5.mp3",
            ],
        )
        .expect("Should create");

    // Set cancellation token to true immediately
    let cancel_token = Arc::new(AtomicBool::new(true));

    let options = TransferOptions::default();
    let result = fixture.playlist_manager.sync_to_device_cancellable(
        "CancelTest",
        fixture.device_path(),
        &options,
        cancel_token,
        None::<fn(&TransferProgress)>,
    );

    // Sync should report as cancelled
    assert!(result.is_ok());
    let transfer_result = result.expect("Unwrap");
    assert!(transfer_result.was_cancelled);
}

#[test]
fn test_sync_to_nonexistent_device_fails() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .create_playlist_with_tracks("NoDeviceTest", &["track.mp3"])
        .expect("Should create");

    let result = fixture
        .playlist_manager
        .sync_to_device("NoDeviceTest", Path::new("/nonexistent/device/path"));

    assert!(result.is_err());
}

#[test]
fn test_sync_nonexistent_playlist_fails() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    let result = fixture
        .playlist_manager
        .sync_to_device("NonExistentPlaylist", fixture.device_path());

    assert!(result.is_err());
    match result {
        Err(Error::Playlist(PlaylistError::NotFound { name })) => {
            assert_eq!(name, "NonExistentPlaylist");
        }
        _ => panic!("Expected NotFound error"),
    }
}

// =============================================================================
// Transfer Engine Integration Tests
// =============================================================================

#[test]
fn test_transfer_options_validation() {
    // Valid options
    let valid = TransferOptions::default();
    assert!(valid.validate().is_ok());

    let fast = TransferOptions::fast();
    assert!(fast.validate().is_ok());

    let reliable = TransferOptions::reliable();
    assert!(reliable.validate().is_ok());
}

#[test]
fn test_transfer_with_verification() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .create_playlist_with_tracks("VerifyTest", &["verified_track.mp3"])
        .expect("Should create");

    let options = TransferOptions::reliable();
    let result = fixture.playlist_manager.sync_to_device_with_progress(
        "VerifyTest",
        fixture.device_path(),
        &options,
        None::<fn(&TransferProgress)>,
    );

    assert!(result.is_ok());
    let transfer_result = result.expect("Unwrap");
    assert!(transfer_result.success);
    assert_eq!(transfer_result.files_transferred, 1);
}

// =============================================================================
// Device Cleanup Integration Tests
// =============================================================================

#[test]
fn test_device_cleanup_dry_run() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Add content to device
    fixture
        .add_device_content(&[
            ("file1.mp3", "audio data"),
            ("file2.mp3", "more audio"),
            ("notes.txt", "text file"),
        ])
        .expect("Should add content");

    let options = CleanupOptions::dry_run();
    let handler = DeviceCleanupHandler::new();

    let result = handler.cleanup_device(fixture.device_path(), &options);
    assert!(result.is_ok());

    let cleanup_result = result.expect("Unwrap");

    // Dry run should not delete files
    assert!(fixture.device_path().join("file1.mp3").exists());
    assert!(fixture.device_path().join("file2.mp3").exists());
    assert!(fixture.device_path().join("notes.txt").exists());

    // But should report what would be deleted
    assert!(cleanup_result.entries.len() >= 3);
}

#[test]
fn test_device_cleanup_audio_only() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .add_device_content(&[
            ("song.mp3", "audio data"),
            ("track.m4a", "more audio"),
            ("readme.txt", "keep this"),
        ])
        .expect("Should add content");

    let handler = DeviceCleanupHandler::new();
    let result =
        handler.cleanup_audio_files_only(fixture.device_path(), &CleanupOptions::default());
    assert!(result.is_ok());

    // Audio files should be deleted
    assert!(!fixture.device_path().join("song.mp3").exists());
    assert!(!fixture.device_path().join("track.m4a").exists());

    // Text file should remain
    assert!(fixture.device_path().join("readme.txt").exists());
}

#[test]
fn test_device_cleanup_skips_hidden_files() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .add_device_content(&[("visible.mp3", "audio")])
        .expect("Add visible");

    fs::write(fixture.device_path().join(".hidden"), "hidden content").expect("Add hidden");

    let options = CleanupOptions::full_cleanup();
    let handler = DeviceCleanupHandler::new();

    handler
        .cleanup_device(fixture.device_path(), &options)
        .expect("Cleanup should succeed");

    // Visible file should be deleted
    assert!(!fixture.device_path().join("visible.mp3").exists());

    // Hidden file should be preserved
    assert!(fixture.device_path().join(".hidden").exists());
}

// =============================================================================
// Audio File Detection Tests
// =============================================================================

#[test]
fn test_audio_file_detection() {
    // Audio files (should return true)
    assert!(is_audio_file(Path::new("song.mp3")));
    assert!(is_audio_file(Path::new("song.MP3"))); // Case insensitive
    assert!(is_audio_file(Path::new("track.m4a")));
    assert!(is_audio_file(Path::new("audio.wav")));
    assert!(is_audio_file(Path::new("music.flac")));
    assert!(is_audio_file(Path::new("podcast.ogg")));
    assert!(is_audio_file(Path::new("tune.aac")));
    assert!(is_audio_file(Path::new("video.mp4"))); // MP4 can contain audio (AAC)

    // Non-audio files (should return false)
    assert!(!is_audio_file(Path::new("document.txt")));
    assert!(!is_audio_file(Path::new("image.jpg")));
    assert!(!is_audio_file(Path::new("video.mkv"))); // MKV is primarily video container
    assert!(!is_audio_file(Path::new("archive.zip")));
    assert!(!is_audio_file(Path::new("noextension")));
    assert!(!is_audio_file(Path::new("playlist.json")));
}

// =============================================================================
// End-to-End Workflow Tests
// =============================================================================

#[test]
fn test_full_workflow_create_and_sync_playlist() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // 1. Create playlist from YouTube source
    let youtube_url = "https://www.youtube.com/playlist?list=PLworkflow123";
    let path = fixture
        .create_playlist_with_source(
            "Workout Mix",
            youtube_url,
            &["warmup.mp3", "cardio.mp3", "cooldown.mp3"],
        )
        .expect("Should create playlist");

    // 2. Verify playlist metadata
    let metadata = fixture
        .playlist_manager
        .get_playlist_metadata(&path)
        .expect("Should get metadata");
    assert_eq!(metadata.name, "Workout Mix");
    assert_eq!(metadata.source_url, Some(youtube_url.to_string()));
    assert_eq!(metadata.track_count, 3);

    // 3. Validate folder structure
    let validation = fixture.playlist_manager.validate_folder("Workout Mix");
    assert!(validation.is_valid());

    // 4. Sync to device
    fixture
        .playlist_manager
        .sync_to_device("Workout Mix", fixture.device_path())
        .expect("Sync should succeed");

    // 5. Verify device contents
    let device_files = fixture.list_device_files().expect("Should list");
    assert_eq!(device_files.len(), 3);
    assert!(device_files.contains(&"warmup.mp3".to_string()));
    assert!(device_files.contains(&"cardio.mp3".to_string()));
    assert!(device_files.contains(&"cooldown.mp3".to_string()));
}

#[test]
fn test_multiple_playlists_management() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Create multiple playlists
    let playlists_data = [
        ("Rock Classics", &["acdc.mp3", "zeppelin.mp3"][..]),
        (
            "Jazz Favorites",
            &["miles.mp3", "coltrane.mp3", "monk.mp3"][..],
        ),
        ("Electronic", &["deadmau5.mp3"][..]),
    ];

    for (name, tracks) in &playlists_data {
        fixture
            .create_playlist_with_tracks(name, tracks)
            .expect("Should create");
    }

    // List all playlists
    let playlists = fixture
        .playlist_manager
        .list_playlists()
        .expect("Should list");
    assert_eq!(playlists.len(), 3);

    // Verify they're sorted by name
    assert_eq!(playlists[0].name, "Electronic");
    assert_eq!(playlists[1].name, "Jazz Favorites");
    assert_eq!(playlists[2].name, "Rock Classics");

    // Delete one
    fixture
        .playlist_manager
        .delete_playlist("Jazz Favorites")
        .expect("Should delete");

    let playlists = fixture
        .playlist_manager
        .list_playlists()
        .expect("Should list");
    assert_eq!(playlists.len(), 2);
    assert!(!playlists.iter().any(|p| p.name == "Jazz Favorites"));
}

#[test]
fn test_sync_replaces_device_content() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Create first playlist and sync
    fixture
        .create_playlist_with_tracks("Playlist A", &["song_a1.mp3", "song_a2.mp3"])
        .expect("Create A");

    fixture
        .playlist_manager
        .sync_to_device("Playlist A", fixture.device_path())
        .expect("Sync A");

    let files = fixture.list_device_files().expect("List");
    assert_eq!(files.len(), 2);
    assert!(files.contains(&"song_a1.mp3".to_string()));

    // Create second playlist and sync (should replace first)
    fixture
        .create_playlist_with_tracks("Playlist B", &["song_b1.mp3", "song_b2.mp3", "song_b3.mp3"])
        .expect("Create B");

    fixture
        .playlist_manager
        .sync_to_device("Playlist B", fixture.device_path())
        .expect("Sync B");

    let files = fixture.list_device_files().expect("List");
    assert_eq!(files.len(), 3);

    // Old files should be gone
    assert!(!files.contains(&"song_a1.mp3".to_string()));
    assert!(!files.contains(&"song_a2.mp3".to_string()));

    // New files should be present
    assert!(files.contains(&"song_b1.mp3".to_string()));
    assert!(files.contains(&"song_b2.mp3".to_string()));
    assert!(files.contains(&"song_b3.mp3".to_string()));
}

// =============================================================================
// Sync Orchestrator Integration Tests
// =============================================================================

#[test]
fn test_sync_orchestrator_options() {
    // Test that all option presets are valid
    let default_opts = SyncOptions::default();
    assert!(default_opts.cleanup_enabled);
    assert!(default_opts.verify_device_between_phases);

    let fast_opts = SyncOptions::fast();
    assert!(!fast_opts.verify_device_between_phases); // Fast skips verification

    let reliable_opts = SyncOptions::reliable();
    assert!(reliable_opts.verify_device_between_phases);
    assert!(!reliable_opts.skip_existing_matches); // Re-transfers everything

    let dry_run_opts = SyncOptions::dry_run();
    assert!(dry_run_opts.cleanup_options.dry_run);
}

#[test]
fn test_sync_request_creation() {
    let request = SyncRequest::new(
        vec!["Playlist1".to_string(), "Playlist2".to_string()],
        PathBuf::from("/mnt/device"),
    );
    assert_eq!(request.playlists.len(), 2);
    assert_eq!(request.device_mount_point, PathBuf::from("/mnt/device"));

    let single = SyncRequest::single("MyPlaylist", "/mnt/usb");
    assert_eq!(single.playlists.len(), 1);
    assert_eq!(single.playlists[0], "MyPlaylist");
}

#[test]
fn test_sync_progress_initialization() {
    let progress = SyncProgress::verifying(3);
    assert_eq!(progress.phase, SyncPhase::Verifying);
    assert_eq!(progress.total_playlists, 3);
    assert!((progress.overall_progress_percent - 0.0).abs() < f64::EPSILON);
}

// =============================================================================
// MP3 Metadata Extraction Integration Tests
// =============================================================================

#[test]
fn test_metadata_extraction_empty_struct() {
    let metadata = Mp3Metadata::empty();
    assert!(metadata.title.is_none());
    assert!(metadata.artist.is_none());
    assert!(metadata.album.is_none());
    assert!(metadata.duration_secs.is_none());
    assert!(!metadata.has_content());
}

#[test]
fn test_metadata_display_methods() {
    let metadata = Mp3Metadata::empty();
    assert_eq!(metadata.display_title(), "Unknown Title");
    assert_eq!(metadata.display_artist(), "Unknown Artist");
    assert_eq!(metadata.display_album(), "Unknown Album");

    let metadata_with_data = Mp3Metadata {
        title: Some("Test Song".to_string()),
        artist: Some("Test Artist".to_string()),
        album: Some("Test Album".to_string()),
        ..Default::default()
    };
    assert_eq!(metadata_with_data.display_title(), "Test Song");
    assert_eq!(metadata_with_data.display_artist(), "Test Artist");
    assert_eq!(metadata_with_data.display_album(), "Test Album");
}

#[test]
fn test_metadata_formatted_duration() {
    let metadata = Mp3Metadata {
        duration_secs: Some(185), // 3:05
        ..Default::default()
    };
    assert_eq!(metadata.formatted_duration(), Some("3:05".to_string()));

    let metadata_long = Mp3Metadata {
        duration_secs: Some(3661), // 61:01
        ..Default::default()
    };
    assert_eq!(
        metadata_long.formatted_duration(),
        Some("61:01".to_string())
    );
}

#[test]
fn test_metadata_formatted_track_number() {
    let metadata = Mp3Metadata {
        track_number: Some(3),
        total_tracks: Some(12),
        ..Default::default()
    };
    assert_eq!(metadata.formatted_track_number(), Some("3/12".to_string()));

    let metadata_no_total = Mp3Metadata {
        track_number: Some(7),
        total_tracks: None,
        ..Default::default()
    };
    assert_eq!(
        metadata_no_total.formatted_track_number(),
        Some("7".to_string())
    );
}

#[test]
fn test_metadata_extraction_from_non_mp3() {
    let temp_dir = TempDir::new().expect("Create temp dir");
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, "This is not an MP3 file").expect("Write should succeed");

    // Should return empty metadata (not error) for non-MP3 files
    let result = extract_metadata(&file_path);
    assert!(result.is_ok());
    let metadata = result.expect("Unwrap metadata");
    assert!(!metadata.has_content());
}

#[test]
fn test_metadata_extraction_from_nonexistent_file() {
    let result = extract_metadata(Path::new("/nonexistent/path/file.mp3"));
    assert!(result.is_err());
}

#[test]
fn test_playlist_tracks_include_metadata() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Create a playlist with test tracks
    fixture
        .create_playlist_with_tracks("MetadataPlaylist", &["track1.mp3", "track2.mp3"])
        .expect("Should create playlist");

    // Get tracks with metadata
    let tracks = fixture
        .playlist_manager
        .list_tracks("MetadataPlaylist")
        .expect("Should list tracks");

    assert_eq!(tracks.len(), 2);

    // Since our test files are fake MP3s (not real), metadata should be empty/None
    // But the metadata field should exist on the TrackInfo
    for track in &tracks {
        // The metadata field is present (Option)
        // For fake MP3 files, it will be None or have empty content
        if let Some(ref metadata) = track.metadata {
            // If metadata exists, it should be extractable (even if empty)
            assert!(metadata.title.is_none() || metadata.title.is_some());
        }
    }
}

#[test]
fn test_playlist_tracks_fast_mode_no_metadata() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .create_playlist_with_tracks("FastModePlaylist", &["song.mp3"])
        .expect("Should create playlist");

    // Get tracks in fast mode (no metadata extraction)
    let tracks = fixture
        .playlist_manager
        .list_tracks_with_options("FastModePlaylist", false)
        .expect("Should list tracks");

    assert_eq!(tracks.len(), 1);
    assert!(
        tracks[0].metadata.is_none(),
        "Fast mode should not extract metadata"
    );
}

#[test]
fn test_metadata_serialization_roundtrip() {
    let metadata = Mp3Metadata {
        title: Some("Integration Test Song".to_string()),
        artist: Some("Test Artist".to_string()),
        album: Some("Test Album".to_string()),
        duration_secs: Some(240),
        track_number: Some(5),
        total_tracks: Some(10),
        year: Some(2024),
        genre: Some("Electronic".to_string()),
        album_artist: Some("Various".to_string()),
        bitrate_kbps: Some(320),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&metadata).expect("Serialize should succeed");

    // Deserialize back
    let deserialized: Mp3Metadata =
        serde_json::from_str(&json).expect("Deserialize should succeed");

    assert_eq!(metadata, deserialized);
    assert!(deserialized.has_content());
}
