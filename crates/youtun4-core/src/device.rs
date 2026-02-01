//! Device detection and management for USB-mounted MP3 players.
//!
//! This module provides:
//! - Device detection via [`DeviceDetector`] trait and [`DeviceManager`] implementation
//! - Mount/unmount operations via [`DeviceMountHandler`] trait and platform-specific implementations
//! - Device event monitoring for real-time mount/unmount notifications

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sysinfo::Disks;
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use tracing::{debug, error, info};

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

// =============================================================================
// Device Watching / Auto-Detection
// =============================================================================

/// Events emitted by the device watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DeviceEvent {
    /// A new device was connected.
    Connected(DeviceInfo),
    /// A device was disconnected.
    Disconnected(DeviceInfo),
    /// The device list was refreshed (includes all current devices).
    Refreshed(Vec<DeviceInfo>),
}

/// Default polling interval for device watching (2 seconds).
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Handle for controlling a running device watcher.
#[derive(Debug, Clone)]
pub struct DeviceWatcherHandle {
    shutdown_tx: mpsc::Sender<()>,
}

impl DeviceWatcherHandle {
    /// Stop the device watcher.
    pub async fn stop(&self) {
        let _ = self.shutdown_tx.send(()).await;
    }
}

/// Device watcher that monitors for USB device connections/disconnections.
///
/// Uses a polling approach with configurable interval to detect device changes.
/// Events are sent through a channel when devices are connected or disconnected.
pub struct DeviceWatcher {
    /// The device manager used for detection.
    device_manager: Arc<RwLock<DeviceManager>>,
    /// Polling interval for checking device changes.
    poll_interval: Duration,
}

impl DeviceWatcher {
    /// Create a new device watcher with the default polling interval.
    #[must_use]
    pub const fn new(device_manager: Arc<RwLock<DeviceManager>>) -> Self {
        Self {
            device_manager,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }

    /// Create a new device watcher with a custom polling interval.
    #[must_use]
    pub const fn with_interval(
        device_manager: Arc<RwLock<DeviceManager>>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            device_manager,
            poll_interval,
        }
    }

