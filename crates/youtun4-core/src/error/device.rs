use std::path::PathBuf;
use thiserror::Error;

/// Errors related to device detection and management.
#[derive(Debug, Error)]
pub enum DeviceError {
    /// Device not found or not connected.
    #[error("device not found: {name}")]
    NotFound {
        /// Device name or identifier.
        name: String,
    },

    /// Device is not mounted or accessible.
    #[error("device not mounted at {mount_point}")]
    NotMounted {
        /// Expected mount point.
        mount_point: PathBuf,
    },

    /// Device was disconnected during operation.
    #[error("device '{name}' was disconnected during operation")]
    Disconnected {
        /// Device name.
        name: String,
    },

    /// Device is read-only.
    #[error("device '{name}' is read-only")]
    ReadOnly {
        /// Device name.
        name: String,
    },

    /// Insufficient space on device.
    #[error(
        "insufficient space on device '{device}': {available_bytes} bytes available, {required_bytes} bytes required"
    )]
    InsufficientSpace {
        /// Device name.
        device: String,
        /// Available space in bytes.
        available_bytes: u64,
        /// Required space in bytes.
        required_bytes: u64,
    },

    /// Permission denied for device access.
    #[error("permission denied for device at {path}: {reason}")]
    PermissionDenied {
        /// Device path.
        path: PathBuf,
        /// Reason for denial.
        reason: String,
    },

    /// Unsupported file system type.
    #[error("unsupported file system '{file_system}' on device '{device}'")]
    UnsupportedFileSystem {
        /// Device name.
        device: String,
        /// File system type.
        file_system: String,
    },

    /// Device enumeration failed.
    #[error("failed to enumerate devices: {reason}")]
    EnumerationFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Mount operation failed.
    #[error("failed to mount device '{device}' at {mount_point}: {reason}")]
    MountFailed {
        /// Device name or identifier.
        device: String,
        /// Target mount point.
        mount_point: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Unmount operation failed.
    #[error("failed to unmount device at {mount_point}: {reason}")]
    UnmountFailed {
        /// Mount point to unmount.
        mount_point: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Device is busy and cannot be unmounted.
    #[error("device at {mount_point} is busy: {reason}")]
    DeviceBusy {
        /// Mount point of the busy device.
        mount_point: PathBuf,
        /// Reason or processes using the device.
        reason: String,
    },

    /// Mount point already exists or is in use.
    #[error("mount point {mount_point} already exists or is in use")]
    MountPointInUse {
        /// The mount point that's already in use.
        mount_point: PathBuf,
    },

    /// Platform not supported for mount operations.
    #[error("mount operations not supported on platform: {platform}")]
    PlatformNotSupported {
        /// Platform identifier.
        platform: String,
    },
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "acceptable in tests"
)]
mod tests {
    use super::*;
    use crate::error::{Error, ErrorKind};

    #[test]
    fn test_device_not_found_error() {
        let err = Error::device_not_found("my-device");
        assert_eq!(err.to_string(), "device not found: my-device");
        assert_eq!(err.kind(), ErrorKind::Device);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_device_not_mounted_error() {
        let err = Error::device_not_mounted("/mnt/usb");
        assert_eq!(err.to_string(), "device not mounted at /mnt/usb");
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    fn test_device_disconnected_error() {
        let err = Error::Device(DeviceError::Disconnected {
            name: "USB Drive".to_string(),
        });
        assert!(err.to_string().contains("USB Drive"));
        assert!(err.to_string().contains("disconnected"));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_insufficient_space_error() {
        let err = Error::insufficient_space("USB Drive", 1_000_000, 5_000_000);
        let msg = err.to_string();
        assert!(msg.contains("USB Drive"));
        assert!(msg.contains("1000000"));
        assert!(msg.contains("5000000"));
    }

    #[test]
    fn test_device_read_only_error() {
        let err = Error::Device(DeviceError::ReadOnly {
            name: "SD Card".to_string(),
        });
        assert!(err.to_string().contains("read-only"));
        assert!(err.to_string().contains("SD Card"));
        assert_eq!(err.kind(), ErrorKind::Device);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_device_permission_denied_error() {
        let err = Error::Device(DeviceError::PermissionDenied {
            path: PathBuf::from("/Volumes/Restricted"),
            reason: "root access required".to_string(),
        });
        assert!(err.to_string().contains("permission denied"));
        assert!(err.to_string().contains("root access"));
        assert_eq!(err.kind(), ErrorKind::Device);
    }

    #[test]
    fn test_device_unsupported_filesystem_error() {
        let err = Error::Device(DeviceError::UnsupportedFileSystem {
            device: "USB Drive".to_string(),
            file_system: "NTFS".to_string(),
        });
        assert!(err.to_string().contains("unsupported file system"));
        assert!(err.to_string().contains("NTFS"));
    }

    #[test]
    fn test_device_enumeration_failed_error() {
        let err = Error::Device(DeviceError::EnumerationFailed {
            reason: "system error".to_string(),
        });
        assert!(err.to_string().contains("enumerate"));
        assert!(err.to_string().contains("system error"));
    }

    #[test]
    fn test_mount_failed_error() {
        let err = Error::mount_failed("disk2", "/Volumes/USB", "device busy");
        assert!(err.to_string().contains("mount"));
        assert!(err.to_string().contains("disk2"));
        assert!(err.to_string().contains("device busy"));
    }

    #[test]
    fn test_unmount_failed_error() {
        let err = Error::unmount_failed("/Volumes/USB", "device in use");
        assert!(err.to_string().contains("unmount"));
        assert!(err.to_string().contains("device in use"));
    }

    #[test]
    fn test_device_busy_error() {
        let err = Error::device_busy("/Volumes/USB", "process PID 1234 using device");
        assert!(err.to_string().contains("busy"));
        assert!(err.to_string().contains("PID 1234"));
    }

    #[test]
    fn test_mount_point_in_use_error() {
        let err = Error::Device(DeviceError::MountPointInUse {
            mount_point: PathBuf::from("/Volumes/USB"),
        });
        assert!(err.to_string().contains("mount point"));
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_platform_not_supported_error() {
        let err = Error::platform_not_supported("wasm");
        assert!(err.to_string().contains("not supported"));
        assert!(err.to_string().contains("wasm"));
    }
}
