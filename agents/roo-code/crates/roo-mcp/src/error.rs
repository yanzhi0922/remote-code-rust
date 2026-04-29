//! MCP error types.

use thiserror::Error;

/// Errors that can occur in the MCP client.
#[derive(Debug, Error)]
pub enum McpError {
    /// The server configuration is invalid.
    #[error("{0}")]
    ConfigError(String),

    /// Connection to the MCP server failed.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// The server is not connected.
    #[error("Server '{0}' is not connected")]
    NotConnected(String),

    /// The server was not found.
    #[error("Server '{0}' was not found")]
    ServerNotFound(String),

    /// A tool call failed.
    #[error("Tool call failed for '{tool_name}' on server '{server_name}': {error}")]
    ToolCallFailed {
        server_name: String,
        tool_name: String,
        error: String,
    },

    /// A resource read failed.
    #[error("Resource read failed for '{uri}' on server '{server_name}': {error}")]
    ResourceReadFailed {
        server_name: String,
        uri: String,
        error: String,
    },

    /// A JSON-RPC protocol error.
    #[error("JSON-RPC error (code {code}): {message}")]
    JsonRpcError { code: i64, message: String },

    /// A transport-level error.
    #[error("Transport error: {0}")]
    TransportError(String),

    /// A serialization/deserialization error.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// An I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// An HTTP request error.
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// A URL parse error.
    #[error("URL parse error: {0}")]
    UrlError(#[from] url::ParseError),

    /// The operation timed out.
    #[error("Operation timed out after {0}ms")]
    Timeout(u64),

    /// The hub has been disposed.
    #[error("McpHub has been disposed")]
    Disposed,

    /// MCP is globally disabled.
    #[error("MCP is globally disabled")]
    McpDisabled,

    /// The server is disabled.
    #[error("Server '{0}' is disabled")]
    ServerDisabled(String),

    /// Generic error.
    #[error("{0}")]
    Other(String),
}

/// Result type alias for MCP operations.
pub type McpResult<T> = Result<T, McpError>;
