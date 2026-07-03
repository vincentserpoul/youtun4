use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::error::{DeviceError, Error, Result};

// =============================================================================
// Device Mount Handler
// =============================================================================

/// Result of a mount operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountResult {
    /// The mount point where the device was mounted.
    pub mount_point: PathBuf,
    /// The device that was mounted.
    pub device_name: String,
    /// Whether the mount was successful.
    pub success: bool,
    /// Optional message with details.
    pub message: Option<String>,
}

/// Result of an unmount operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmountResult {
    /// The mount point that was unmounted.
    pub mount_point: PathBuf,
    /// Whether the unmount was successful.
    pub success: bool,
    /// Optional message with details.
    pub message: Option<String>,
}

/// Information about a device's mount status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountStatus {
    /// Whether the device is currently mounted.
    pub is_mounted: bool,
    /// The current mount point, if mounted.
    pub mount_point: Option<PathBuf>,
    /// Whether the device is accessible (readable/writable).
    pub is_accessible: bool,
    /// Whether the device is read-only.
    pub is_read_only: bool,
}

/// Trait for device mount/unmount operations.
///
/// This trait defines the interface for platform-specific mount operations.
/// Different platforms (macOS, Linux, Windows) have different mechanisms
/// for mounting and unmounting removable devices.
#[cfg_attr(test, mockall::automock)]
pub trait DeviceMountHandler: Send + Sync {
    /// Check the mount status of a device.
    fn get_mount_status(&self, device_path: &Path) -> Result<MountStatus>;

    /// Mount a device with automatic mount point selection.
    ///
    /// On most platforms, the system will automatically choose an appropriate
    /// mount point (e.g., `/Volumes/DeviceName` on macOS, `/media/user/DeviceName` on Linux).
    fn mount_device_auto(&self, device_path: &Path) -> Result<MountResult>;

    /// Mount a device to a specific mount point.
    ///
    /// If the platform doesn't support specifying mount points, this may fail
    /// or behave the same as `mount_device_auto`.
    fn mount_device_at(&self, device_path: &Path, mount_point: &Path) -> Result<MountResult>;

    /// Unmount a device from its current mount point.
    fn unmount_device(&self, mount_point: &Path, force: bool) -> Result<UnmountResult>;

    /// Safely eject a device.
    fn eject_device(&self, mount_point: &Path) -> Result<UnmountResult>;

    /// Check if a mount point is accessible for read/write operations.
    fn is_mount_point_accessible(&self, mount_point: &Path) -> bool;

    /// Get the platform identifier.
    fn platform(&self) -> &'static str;
}

/// Platform-specific device mount handler.
///
/// This implementation provides mount/unmount functionality for
/// macOS, Linux, and Windows platforms.
#[derive(Debug)]
pub struct PlatformMountHandler {
    platform: &'static str,
}

impl PlatformMountHandler {
    /// Create a new platform mount handler.
    #[must_use]
    pub const fn new() -> Self {
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "unknown"
        };

