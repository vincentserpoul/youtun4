//! Error handling utilities for Tauri commands.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::error;
use youtun4_core::{Error, ErrorKind};

/// Structured error response for Tauri IPC.
///
/// Includes both the error message and the error kind for frontend handling.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorResponse {
    /// Human-readable error message.
    pub message: String,
    /// Error category for programmatic handling.
    pub kind: String,
    /// Whether the error can be retried.
    pub retryable: bool,
    /// Suggested retry delay in seconds, if applicable.
    pub retry_delay_secs: Option<u64>,
}

impl From<&Error> for ErrorResponse {
    fn from(e: &Error) -> Self {
        Self {
            message: e.to_string(),
            kind: format!("{:?}", e.kind()),
            retryable: e.is_retryable(),
            retry_delay_secs: e.retry_delay_secs(),
        }
    }
}

/// Convert our error type to a string for Tauri.
///
/// The returned string is JSON-encoded `ErrorResponse` for structured error handling
/// in the frontend. Falls back to plain error message if serialization fails.
#[allow(
    clippy::needless_pass_by_value,
    reason = "consumed from .map_err(map_err) closures"
)]
pub fn map_err(e: Error) -> String {
    let kind = e.kind();
    let is_retryable = e.is_retryable();

    error!(
        "Command error [kind={:?}, retryable={}]: {}",
        kind, is_retryable, e
    );

    // Try to return structured JSON for the frontend
    let response = ErrorResponse::from(&e);
    serde_json::to_string(&response).unwrap_or_else(|_| e.to_string())
}

/// Get the error kind from an error, useful for testing.
#[allow(dead_code, reason = "utility function for testing")]
pub fn error_kind(e: &Error) -> ErrorKind {
    e.kind()
}

/// Emit a Tauri event, logging on failure instead of propagating the error.
///
/// Most event emissions in command handlers are fire-and-forget notifications
/// to the frontend. This helper reduces the repetitive `if let Err` pattern.
pub fn emit_or_log(app: &AppHandle, event: &str, payload: &impl Serialize) {
    if let Err(e) = app.emit(event, payload) {
        error!("Failed to emit {event} event: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_response_always_serializes() {
        let response = ErrorResponse {
            message: "something broke".to_string(),
            kind: "Io".to_string(),
            retryable: true,
            retry_delay_secs: Some(5),
        };
        serde_json::to_string(&response).expect("ErrorResponse must always serialize");
    }
}
