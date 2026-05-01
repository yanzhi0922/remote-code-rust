//! # roo-mcp — MCP (Model Context Protocol) client for Roo Code
//!
//! This crate implements the complete MCP client protocol, including:
//! - **Stdio transport** — communicates with MCP servers via stdin/stdout pipes
//! - **SSE transport** — uses Server-Sent Events for receiving and HTTP POST for sending
//! - **StreamableHTTP transport** — standard HTTP request/response communication
//!
//! ## Architecture
//!
//! - [`hub::McpHub`] — Central connection manager (corresponds to TS `McpHub`)
//! - [`manager::McpServerManager`] — Singleton manager for hub instances
//! - [`client::McpClient`] — JSON-RPC 2.0 client for MCP protocol
//! - [`transport`] — Transport layer trait and implementations
//! - [`config`] — Server configuration validation
//! - [`name_utils`] — MCP name sanitization and matching
//! - [`types`] — Internal connection types

pub mod client;
pub mod config;
pub mod error;
pub mod hub;
pub mod manager;
pub mod name_utils;
pub mod transport;
pub mod types;

// Re-export key types for convenience
pub use error::{McpError, McpResult};
pub use hub::McpHub;
pub use manager::McpServerManager;

pub use client::McpClient;
pub use config::{validate_server_config, ValidatedServerConfig};
pub use hub::{
    get_default_environment, inject_variables, json_deep_equal, merge_environment,
    McpConnectionExt,
};
pub use name_utils::{
    build_mcp_tool_name, is_mcp_tool, normalize_for_comparison, normalize_mcp_tool_name,
    parse_mcp_tool_name, sanitize_mcp_name, tool_names_match, MCP_TOOL_PREFIX,
    MCP_TOOL_SEPARATOR,
};
pub use transport::{
    JsonRpcError, JsonRpcMessage, McpTransport, SseTransport, StdioTransport,
    StreamableHttpTransport,
};
pub use types::{
    ConnectedMcpConnection, DisableReason, DisconnectedMcpConnection, McpConnection,
    McpServerState, McpSource,
};
