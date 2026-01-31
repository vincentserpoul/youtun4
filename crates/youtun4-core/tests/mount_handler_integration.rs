//! Integration tests for the `DeviceMountHandler` functionality.
//!
//! These tests verify that the mount handler correctly detects and manages
//! USB device mount status on the current platform.

use std::path::PathBuf;
use youtun4_core::device::{DeviceMountHandler, PlatformMountHandler};

/// Test that the `PlatformMountHandler` can be created and reports the correct platform.
#[test]
fn test_platform_mount_handler_reports_platform() {
    let handler = PlatformMountHandler::new();
    let platform = handler.platform();

    // Should match the compile-time target OS
    #[cfg(target_os = "macos")]
    assert_eq!(platform, "macos", "Platform should be macos on macOS");

    #[cfg(target_os = "linux")]
    assert_eq!(platform, "linux", "Platform should be linux on Linux");

    #[cfg(target_os = "windows")]
    assert_eq!(platform, "windows", "Platform should be windows on Windows");

    println!("Detected platform: {platform}");
}

/// Test mount status detection for an existing mounted volume.
#[test]
fn test_get_mount_status_for_root_volume() {
    let handler = PlatformMountHandler::new();

    // On macOS, /Volumes/Macintosh HD should exist but may not be detected as "mounted"
    // because our implementation treats it specially
    #[cfg(target_os = "macos")]
    {
        // Test with the root volume path
        let result = handler.get_mount_status(&PathBuf::from("/"));
        // This should not panic - the result depends on how the handler interprets it
        match result {
            Ok(status) => {
                println!(
                    "Root volume status: is_mounted={}, is_accessible={}",
                    status.is_mounted, status.is_accessible
                );
            }
            Err(e) => {
                // NotFound is acceptable for non-/Volumes paths
                println!("Root volume check returned error (acceptable): {e}");
            }
        }
    }
}

/// Test mount status detection for a temporary directory.
#[test]
fn test_get_mount_status_for_temp_dir() {
    let handler = PlatformMountHandler::new();

    // Create a temp directory
    let temp_dir = std::env::temp_dir();
    println!("Testing mount status for temp dir: {temp_dir:?}");

    let result = handler.get_mount_status(&temp_dir);

    // On macOS, non-/Volumes paths will return NotFound because
    // our implementation expects device paths or /Volumes paths
    match result {
        Ok(status) => {
            println!(
                "Temp dir status: is_mounted={}, is_accessible={}, is_read_only={}",
                status.is_mounted, status.is_accessible, status.is_read_only
            );
        }
        Err(e) => {
            // This is expected behavior for paths outside /Volumes on macOS
            println!("Temp dir check returned error (acceptable): {e}");
        }
    }
}

/// Test mount status detection for a known mounted volume.
/// This test is macOS-specific and uses /Volumes paths.
#[cfg(target_os = "macos")]
#[test]
fn test_get_mount_status_for_volumes_path() {
    let handler = PlatformMountHandler::new();

    // List all mounted volumes
    if let Ok(entries) = std::fs::read_dir("/Volumes") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let path_str = path.to_string_lossy();

                // Skip Macintosh HD symlink
                if path_str.contains("Macintosh HD") {
                    continue;
                }

                println!("\nTesting volume: {path:?}");
                let result = handler.get_mount_status(&path);

                match result {
                    Ok(status) => {
                        println!("  is_mounted: {}", status.is_mounted);
                        println!("  mount_point: {:?}", status.mount_point);
                        println!("  is_accessible: {}", status.is_accessible);
                        println!("  is_read_only: {}", status.is_read_only);

                        // For a real mounted volume, these should be true
                        assert!(status.is_mounted, "Volume should be detected as mounted");
                        assert_eq!(
                            status.mount_point,
                            Some(path.clone()),
                            "Mount point should match"
                        );
                        assert!(status.is_accessible, "Volume should be accessible");
                    }
                    Err(e) => {
                        println!("  Error: {e}");
                        // This might happen for special volumes
                    }
                }
            }
        }
    } else {
        println!("Could not read /Volumes directory");
    }
}

/// Test `is_mount_point_accessible` for various paths.
#[test]
fn test_is_mount_point_accessible() {
    let handler = PlatformMountHandler::new();

    // Temp dir should be accessible
    let temp_dir = std::env::temp_dir();
    assert!(
        handler.is_mount_point_accessible(&temp_dir),
        "Temp directory should be accessible"
    );

    // Nonexistent path should not be accessible
    assert!(
        !handler.is_mount_point_accessible(&PathBuf::from("/nonexistent/path/12345")),
        "Nonexistent path should not be accessible"
    );

    // Current working directory should be accessible
    if let Ok(cwd) = std::env::current_dir() {
        assert!(
            handler.is_mount_point_accessible(&cwd),
            "Current working directory should be accessible"
        );
    }
}

/// Test that `unmount_device` returns appropriate error for non-mount-point.
#[test]
fn test_unmount_nonexistent_path_fails() {
    let handler = PlatformMountHandler::new();

    let result = handler.unmount_device(&PathBuf::from("/nonexistent/path/12345"), false);

    assert!(result.is_err(), "Unmounting nonexistent path should fail");

    let err = result.unwrap_err();
    let err_str = err.to_string();
    println!("Unmount error (expected): {err_str}");

    // Should be a "not mounted" error
    assert!(
        err_str.contains("not mounted")
            || err_str.contains("NotMounted")
            || err_str.contains("does not exist"),
        "Error should indicate the path is not mounted"
    );
}

/// Test that `eject_device` returns appropriate error for non-mount-point.
#[test]
fn test_eject_nonexistent_path_fails() {
    let handler = PlatformMountHandler::new();

    let result = handler.eject_device(&PathBuf::from("/nonexistent/path/12345"));

    assert!(result.is_err(), "Ejecting nonexistent path should fail");

    let err = result.unwrap_err();
    println!("Eject error (expected): {err}");
}

/// Test Default trait implementation.
#[test]
fn test_platform_mount_handler_default() {
    let handler1 = PlatformMountHandler::new();
    let handler2 = PlatformMountHandler::default();

    assert_eq!(
        handler1.platform(),
        handler2.platform(),
        "new() and default() should produce same platform"
    );
}

/// Test `mount_device_auto` returns appropriate error for invalid device path.
#[test]
fn test_mount_invalid_device_fails() {
    let handler = PlatformMountHandler::new();

    let result = handler.mount_device_auto(&PathBuf::from("/nonexistent/device/12345"));

    // This should fail because the device doesn't exist
    assert!(result.is_err(), "Mounting nonexistent device should fail");

    let err = result.unwrap_err();
    println!("Mount error (expected): {err}");
}

/// Test `mount_device_at` returns appropriate error for invalid paths.
#[test]
fn test_mount_at_invalid_paths_fails() {
    let handler = PlatformMountHandler::new();

    let result = handler.mount_device_at(
        &PathBuf::from("/nonexistent/device"),
        &PathBuf::from("/nonexistent/mount_point"),
    );

    assert!(result.is_err(), "Mounting with invalid paths should fail");

    let err = result.unwrap_err();
    println!("Mount at error (expected): {err}");
}
