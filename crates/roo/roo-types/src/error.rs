//! Error types for the roo-types crate.

use thiserror::Error;

/// General error type for Roo operations.
#[derive(Debug, Error)]
pub enum RooError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Context error: {0}")]
    Context(String),

    #[error("Task error: {0}")]
    Task(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Token limit exceeded: {used} / {limit}")]
    TokenLimit { used: u64, limit: u64 },

    #[error("File restriction: {0}")]
    FileRestriction(String),

    #[error("Mode not found: {0}")]
    ModeNotFound(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Invalid parameter: {0}")]
    InvalidParam(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

/// Result type alias for Roo operations.
pub type RooResult<T> = Result<T, RooError>;
