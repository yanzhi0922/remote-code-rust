//! Configuration error types.

use thiserror::Error;

/// Errors that can occur during configuration operations.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A permission error occurred.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// A configuration file parsing error.
    #[error("Parse error in {path}: {message}")]
    ParseError {
        /// Path to the configuration file.
        path: String,
        /// Error message.
        message: String,
    },
}
