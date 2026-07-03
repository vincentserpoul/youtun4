use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sysinfo::Disks;

use crate::error::{DeviceError, Error, Result};

/// Information about a detected device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Device name/identifier.
    pub name: String,
    /// Mount point path.
    pub mount_point: PathBuf,
    /// Total capacity in bytes.
    pub total_bytes: u64,
    /// Available space in bytes.
    pub available_bytes: u64,
    /// File system type (e.g., FAT32, exFAT).
    pub file_system: String,
    /// Whether the device is removable.
    pub is_removable: bool,
}

impl DeviceInfo {
    /// Returns the used space in bytes.
    #[must_use]
    pub const fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }

    /// Returns the usage percentage (0.0 - 100.0).
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::float_arithmetic,
        reason = "percentage calculation for display"
    )]
    pub fn usage_percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes() as f64 / self.total_bytes as f64) * 100.0
    }
}

/// Trait for device detection operations.
/// This trait allows for mocking in tests.
#[cfg_attr(test, mockall::automock)]
pub trait DeviceDetector: Send + Sync {
    /// List all detected removable devices.
    fn list_devices(&self) -> Result<Vec<DeviceInfo>>;

    /// Check if a device is still connected.
    fn is_device_connected(&self, mount_point: &Path) -> bool;

    /// Refresh device list.
    fn refresh(&mut self);
}

/// Default device manager using `sysinfo`.
#[derive(Debug)]
pub struct DeviceManager {
    disks: Disks,
}

impl DeviceManager {
    /// Create a new device manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            disks: Disks::new_with_refreshed_list(),
        }
    }

    /// Filter function to determine if a disk is likely an MP3 player.
    fn is_likely_mp3_device(disk: &sysinfo::Disk) -> bool {
        let mount_point = disk.mount_point().to_string_lossy();
        let fs = disk.file_system().to_string_lossy().to_lowercase();

        // On macOS, external devices are mounted under /Volumes
        // On Linux, they're typically under /media or /mnt
        let is_external_mount = mount_point.starts_with("/Volumes/")
            || mount_point.starts_with("/media/")
            || mount_point.starts_with("/mnt/")
            || mount_point.starts_with("/run/media/");

        // Check if removable OR if it's mounted in an external location
        let is_removable = disk.is_removable() || is_external_mount;

        if !is_removable {
            return false;
        }

        // Skip system volumes on macOS
        if mount_point == "/Volumes/Macintosh HD"
            || mount_point.contains("Recovery")
            || mount_point.contains("Preboot")
        {
            return false;
        }

        // Check file system - MP3 players typically use FAT32 or exFAT
        // Include common variations: fat, fat32, vfat, msdos, exfat
        let supported_fs = ["fat32", "fat", "vfat", "exfat", "msdos", "msdosfs"];

        supported_fs.iter().any(|&supported| fs.contains(supported))
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceDetector for DeviceManager {
    fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        let devices: Vec<DeviceInfo> = self
            .disks
            .iter()
            .filter(|disk| Self::is_likely_mp3_device(disk))
            .map(|disk| DeviceInfo {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_path_buf(),
                total_bytes: disk.total_space(),
                available_bytes: disk.available_space(),
                file_system: disk.file_system().to_string_lossy().to_string(),
                is_removable: disk.is_removable(),
            })
            .collect();

        Ok(devices)
    }

    fn is_device_connected(&self, mount_point: &Path) -> bool {
        self.disks
            .iter()
            .any(|disk| disk.mount_point() == mount_point)
    }

    fn refresh(&mut self) {
        self.disks.refresh(true);
    }
}

/// Get a specific device by mount point.
///
/// # Errors
///
/// Returns `Error::Device(DeviceError::NotFound)` if no device is found at the mount point.
pub fn get_device_by_mount_point(
    detector: &dyn DeviceDetector,
    mount_point: &PathBuf,
) -> Result<DeviceInfo> {
    let devices = detector.list_devices()?;
    devices
        .into_iter()
        .find(|d| d.mount_point == *mount_point)
        .ok_or_else(|| {
            Error::Device(DeviceError::NotFound {
                name: mount_point.display().to_string(),
            })
        })
}

/// Check if a device has sufficient space for a transfer.
///
/// # Errors
///
/// Returns `Error::Device(DeviceError::InsufficientSpace)` if there isn't enough space.
pub fn check_device_space(device: &DeviceInfo, required_bytes: u64) -> Result<()> {
    if device.available_bytes < required_bytes {
        return Err(Error::Device(DeviceError::InsufficientSpace {
            device: device.name.clone(),
            available_bytes: device.available_bytes,
            required_bytes,
        }));
    }
    Ok(())
}

