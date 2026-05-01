//! Types for MCP tool results and errors.

use serde::{Deserialize, Serialize};

/// Validation result for MCP tool parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolValidation {
    pub server_name: String,
    pub tool_name: String,
    pub is_valid: bool,
    pub error: Option<String>,
}

/// Result of an MCP resource access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceResult {
    pub server_name: String,
    pub uri: String,
    pub content: String,
    pub content_type: String,
}

/// Result of an MCP tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub server_name: String,
    pub tool_name: String,
    pub result: serde_json::Value,
    pub is_error: bool,
}

/// Result of executing an MCP tool call via McpHub.
///
/// Contains formatted text output and optional base64-encoded images.
#[derive(Debug, Clone)]
pub struct McpToolExecutionResult {
    /// The formatted text output from the tool call.
    pub text: String,
    /// Optional base64-encoded images returned by the tool.
    pub images: Vec<String>,
    /// Whether the tool execution resulted in an error.
    pub is_error: bool,
}

impl McpToolExecutionResult {
    /// Create a successful execution result.
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            images: Vec::new(),
            is_error: false,
        }
    }

    /// Create a successful execution result with images.
    pub fn success_with_images(text: impl Into<String>, images: Vec<String>) -> Self {
        Self {
            text: text.into(),
            images,
            is_error: false,
        }
    }

    /// Create an error execution result.
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            images: Vec::new(),
            is_error: true,
        }
    }
}

/// Result of executing an MCP resource read via McpHub.
///
/// Contains formatted text output from the resource.
#[derive(Debug, Clone)]
pub struct McpResourceExecutionResult {
    /// The formatted text output from the resource read.
    pub text: String,
    /// Whether the resource read resulted in an error.
    pub is_error: bool,
}

impl McpResourceExecutionResult {
    /// Create a successful resource execution result.
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }

    /// Create an error resource execution result.
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
}

/// Error type for MCP tool operations.
#[derive(Debug, thiserror::Error)]
pub enum McpToolError {
    #[error("Missing parameter: {0}")]
    MissingParameter(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool name mismatch: expected '{expected}', got '{actual}'")]
    ToolNameMismatch { expected: String, actual: String },

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("MCP error: {0}")]
    Mcp(String),
}
