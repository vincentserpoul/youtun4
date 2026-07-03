use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;

use crate::device::{DeviceDetector, DeviceInfo, DeviceManager};

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
#[derive(Debug)]
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;

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
            DeviceEvent::Disconnected(_) | DeviceEvent::Refreshed(_) => {
                panic!("Expected Connected event")
            }
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
            DeviceEvent::Connected(_) | DeviceEvent::Disconnected(_) => {
                panic!("Expected Refreshed event")
            }
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

    #[test]
    fn test_default_poll_interval_value() {
        assert_eq!(DEFAULT_POLL_INTERVAL, Duration::from_secs(2));
    }
}
