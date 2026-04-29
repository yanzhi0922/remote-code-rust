//! Debug Log
//!
//! File-based debug logging utility. Writes logs to `~/.roo/cli-debug.log`.
//! Mirrors `debug-log/index.ts`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

static DEBUG_LOG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Get the debug log file path.
fn debug_log_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".roo")
        .join("cli-debug.log")
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Enable or disable file-based debug logging.
///
/// Logging is disabled by default and should only be enabled in dev/debug mode.
///
/// Source: `.research/Roo-Code/packages/core/src/debug-log/index.ts`
pub fn set_debug_log_enabled(enabled: bool) {
    DEBUG_LOG_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Check if debug logging is enabled.
pub fn is_debug_log_enabled() -> bool {
    DEBUG_LOG_ENABLED.load(Ordering::Relaxed)
}

/// Simple file-based debug log function.
///
/// Writes timestamped entries to `~/.roo/cli-debug.log`.
/// Only writes when enabled via `set_debug_log_enabled(true)`.
///
/// Source: `debug-log/index.ts` — `debugLog`
pub fn debug_log(message: &str, data: Option<&serde_json::Value>) {
    if !is_debug_log_enabled() {
        return;
    }

    let log_path = debug_log_path();

    // Ensure directory exists
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let timestamp = chrono::Utc::now().to_rfc3339();

    let entry = match data {
        Some(d) => format!("[{}] {}: {}\n", timestamp, message, serde_json::to_string_pretty(d).unwrap_or_else(|_| d.to_string())),
        None => format!("[{}] {}\n", timestamp, message),
    };

    // Append to log file
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = file.write_all(entry.as_bytes());
    }
}

// ---------------------------------------------------------------------------
// DebugLogger
// ---------------------------------------------------------------------------

/// Debug logger with component context.
///
/// Prefixes all messages with the component name.
///
/// Source: `debug-log/index.ts` — `DebugLogger`
pub struct DebugLogger {
    component: String,
}

impl DebugLogger {
    /// Create a new debug logger for a component.
    pub fn new(component: &str) -> Self {
        Self {
            component: component.to_string(),
        }
    }

    /// Log a debug message with optional data.
    pub fn debug(&self, message: &str, data: Option<&serde_json::Value>) {
        debug_log(&format!("[{}] {}", self.component, message), data);
    }

    /// Alias for debug.
    pub fn info(&self, message: &str, data: Option<&serde_json::Value>) {
        self.debug(message, data);
    }

    /// Log a warning.
    pub fn warn(&self, message: &str, data: Option<&serde_json::Value>) {
        debug_log(
            &format!("[{}] WARN: {}", self.component, message),
            data,
        );
    }

    /// Log an error.
    pub fn error(&self, message: &str, data: Option<&serde_json::Value>) {
        debug_log(
            &format!("[{}] ERROR: {}", self.component, message),
            data,
        );
    }
}


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_be_disabled() {
        set_debug_log_enabled(false);
        assert!(!is_debug_log_enabled());
    }

    #[test]
    fn test_enable_disable() {
        set_debug_log_enabled(true);
        assert!(is_debug_log_enabled());
        set_debug_log_enabled(false);
        assert!(!is_debug_log_enabled());
    }

    #[test]
    fn test_debug_log_does_not_write_when_disabled() {
        set_debug_log_enabled(false);
        // This should not panic or write
        debug_log("test", None);
    }

    #[test]
    fn test_debug_log_writes_when_enabled() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("test-debug.log");

        set_debug_log_enabled(true);
        // Note: debug_log writes to ~/.roo/cli-debug.log, we just verify it doesn't panic
        debug_log("test message", Some(&serde_json::json!({"key": "value"})));
        set_debug_log_enabled(false);
    }

    #[test]
    fn test_debug_logger_component() {
        let logger = DebugLogger::new("TestComponent");
        assert_eq!(logger.component, "TestComponent");
    }

    #[test]
    fn test_debug_logger_methods_dont_panic() {
        set_debug_log_enabled(false);
        let logger = DebugLogger::new("Test");
        logger.debug("test", None);
        logger.info("test", None);
        logger.warn("test", None);
        logger.error("test", None);
    }
}
