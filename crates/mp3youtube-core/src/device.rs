//! Device detection and management for USB-mounted MP3 players.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sysinfo::Disks;

use crate::error::{Error, Result};

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
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }

    /// Returns the usage percentage (0.0 - 100.0).
    #[must_use]
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
/// Returns `Error::DeviceNotFound` if no device is found at the mount point.
pub fn get_device_by_mount_point(
    detector: &dyn DeviceDetector,
    mount_point: &PathBuf,
) -> Result<DeviceInfo> {
    let devices = detector.list_devices()?;
    devices
        .into_iter()
        .find(|d| d.mount_point == *mount_point)
        .ok_or_else(|| Error::DeviceNotFound(mount_point.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(matches!(result, Err(Error::DeviceNotFound(_))));
    }

    #[test]
    fn test_device_manager_creation() {
        let manager = DeviceManager::new();
        // Just verify it can be created without panicking
        let result = manager.list_devices();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_all_disks_debug() {
        use sysinfo::Disks;

        let disks = Disks::new_with_refreshed_list();
        println!("\n=== All Disks ===");
        for disk in disks.iter() {
            let mount = disk.mount_point().to_string_lossy();
            let fs = disk.file_system().to_string_lossy();
            let name = disk.name().to_string_lossy();
            let removable = disk.is_removable();

            println!("  Name: {}", name);
            println!("  Mount: {}", mount);
            println!("  FS: {}", fs);
            println!("  Removable: {}", removable);

            // Check our filter
            let is_mp3 = DeviceManager::is_likely_mp3_device(disk);
            println!("  Would detect as MP3 device: {}", is_mp3);
            println!("  ---");
        }

        let manager = DeviceManager::new();
        let devices = manager.list_devices().expect("list_devices failed");
        println!("\n=== Detected MP3 Devices ===");
        for device in &devices {
            println!("  {:?}", device);
        }
        println!("Total: {} devices", devices.len());

        // Print JSON serialization
        let json = serde_json::to_string_pretty(&devices).expect("serialize failed");
        println!("\n=== JSON Format ===\n{}", json);
    }
}