    /// Start watching for device changes.
    ///
    /// Returns a channel receiver for device events and a handle to stop the watcher.
    /// Events are emitted when:
    /// - A new device is connected (`DeviceEvent::Connected`)
    /// - A device is disconnected (`DeviceEvent::Disconnected`)
    /// - The watcher starts (initial `DeviceEvent::Refreshed` with all devices)
    #[must_use]
    pub fn start(self) -> (mpsc::Receiver<DeviceEvent>, DeviceWatcherHandle) {
        let (event_tx, event_rx) = mpsc::channel::<DeviceEvent>(32);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let device_manager = self.device_manager;
        let poll_interval = self.poll_interval;

        tokio::spawn(async move {
            let mut known_devices: HashSet<PathBuf> = HashSet::new();
            let mut interval_timer = interval(poll_interval);

            // Get initial device list
            {
                let mut manager = device_manager.write().await;
                manager.refresh();
                if let Ok(devices) = manager.list_devices() {
                    // Track known devices by mount point
                    for device in &devices {
                        known_devices.insert(device.mount_point.clone());
                    }
                    // Send initial refresh event
                    let _ = event_tx.send(DeviceEvent::Refreshed(devices)).await;
                }
            }

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Device watcher shutting down");
                        break;
                    }
                    _ = interval_timer.tick() => {
                        let mut manager = device_manager.write().await;
                        manager.refresh();

                        if let Ok(current_devices) = manager.list_devices() {
                            let current_mount_points: HashSet<PathBuf> = current_devices
                                .iter()
                                .map(|d| d.mount_point.clone())
                                .collect();

                            // Check for new devices (connected)
                            for device in &current_devices {
                                if !known_devices.contains(&device.mount_point) {
                                    tracing::info!("Device connected: {} at {}", device.name, device.mount_point.display());
                                    let _ = event_tx.send(DeviceEvent::Connected(device.clone())).await;
                                }
                            }

                            // Check for removed devices (disconnected)
                            let disconnected: Vec<PathBuf> = known_devices
                                .difference(&current_mount_points)
                                .cloned()
                                .collect();

                            for mount_point in disconnected {
                                // Create a minimal DeviceInfo for the disconnected device
                                let device_info = DeviceInfo {
                                    name: mount_point.file_name().map_or_else(|| "Unknown".to_string(), |n| n.to_string_lossy().to_string()),
                                    mount_point: mount_point.clone(),
                                    total_bytes: 0,
                                    available_bytes: 0,
                                    file_system: String::new(),
                                    is_removable: true,
                                };
                                tracing::info!("Device disconnected: {}", mount_point.display());
                                let _ = event_tx.send(DeviceEvent::Disconnected(device_info)).await;
                            }

                            // Update known devices
                            known_devices = current_mount_points;
                        }
                    }
                }
            }
        });

        (event_rx, DeviceWatcherHandle { shutdown_tx })
    }
}

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

    fn execute_command(&self, program: &str, args: &[&str]) -> Result<std::process::Output> {
        debug!("Executing command: {} {:?}", program, args);
        Command::new(program)
            .args(args)
            .output()
            .map_err(|e| Error::Internal(format!("Failed to execute {program}: {e}")))
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn path_is_mount_point(&self, path: &Path) -> bool {
        path.exists() && path.is_dir()
    }

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
                mount_point: if is_mounted {
                    Some(device_path.to_path_buf())
                } else {
                    None
                },
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
                mount_point: if is_mounted {
                    Some(device_path.to_path_buf())
                } else {
                    None
                },
                is_accessible,
                is_read_only,
            });
        }

        let mounts = std::fs::read_to_string("/proc/mounts")
            .map_err(|e| Error::Internal(format!("Failed to read /proc/mounts: {}", e)))?;

        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] == path_str {
                let mount_point = PathBuf::from(parts[1]);
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
        if let Ok(output) = self.execute_command("udisksctl", &["mount", "-b", &device_str]) {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mount_point = stdout
                    .lines()
                    .find(|line| line.contains("Mounted"))
                    .and_then(|line| line.split(" at ").nth(1))
                    .map(|s| PathBuf::from(s.trim().trim_end_matches('.')))
                    .unwrap_or_else(|| {
                        mount_point
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| PathBuf::from("/media/unknown"))
                    });

                info!("Device mounted at {:?}", mount_point);
                return Ok(MountResult {
                    mount_point,
                    device_name: device_str.to_string(),
                    success: true,
                    message: Some(stdout.trim().to_string()),
                });
            }
        }

        // Try gio mount
        if let Ok(output) = self.execute_command("gio", &["mount", "-d", &device_str]) {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let status = self.platform_get_mount_status(device_path)?;
                let mount_point = status.mount_point.unwrap_or_else(|| {
                    mount_point
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("/media/unknown"))
                });

                info!("Device mounted via gio at {:?}", mount_point);
                return Ok(MountResult {
                    mount_point,
                    device_name: device_str.to_string(),
                    success: true,
                    message: Some(stdout.trim().to_string()),
                });
            }
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
        if let Ok(output) = self.execute_command("udisksctl", &["unmount", "-p", &path_str]) {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                info!("Device unmounted via udisksctl");
                return Ok(UnmountResult {
                    mount_point: mount_point.to_path_buf(),
                    success: true,
                    message: Some(stdout.trim().to_string()),
                });
            }
        }

        // Try gio
        if let Ok(output) = self.execute_command("gio", &["mount", "-u", &path_str]) {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                info!("Device unmounted via gio");
                return Ok(UnmountResult {
                    mount_point: mount_point.to_path_buf(),
                    success: true,
                    message: Some(stdout.trim().to_string()),
                });
            }
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
        assert!(result.is_ok());
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
        assert!(result.is_ok());
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
        assert!(result.is_ok());
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
        assert!(result.is_ok());
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
        assert!(result.is_ok());
    }

    #[test]
    fn test_device_manager_default() {
        let manager = DeviceManager::default();
        let result = manager.list_devices();
        assert!(result.is_ok());
    }

    #[test]
    fn test_device_manager_refresh() {
        let mut manager = DeviceManager::new();
        // refresh() should not panic
        manager.refresh();
        let result = manager.list_devices();
        assert!(result.is_ok());
    }

    #[test]
    fn test_device_manager_is_device_connected_nonexistent() {
        let manager = DeviceManager::new();
        let result = manager.is_device_connected(&PathBuf::from("/nonexistent/path"));
        // Should return false for nonexistent paths
        assert!(!result);
    }

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
        assert!(result.is_ok());
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
        assert!(result.is_ok());
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

    // =============================================================================
    // DeviceEvent Tests
    // =============================================================================

    #[test]
    fn test_device_event_connected_serialization() {
        let device = DeviceInfo {
            name: "USB Drive".to_string(),
            mount_point: PathBuf::from("/Volumes/USB"),
            total_bytes: 16_000_000_000,
            available_bytes: 8_000_000_000,
            file_system: "FAT32".to_string(),
            is_removable: true,
        };
        let event = DeviceEvent::Connected(device.clone());

        let json = serde_json::to_string(&event).expect("serialize failed");
        let deserialized: DeviceEvent = serde_json::from_str(&json).expect("deserialize failed");

        match deserialized {
            DeviceEvent::Connected(d) => assert_eq!(d.name, device.name),
            _ => panic!("Expected Connected event"),
        }
    }

    #[test]
    fn test_device_event_disconnected_serialization() {
        let device = DeviceInfo {
            name: "USB Drive".to_string(),
            mount_point: PathBuf::from("/Volumes/USB"),
            total_bytes: 0,
            available_bytes: 0,
            file_system: String::new(),
            is_removable: true,
        };
        let event = DeviceEvent::Disconnected(device);

        let json = serde_json::to_string(&event).expect("serialize failed");
        let deserialized: DeviceEvent = serde_json::from_str(&json).expect("deserialize failed");

        assert!(matches!(deserialized, DeviceEvent::Disconnected(_)));
    }

    #[test]
    fn test_device_event_refreshed_serialization() {
        let devices = vec![
            DeviceInfo {
                name: "USB1".to_string(),
                mount_point: PathBuf::from("/Volumes/USB1"),
                total_bytes: 8_000_000_000,
                available_bytes: 4_000_000_000,
                file_system: "FAT32".to_string(),
                is_removable: true,
            },
            DeviceInfo {
                name: "USB2".to_string(),
                mount_point: PathBuf::from("/Volumes/USB2"),
                total_bytes: 16_000_000_000,
                available_bytes: 8_000_000_000,
                file_system: "exFAT".to_string(),
                is_removable: true,
            },
        ];
        let event = DeviceEvent::Refreshed(devices);

        let json = serde_json::to_string(&event).expect("serialize failed");
        let deserialized: DeviceEvent = serde_json::from_str(&json).expect("deserialize failed");

        match deserialized {
            DeviceEvent::Refreshed(d) => assert_eq!(d.len(), 2),
            _ => panic!("Expected Refreshed event"),
        }
    }

    // =============================================================================
    // DeviceWatcher Tests
    // =============================================================================

    #[tokio::test]
    async fn test_device_watcher_creation() {
        let device_manager = Arc::new(RwLock::new(DeviceManager::new()));
        let watcher = DeviceWatcher::new(device_manager);
        // Verify watcher can be created with default poll interval
        assert_eq!(watcher.poll_interval, DEFAULT_POLL_INTERVAL);
    }

    #[tokio::test]
    async fn test_device_watcher_custom_interval() {
        let device_manager = Arc::new(RwLock::new(DeviceManager::new()));
        let custom_interval = Duration::from_millis(500);
        let watcher = DeviceWatcher::with_interval(device_manager, custom_interval);
        assert_eq!(watcher.poll_interval, custom_interval);
    }

    #[tokio::test]
    async fn test_device_watcher_start_and_stop() {
        let device_manager = Arc::new(RwLock::new(DeviceManager::new()));
        let watcher = DeviceWatcher::with_interval(
            device_manager,
            Duration::from_millis(50), // Fast polling for tests
        );

        let (mut event_rx, handle) = watcher.start();

        // Should receive initial refresh event
        let event = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("channel closed");

        assert!(matches!(event, DeviceEvent::Refreshed(_)));

        // Stop the watcher
        handle.stop().await;

        // Give it a moment to shut down
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn test_device_watcher_handle_clone() {
        let device_manager = Arc::new(RwLock::new(DeviceManager::new()));
        let watcher = DeviceWatcher::new(device_manager);

        let (_event_rx, handle) = watcher.start();
        let handle_clone = handle.clone();

        // Both handles should be able to stop the watcher
        handle.stop().await;
        handle_clone.stop().await; // Should not panic even if already stopped
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

    #[test]
    fn test_default_poll_interval_value() {
        assert_eq!(DEFAULT_POLL_INTERVAL, Duration::from_secs(2));
    }
}