/// Verify a device is still connected and accessible.
///
/// # Errors
///
/// Returns an error if the device is no longer accessible.
pub fn verify_device_accessible(detector: &dyn DeviceDetector, device: &DeviceInfo) -> Result<()> {
    if !detector.is_device_connected(&device.mount_point) {
        return Err(Error::Device(DeviceError::Disconnected {
            name: device.name.clone(),
        }));
    }

    // Check if mount point is still accessible
    if !device.mount_point.exists() {
        return Err(Error::Device(DeviceError::NotMounted {
            mount_point: device.mount_point.clone(),
        }));
    }

    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "acceptable in tests"
)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    // =============================================================================
    // DeviceInfo Tests
    // =============================================================================

    #[test]
    fn test_device_info_used_bytes() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1000,
            available_bytes: 300,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        assert_eq!(device.used_bytes(), 700);
    }

    #[test]
    fn test_device_info_used_bytes_overflow_protection() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 100,
            available_bytes: 200, // More available than total (edge case)
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        // saturating_sub should return 0 instead of underflowing
        assert_eq!(device.used_bytes(), 0);
    }

    #[test]
    fn test_device_info_usage_percentage() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1000,
            available_bytes: 250,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        assert!((device.usage_percentage() - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_device_info_usage_percentage_zero_total() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 0,
            available_bytes: 0,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        assert!((device.usage_percentage() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_device_info_usage_percentage_full() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1000,
            available_bytes: 0,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        assert!((device.usage_percentage() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_device_info_usage_percentage_empty() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1000,
            available_bytes: 1000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        assert!((device.usage_percentage() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_device_info_serialization() {
        let device = DeviceInfo {
            name: "USB Drive".to_string(),
            mount_point: PathBuf::from("/Volumes/USB"),
            total_bytes: 16_000_000_000,
            available_bytes: 8_000_000_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };

        let json = serde_json::to_string(&device).expect("serialize failed");
        let deserialized: DeviceInfo = serde_json::from_str(&json).expect("deserialize failed");

        assert_eq!(device.name, deserialized.name);
        assert_eq!(device.mount_point, deserialized.mount_point);
        assert_eq!(device.total_bytes, deserialized.total_bytes);
        assert_eq!(device.available_bytes, deserialized.available_bytes);
        assert_eq!(device.file_system, deserialized.file_system);
        assert_eq!(device.is_removable, deserialized.is_removable);
    }

    #[test]
    fn test_device_info_equality() {
        let device1 = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1000,
            available_bytes: 500,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        let device2 = device1.clone();
        assert_eq!(device1, device2);

        let device3 = DeviceInfo {
            name: "different".to_string(),
            ..device1.clone()
        };
        assert_ne!(device1, device3);
    }

    // =============================================================================
    // MockDeviceDetector Tests
    // =============================================================================

    #[test]
    fn test_get_device_by_mount_point_found() {
        let mut mock = MockDeviceDetector::new();
        let expected_device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/mnt/mp3"),
            total_bytes: 1000,
            available_bytes: 500,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        let returned_device = expected_device.clone();

        mock.expect_list_devices()
            .returning(move || Ok(vec![returned_device.clone()]));

        let result = get_device_by_mount_point(&mock, &PathBuf::from("/mnt/mp3"));
        assert!(result.is_ok());
        assert_eq!(result.ok(), Some(expected_device));
    }

    #[test]
    fn test_get_device_by_mount_point_not_found() {
        let mut mock = MockDeviceDetector::new();
        mock.expect_list_devices().returning(|| Ok(vec![]));

        let result = get_device_by_mount_point(&mock, &PathBuf::from("/nonexistent"));
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Device(DeviceError::NotFound { .. }))
        ));
    }

    #[test]
    fn test_get_device_by_mount_point_multiple_devices() {
        let mut mock = MockDeviceDetector::new();
        let device1 = DeviceInfo {
            name: "device1".to_string(),
            mount_point: PathBuf::from("/mnt/usb1"),
            total_bytes: 1000,
            available_bytes: 500,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        let device2 = DeviceInfo {
            name: "device2".to_string(),
            mount_point: PathBuf::from("/mnt/usb2"),
            total_bytes: 2000,
            available_bytes: 1000,
            file_system: "exFAT".to_string(),
            is_removable: true,
        };
        let expected = device2.clone();

        mock.expect_list_devices()
            .returning(move || Ok(vec![device1.clone(), device2.clone()]));

        let result = get_device_by_mount_point(&mock, &PathBuf::from("/mnt/usb2"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, expected.name);
    }

    #[test]
    fn test_mock_device_detector_is_connected() {
        let mut mock = MockDeviceDetector::new();
        let mount_point = PathBuf::from("/mnt/usb");
        let mp_clone = mount_point.clone();

        mock.expect_is_device_connected()
            .withf(move |mp| *mp == mp_clone)
            .returning(|_| true);

        assert!(mock.is_device_connected(&mount_point));
    }

    #[test]
    fn test_mock_device_detector_not_connected() {
        let mut mock = MockDeviceDetector::new();

        mock.expect_is_device_connected().returning(|_| false);

        assert!(!mock.is_device_connected(&PathBuf::from("/nonexistent")));
    }

    // =============================================================================
    // Device Space Check Tests
    // =============================================================================

    #[test]
    fn test_check_device_space_sufficient() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1_000_000,
            available_bytes: 500_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        let result = check_device_space(&device, 100_000);
        result.unwrap();
    }

    #[test]
    fn test_check_device_space_insufficient() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1_000_000,
            available_bytes: 50_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        let result = check_device_space(&device, 100_000);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Device(DeviceError::InsufficientSpace { .. }))
        ));
    }

    #[test]
    fn test_check_device_space_exact() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1_000_000,
            available_bytes: 100_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        // Exactly enough space should be OK
        let result = check_device_space(&device, 100_000);
        result.unwrap();
    }

    #[test]
    fn test_check_device_space_zero_required() {
        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1_000_000,
            available_bytes: 0,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        // Zero required should always succeed
        let result = check_device_space(&device, 0);
        result.unwrap();
    }

    #[test]
    fn test_check_device_space_error_details() {
        let device = DeviceInfo {
            name: "USB Drive".to_string(),
            mount_point: PathBuf::from("/test"),
            total_bytes: 1_000_000,
            available_bytes: 50_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        let result = check_device_space(&device, 100_000);

        match result {
            Err(Error::Device(DeviceError::InsufficientSpace {
                device,
                available_bytes,
                required_bytes,
            })) => {
                assert_eq!(device, "USB Drive");
                assert_eq!(available_bytes, 50_000);
                assert_eq!(required_bytes, 100_000);
            }
            _ => panic!("Expected InsufficientSpace error"),
        }
    }

    // =============================================================================
    // Verify Device Accessible Tests
    // =============================================================================

    #[test]
    fn test_verify_device_accessible_connected() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let mut mock = MockDeviceDetector::new();
        let mount_path = temp_dir.path().to_path_buf();
        let mp_clone = mount_path.clone();

        mock.expect_is_device_connected()
            .withf(move |mp| *mp == mp_clone)
            .returning(|_| true);

        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: mount_path,
            total_bytes: 1000,
            available_bytes: 500,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };

        let result = verify_device_accessible(&mock, &device);
        result.unwrap();
    }

    #[test]
    fn test_verify_device_accessible_disconnected() {
        let mut mock = MockDeviceDetector::new();

        mock.expect_is_device_connected().returning(|_| false);

        let device = DeviceInfo {
            name: "USB Drive".to_string(),
            mount_point: PathBuf::from("/mnt/usb"),
            total_bytes: 1000,
            available_bytes: 500,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };

        let result = verify_device_accessible(&mock, &device);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Device(DeviceError::Disconnected { .. }))
        ));
    }

    #[test]
    fn test_verify_device_accessible_mount_point_missing() {
        let mut mock = MockDeviceDetector::new();

        mock.expect_is_device_connected().returning(|_| true);

        let device = DeviceInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/nonexistent/path/that/does/not/exist"),
            total_bytes: 1000,
            available_bytes: 500,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };

        let result = verify_device_accessible(&mock, &device);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Device(DeviceError::NotMounted { .. }))
        ));
    }

    // =============================================================================
    // DeviceManager Tests
    // =============================================================================

    #[test]
    fn test_device_manager_creation() {
        let manager = DeviceManager::new();
        // Just verify it can be created without panicking
        let result = manager.list_devices();
        result.unwrap();
    }

    #[test]
    fn test_device_manager_default() {
        let manager = DeviceManager::default();
        let result = manager.list_devices();
        result.unwrap();
    }

    #[test]
    fn test_device_manager_refresh() {
        let mut manager = DeviceManager::new();
        // refresh() should not panic
        manager.refresh();
        let result = manager.list_devices();
        result.unwrap();
    }

    #[test]
    fn test_device_manager_is_device_connected_nonexistent() {
        let manager = DeviceManager::new();
        let result = manager.is_device_connected(&PathBuf::from("/nonexistent/path"));
        // Should return false for nonexistent paths
        assert!(!result);
    }

    // =============================================================================
    // Debug/Integration Tests
    // =============================================================================

    #[test]
    fn test_list_all_disks_debug() {
        use sysinfo::Disks;

        let disks = Disks::new_with_refreshed_list();
        println!("\n=== All Disks ===");
        for disk in &disks {
            let mount = disk.mount_point().to_string_lossy();
            let fs = disk.file_system().to_string_lossy();
            let name = disk.name().to_string_lossy();
            let removable = disk.is_removable();

            println!("  Name: {name}");
            println!("  Mount: {mount}");
            println!("  FS: {fs}");
            println!("  Removable: {removable}");

            // Check our filter
            let is_mp3 = DeviceManager::is_likely_mp3_device(disk);
            println!("  Would detect as MP3 device: {is_mp3}");
            println!("  ---");
        }

        let manager = DeviceManager::new();
        let devices = manager.list_devices().expect("list_devices failed");
        println!("\n=== Detected MP3 Devices ===");
        for device in &devices {
            println!("  {device:?}");
        }
        println!("Total: {} devices", devices.len());

        // Print JSON serialization
        let json = serde_json::to_string_pretty(&devices).expect("serialize failed");
        println!("\n=== JSON Format ===\n{json}");
    }
}
