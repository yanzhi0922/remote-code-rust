//! Server error types.
//!
//! Source: `packages/types/src/ipc.ts` — error handling patterns

use thiserror::Error;

/// Errors that can occur in the server layer.
#[derive(Error, Debug)]
pub enum ServerError {
    /// The requested JSON-RPC method was not found.
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Invalid parameters for the requested method.
    #[error("Invalid params for {method}: {detail}")]
    InvalidParams {
        method: String,
        detail: String,
    },

    /// The server has not been initialized.
    #[error("Server not initialized")]
    NotInitialized,

    /// The server is already initialized.
    #[error("Server already initialized")]
    AlreadyInitialized,

    /// The server has been shut down.
    #[error("Server shut down")]
    ShutDown,

    /// Application-level error.
    #[error("App error: {0}")]
    App(#[from] roo_app::AppError),

    /// IO error during transport.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Internal server error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Convenience result type for server operations.
pub type ServerResult<T> = Result<T, ServerError>;
