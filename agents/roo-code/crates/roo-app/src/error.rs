//! Application-level error types.

use thiserror::Error;

/// Errors that can occur in the application layer.
#[derive(Error, Debug)]
pub enum AppError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Provider error.
    #[error("Provider error: {0}")]
    Provider(#[from] roo_provider::ProviderError),

    /// Task error.
    #[error("Task error: {0}")]
    Task(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// General error.
    #[error("{0}")]
    Other(String),
}

/// Convenience result type for the application layer.
pub type AppResult<T> = Result<T, AppError>;
