//! Shared timestamp utilities.

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current time as a Unix timestamp in seconds.
///
/// Returns `0` if the system clock is before the Unix epoch.
#[must_use]
pub fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Converts a [`SystemTime`] to a Unix timestamp in seconds.
///
/// Returns `0` if the time is before the Unix epoch.
#[must_use]
pub fn system_time_to_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