        Self { platform }
    }

    #[allow(
        clippy::unused_self,
        reason = "method for consistency with other PlatformDeviceOps methods"
    )]
    fn execute_command(&self, program: &str, args: &[&str]) -> Result<std::process::Output> {
        debug!("Executing command: {} {:?}", program, args);
        Command::new(program)
            .args(args)
            .output()
            .map_err(|e| Error::Internal(format!("Failed to execute {program}: {e}")))
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[allow(
        clippy::unused_self,
        reason = "method for consistency with other PlatformDeviceOps methods"
    )]
    fn path_is_mount_point(&self, path: &Path) -> bool {
        path.exists() && path.is_dir()
    }

    #[allow(
        clippy::unused_self,
        reason = "method for consistency with other PlatformDeviceOps methods"
    )]
    fn check_write_access(&self, mount_point: &Path) -> bool {
        let test_file = mount_point.join(".youtun4_access_check");
        match std::fs::write(&test_file, "test") {
            Ok(()) => {
                let _ = std::fs::remove_file(&test_file);
                true
            }
            Err(_) => false,
        }
    }

    #[cfg(target_os = "macos")]
    fn platform_get_mount_status(&self, device_path: &Path) -> Result<MountStatus> {
        let path_str = device_path.to_string_lossy();

        if path_str.starts_with("/Volumes/") {
            let is_mounted = device_path.exists() && device_path.is_dir();
            let is_accessible = is_mounted && device_path.read_dir().is_ok();
            let is_read_only = is_mounted && !self.check_write_access(device_path);

            return Ok(MountStatus {
                is_mounted,
                mount_point: is_mounted.then(|| device_path.to_path_buf()),
                is_accessible,
                is_read_only,
            });
        }

        let output = self.execute_command("diskutil", &["info", &path_str])?;
        if !output.status.success() {
            return Err(Error::Device(DeviceError::NotFound {
                name: path_str.to_string(),
            }));
        }

        let info = String::from_utf8_lossy(&output.stdout);
        let is_mounted = info.contains("Mounted:") && info.contains("Yes");
        let mount_point = info
            .lines()
            .find(|line| line.contains("Mount Point:"))
            .and_then(|line| line.split(':').nth(1))
            .map(|s| PathBuf::from(s.trim()));

        let is_accessible = mount_point.as_ref().is_some_and(|mp| mp.read_dir().is_ok());
        let is_read_only = mount_point
            .as_ref()
            .is_some_and(|mp| !self.check_write_access(mp));

        Ok(MountStatus {
            is_mounted,
            mount_point,
            is_accessible,
            is_read_only,
        })
    }

    #[cfg(target_os = "macos")]
    fn platform_mount_device(
        &self,
        device_path: &Path,
        _mount_point: Option<&Path>,
    ) -> Result<MountResult> {
        let path_str = device_path.to_string_lossy();
        info!("Mounting device on macOS: {}", path_str);

        let output = self.execute_command("diskutil", &["mount", &path_str])?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mount_point = self
                .platform_get_mount_status(device_path)?
                .mount_point
                .unwrap_or_else(|| PathBuf::from("/Volumes/Untitled"));

            info!("Device mounted at {:?}", mount_point);
            Ok(MountResult {
                mount_point,
                device_name: path_str.to_string(),
                success: true,
                message: Some(stdout.trim().to_string()),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to mount device: {}", stderr);
            Err(Error::mount_failed(
                path_str.to_string(),
                "/Volumes",
                stderr.trim().to_string(),
            ))
        }
    }

    #[cfg(target_os = "macos")]
    fn platform_unmount_device(&self, mount_point: &Path, force: bool) -> Result<UnmountResult> {
        let path_str = mount_point.to_string_lossy();
        info!("Unmounting device on macOS: {}", path_str);

        if !self.path_is_mount_point(mount_point) {
            return Err(Error::Device(DeviceError::NotMounted {
                mount_point: mount_point.to_path_buf(),
            }));
        }

        let args: Vec<&str> = if force {
            vec!["unmount", "force", &path_str]
        } else {
            vec!["unmount", &path_str]
        };

        let output = self.execute_command("diskutil", &args)?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            info!("Device unmounted successfully");
            Ok(UnmountResult {
                mount_point: mount_point.to_path_buf(),
                success: true,
                message: Some(stdout.trim().to_string()),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("busy") || stderr.contains("in use") {
                error!("Device is busy: {}", stderr);
                return Err(Error::device_busy(mount_point, stderr.trim().to_string()));
            }
            error!("Failed to unmount device: {}", stderr);
            Err(Error::unmount_failed(
                mount_point,
                stderr.trim().to_string(),
            ))
        }
    }

    #[cfg(target_os = "macos")]
    fn platform_eject_device(&self, mount_point: &Path) -> Result<UnmountResult> {
        let path_str = mount_point.to_string_lossy();
        info!("Ejecting device on macOS: {}", path_str);

        let output = self.execute_command("diskutil", &["eject", &path_str])?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            info!("Device ejected successfully");
            Ok(UnmountResult {
                mount_point: mount_point.to_path_buf(),
                success: true,
                message: Some(stdout.trim().to_string()),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("busy") || stderr.contains("in use") {
                error!("Device is busy: {}", stderr);
                return Err(Error::device_busy(mount_point, stderr.trim().to_string()));
            }
            error!("Failed to eject device: {}", stderr);
            Err(Error::unmount_failed(
                mount_point,
                stderr.trim().to_string(),
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn platform_get_mount_status(&self, device_path: &Path) -> Result<MountStatus> {
        let path_str = device_path.to_string_lossy();

        if path_str.starts_with("/media/")
            || path_str.starts_with("/mnt/")
            || path_str.starts_with("/run/media/")
        {
            let is_mounted = device_path.exists() && device_path.is_dir();
            let is_accessible = is_mounted && device_path.read_dir().is_ok();
            let is_read_only = is_mounted && !self.check_write_access(device_path);

            return Ok(MountStatus {
                is_mounted,
                mount_point: is_mounted.then(|| device_path.to_path_buf()),
                is_accessible,
                is_read_only,
            });
        }

        let mounts = std::fs::read_to_string("/proc/mounts")
            .map_err(|e| Error::Internal(format!("Failed to read /proc/mounts: {e}")))?;

        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let (Some(device_col), Some(mount_col)) = (parts.first(), parts.get(1))
                && *device_col == path_str
            {
                let mount_point = PathBuf::from(*mount_col);
                let is_accessible = mount_point.read_dir().is_ok();
                let is_read_only = parts.get(3).is_some_and(|opts| opts.contains("ro"));
                return Ok(MountStatus {
                    is_mounted: true,
                    mount_point: Some(mount_point),
                    is_accessible,
                    is_read_only,
                });
            }
        }

        Ok(MountStatus {
            is_mounted: false,
            mount_point: None,
            is_accessible: false,
            is_read_only: true,
        })
    }

    #[cfg(target_os = "linux")]
    fn platform_mount_device(
        &self,
        device_path: &Path,
        mount_point: Option<&Path>,
    ) -> Result<MountResult> {
        let device_str = device_path.to_string_lossy();
        info!("Mounting device on Linux: {}", device_str);

        // Try udisksctl first
        if let Ok(output) = self.execute_command("udisksctl", &["mount", "-b", &device_str])
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let resolved_mount_point = stdout
                .lines()
                .find(|line| line.contains("Mounted"))
                .and_then(|line| line.split(" at ").nth(1))
                .map_or_else(
                    || {
                        mount_point.map_or_else(
                            || PathBuf::from("/media/unknown"),
                            std::path::Path::to_path_buf,
                        )
                    },
                    |s| PathBuf::from(s.trim().trim_end_matches('.')),
                );

            info!("Device mounted at {:?}", resolved_mount_point);
            return Ok(MountResult {
                mount_point: resolved_mount_point,
                device_name: device_str.to_string(),
                success: true,
                message: Some(stdout.trim().to_string()),
            });
        }

        // Try gio mount
        if let Ok(output) = self.execute_command("gio", &["mount", "-d", &device_str])
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let status = self.platform_get_mount_status(device_path)?;
            let resolved_mount_point = status.mount_point.unwrap_or_else(|| {
                mount_point.map_or_else(
                    || PathBuf::from("/media/unknown"),
                    std::path::Path::to_path_buf,
                )
            });

            info!("Device mounted via gio at {:?}", resolved_mount_point);
            return Ok(MountResult {
                mount_point: resolved_mount_point,
                device_name: device_str.to_string(),
                success: true,
                message: Some(stdout.trim().to_string()),
            });
        }

        Err(Error::mount_failed(
            device_str.to_string(),
            "/media",
            "No mount method succeeded",
        ))
    }

    #[cfg(target_os = "linux")]
    fn platform_unmount_device(&self, mount_point: &Path, force: bool) -> Result<UnmountResult> {
        let path_str = mount_point.to_string_lossy();
        info!("Unmounting device on Linux: {}", path_str);

        if !self.path_is_mount_point(mount_point) {
            return Err(Error::Device(DeviceError::NotMounted {
                mount_point: mount_point.to_path_buf(),
            }));
        }

        // Try udisksctl
        if let Ok(output) = self.execute_command("udisksctl", &["unmount", "-p", &path_str])
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            info!("Device unmounted via udisksctl");
            return Ok(UnmountResult {
                mount_point: mount_point.to_path_buf(),
                success: true,
                message: Some(stdout.trim().to_string()),
            });
        }

        // Try gio
        if let Ok(output) = self.execute_command("gio", &["mount", "-u", &path_str])
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            info!("Device unmounted via gio");
            return Ok(UnmountResult {
                mount_point: mount_point.to_path_buf(),
                success: true,
                message: Some(stdout.trim().to_string()),
            });
        }

        // Fallback to umount
        let args: Vec<&str> = if force {
            vec!["-f", &path_str]
        } else {
            vec![&*path_str]
        };
        let output = self.execute_command("umount", &args)?;

        if output.status.success() {
            info!("Device unmounted via umount");
            Ok(UnmountResult {
                mount_point: mount_point.to_path_buf(),
                success: true,
                message: Some("Unmounted successfully".to_string()),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("busy") || stderr.contains("target is busy") {
                error!("Device is busy: {}", stderr);
                return Err(Error::device_busy(mount_point, stderr.trim().to_string()));
            }
            error!("Failed to unmount device: {}", stderr);
            Err(Error::unmount_failed(
                mount_point,
                stderr.trim().to_string(),
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn platform_eject_device(&self, mount_point: &Path) -> Result<UnmountResult> {
        self.platform_unmount_device(mount_point, false)?;
        let path_str = mount_point.to_string_lossy();
        let _ = self.execute_command("udisksctl", &["power-off", "-p", &path_str]);
        info!("Device ejected on Linux");
        Ok(UnmountResult {
            mount_point: mount_point.to_path_buf(),
            success: true,
            message: Some("Device ejected successfully".to_string()),
        })
    }

    #[cfg(target_os = "windows")]
    fn platform_get_mount_status(&self, device_path: &Path) -> Result<MountStatus> {
        let is_mounted = device_path.exists();
        let is_accessible = is_mounted && device_path.read_dir().is_ok();
        let is_read_only = is_mounted && !self.check_write_access(device_path);

        Ok(MountStatus {
            is_mounted,
            mount_point: if is_mounted {
                Some(device_path.to_path_buf())
            } else {
                None
            },
            is_accessible,
            is_read_only,
        })
    }

    #[cfg(target_os = "windows")]
    fn platform_mount_device(
        &self,
        device_path: &Path,
        _mount_point: Option<&Path>,
    ) -> Result<MountResult> {
        let path_str = device_path.to_string_lossy();
        if device_path.exists() {
            return Ok(MountResult {
                mount_point: device_path.to_path_buf(),
                device_name: path_str.to_string(),
                success: true,
                message: Some("Device already mounted".to_string()),
            });
        }
        Err(Error::platform_not_supported(
            "Manual mounting on Windows requires administrator privileges",
        ))
    }

    #[cfg(target_os = "windows")]
    fn platform_unmount_device(&self, mount_point: &Path, _force: bool) -> Result<UnmountResult> {
        let path_str = mount_point.to_string_lossy();
        info!("Unmounting device on Windows: {}", path_str);

        let output = self.execute_command("mountvol", &[&path_str, "/P"])?;

        if output.status.success() {
            info!("Device unmounted on Windows");
            Ok(UnmountResult {
                mount_point: mount_point.to_path_buf(),
                success: true,
                message: Some("Unmounted successfully".to_string()),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to unmount on Windows: {}", stderr);
            Err(Error::unmount_failed(
                mount_point,
                stderr.trim().to_string(),
            ))
        }
    }

    #[cfg(target_os = "windows")]
    fn platform_eject_device(&self, mount_point: &Path) -> Result<UnmountResult> {
        let path_str = mount_point.to_string_lossy();
        let script = format!(
            "(New-Object -ComObject Shell.Application).NameSpace(17).ParseName('{}').InvokeVerb('Eject')",
            path_str.trim_end_matches('\\')
        );

        let output = self.execute_command("powershell", &["-Command", &script])?;

        if output.status.success() {
            info!("Device ejected on Windows");
            Ok(UnmountResult {
                mount_point: mount_point.to_path_buf(),
                success: true,
                message: Some("Device ejected successfully".to_string()),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to eject on Windows: {}", stderr);
            Err(Error::unmount_failed(
                mount_point,
                stderr.trim().to_string(),
            ))
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn platform_get_mount_status(&self, device_path: &Path) -> Result<MountStatus> {
        Ok(MountStatus {
            is_mounted: device_path.exists(),
            mount_point: if device_path.exists() {
                Some(device_path.to_path_buf())
            } else {
                None
            },
            is_accessible: device_path.read_dir().is_ok(),
            is_read_only: !self.check_write_access(device_path),
        })
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn platform_mount_device(
        &self,
        _device_path: &Path,
        _mount_point: Option<&Path>,
    ) -> Result<MountResult> {
        Err(Error::platform_not_supported(self.platform))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn platform_unmount_device(&self, _mount_point: &Path, _force: bool) -> Result<UnmountResult> {
        Err(Error::platform_not_supported(self.platform))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn platform_eject_device(&self, _mount_point: &Path) -> Result<UnmountResult> {
        Err(Error::platform_not_supported(self.platform))
    }
}

impl Default for PlatformMountHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceMountHandler for PlatformMountHandler {
    fn get_mount_status(&self, device_path: &Path) -> Result<MountStatus> {
        self.platform_get_mount_status(device_path)
    }

    fn mount_device_auto(&self, device_path: &Path) -> Result<MountResult> {
        self.platform_mount_device(device_path, None)
    }

    fn mount_device_at(&self, device_path: &Path, mount_point: &Path) -> Result<MountResult> {
        self.platform_mount_device(device_path, Some(mount_point))
    }

    fn unmount_device(&self, mount_point: &Path, force: bool) -> Result<UnmountResult> {
        self.platform_unmount_device(mount_point, force)
    }

    fn eject_device(&self, mount_point: &Path) -> Result<UnmountResult> {
        self.platform_eject_device(mount_point)
    }

    fn is_mount_point_accessible(&self, mount_point: &Path) -> bool {
        mount_point.exists() && mount_point.is_dir() && mount_point.read_dir().is_ok()
    }

    fn platform(&self) -> &'static str {
        self.platform
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =============================================================================
    // MockDeviceMountHandler Tests
    // =============================================================================

    #[test]
    fn test_mock_mount_handler_get_status() {
        let mut mock = MockDeviceMountHandler::new();
        let mount_point = PathBuf::from("/Volumes/USB");

        mock.expect_get_mount_status().returning(|_| {
            Ok(MountStatus {
                is_mounted: true,
                mount_point: Some(PathBuf::from("/Volumes/USB")),
                is_accessible: true,
                is_read_only: false,
            })
        });

        let result = mock.get_mount_status(&mount_point);
        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.is_mounted);
        assert!(status.is_accessible);
        assert!(!status.is_read_only);
    }

    #[test]
    fn test_mock_mount_handler_mount_auto() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_mount_device_auto().returning(|_| {
            Ok(MountResult {
                mount_point: PathBuf::from("/Volumes/USB"),
                device_name: "disk2s1".to_string(),
                success: true,
                message: Some("Mounted successfully".to_string()),
            })
        });

        let result = mock.mount_device_auto(&PathBuf::from("/dev/disk2s1"));
        assert!(result.is_ok());
        let mount_result = result.unwrap();
        assert!(mount_result.success);
        assert_eq!(mount_result.mount_point, PathBuf::from("/Volumes/USB"));
    }

    #[test]
    fn test_mock_mount_handler_mount_at() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_mount_device_at().returning(|_, _| {
            Ok(MountResult {
                mount_point: PathBuf::from("/mnt/custom"),
                device_name: "disk2s1".to_string(),
                success: true,
                message: Some("Mounted at custom location".to_string()),
            })
        });

        let result = mock.mount_device_at(
            &PathBuf::from("/dev/disk2s1"),
            &PathBuf::from("/mnt/custom"),
        );
        assert!(result.is_ok());
        let mount_result = result.unwrap();
        assert_eq!(mount_result.mount_point, PathBuf::from("/mnt/custom"));
    }

    #[test]
    fn test_mock_mount_handler_unmount() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_unmount_device().returning(|mp, _| {
            Ok(UnmountResult {
                mount_point: mp.to_path_buf(),
                success: true,
                message: Some("Unmounted successfully".to_string()),
            })
        });

        let result = mock.unmount_device(&PathBuf::from("/Volumes/USB"), false);
        assert!(result.is_ok());
        let unmount_result = result.unwrap();
        assert!(unmount_result.success);
    }

    #[test]
    fn test_mock_mount_handler_force_unmount() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_unmount_device()
            .withf(|_, force| *force)
            .returning(|mp, _| {
                Ok(UnmountResult {
                    mount_point: mp.to_path_buf(),
                    success: true,
                    message: Some("Force unmounted".to_string()),
                })
            });

        let result = mock.unmount_device(&PathBuf::from("/Volumes/USB"), true);
        result.unwrap();
    }

    #[test]
    fn test_mock_mount_handler_eject() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_eject_device().returning(|mp| {
            Ok(UnmountResult {
                mount_point: mp.to_path_buf(),
                success: true,
                message: Some("Ejected successfully".to_string()),
            })
        });

        let result = mock.eject_device(&PathBuf::from("/Volumes/USB"));
        result.unwrap();
    }

    #[test]
    fn test_mock_mount_handler_is_accessible() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_is_mount_point_accessible().returning(|_| true);

        assert!(mock.is_mount_point_accessible(&PathBuf::from("/Volumes/USB")));
    }

    #[test]
    fn test_mock_mount_handler_platform() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_platform().returning(|| "test");

        assert_eq!(mock.platform(), "test");
    }

    #[test]
    fn test_mock_mount_handler_mount_failure() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_mount_device_auto()
            .returning(|_| Err(Error::mount_failed("disk2s1", "/Volumes", "Device busy")));

        let result = mock.mount_device_auto(&PathBuf::from("/dev/disk2s1"));
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Device(DeviceError::MountFailed { .. }))
        ));
    }

    #[test]
    fn test_mock_mount_handler_unmount_failure_busy() {
        let mut mock = MockDeviceMountHandler::new();

        mock.expect_unmount_device()
            .returning(|mp, _| Err(Error::device_busy(mp, "Resource busy")));

        let result = mock.unmount_device(&PathBuf::from("/Volumes/USB"), false);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(Error::Device(DeviceError::DeviceBusy { .. }))
        ));
    }

    // =============================================================================
    // PlatformMountHandler Tests
    // =============================================================================

    #[test]
    fn test_platform_mount_handler_creation() {
        let handler = PlatformMountHandler::new();
        // Platform should be one of the known values
        let platform = handler.platform();
        assert!(
            platform == "macos"
                || platform == "linux"
                || platform == "windows"
                || platform == "unknown",
            "Unexpected platform: {platform}"
        );
    }

    #[test]
    fn test_platform_mount_handler_default() {
        let handler = PlatformMountHandler::default();
        // Should not panic and should have a platform set
        assert!(!handler.platform().is_empty());
    }

    #[test]
    fn test_platform_mount_handler_accessibility_check() {
        let handler = PlatformMountHandler::new();
        let temp_dir = TempDir::new().expect("create temp dir");

        // Temp directory should be accessible
        assert!(handler.is_mount_point_accessible(temp_dir.path()));

        // Nonexistent path should not be accessible
        assert!(!handler.is_mount_point_accessible(&PathBuf::from("/nonexistent/path")));
    }

    #[test]
    fn test_platform_mount_handler_get_status_existing_dir() {
        let handler = PlatformMountHandler::new();
        let temp_dir = TempDir::new().expect("create temp dir");

        // Note: On macOS, get_mount_status expects paths under /Volumes/ for proper handling,
        // so a temp directory might fail or return NotFound. We just verify it doesn't panic.
        let result = handler.get_mount_status(temp_dir.path());
        // The result could be Ok or Err depending on platform behavior with temp dirs
        // What we're testing is that the function doesn't panic and returns a valid result type
        if let Ok(status) = result {
            // If it succeeds, just verify the struct is valid
            // is_mounted could be true or false depending on platform interpretation
            let _ = status.is_mounted; // Just verify we can access the field
        } else {
            // On macOS, temp dirs aren't under /Volumes, so NotFound is acceptable
            // This is expected behavior, not a failure
        }
    }

    // =============================================================================
    // MountResult and UnmountResult Tests
    // =============================================================================

    #[test]
    fn test_mount_result_serialization() {
        let result = MountResult {
            mount_point: PathBuf::from("/Volumes/USB"),
            device_name: "disk2s1".to_string(),
            success: true,
            message: Some("Mounted successfully".to_string()),
        };

        let json = serde_json::to_string(&result).expect("serialize failed");
        let deserialized: MountResult = serde_json::from_str(&json).expect("deserialize failed");

        assert_eq!(result.mount_point, deserialized.mount_point);
        assert_eq!(result.device_name, deserialized.device_name);
        assert_eq!(result.success, deserialized.success);
        assert_eq!(result.message, deserialized.message);
    }

    #[test]
    fn test_unmount_result_serialization() {
        let result = UnmountResult {
            mount_point: PathBuf::from("/Volumes/USB"),
            success: true,
            message: None,
        };

        let json = serde_json::to_string(&result).expect("serialize failed");
        let deserialized: UnmountResult = serde_json::from_str(&json).expect("deserialize failed");

        assert_eq!(result.mount_point, deserialized.mount_point);
        assert_eq!(result.success, deserialized.success);
        assert_eq!(result.message, deserialized.message);
    }

    #[test]
    fn test_mount_status_serialization() {
        let status = MountStatus {
            is_mounted: true,
            mount_point: Some(PathBuf::from("/Volumes/USB")),
            is_accessible: true,
            is_read_only: false,
        };

        let json = serde_json::to_string(&status).expect("serialize failed");
        let deserialized: MountStatus = serde_json::from_str(&json).expect("deserialize failed");

        assert_eq!(status.is_mounted, deserialized.is_mounted);
        assert_eq!(status.mount_point, deserialized.mount_point);
        assert_eq!(status.is_accessible, deserialized.is_accessible);
        assert_eq!(status.is_read_only, deserialized.is_read_only);
    }
}
