//! Internal connection types for MCP.
//!
//! Corresponds to TS: `ConnectedMcpConnection` / `DisconnectedMcpConnection` / `McpConnection`.

use roo_types::mcp::{
    McpConnectionStatus, McpErrorEntry, McpResource, McpResourceTemplate, McpTool,
};

use crate::client::McpClient;
use crate::transport::McpTransport;

/// The source of an MCP server configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSource {
    /// Global configuration (user-level settings).
    Global,
    /// Project-level configuration (.roo/mcp.json).
    Project,
}

/// The reason a server connection is disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisableReason {
    /// MCP is globally disabled.
    McpDisabled,
    /// The individual server is disabled.
    ServerDisabled,
}

/// Internal server state tracked by the hub.
#[derive(Debug, Clone)]
pub struct McpServerState {
    /// The server name.
    pub name: String,
    /// JSON string of the original validated configuration.
    pub config: String,
    /// Current connection status.
    pub status: McpConnectionStatus,
    /// Whether this server is explicitly disabled.
    pub disabled: bool,
    /// Where this server configuration comes from.
    pub source: McpSource,
    /// For project-level servers, the project path.
    pub project_path: Option<String>,
    /// Last error message (truncated to 1000 chars).
    pub error: String,
    /// History of errors (max 100 entries).
    pub error_history: Vec<McpErrorEntry>,
    /// Tools discovered from this server.
    pub tools: Vec<McpTool>,
    /// Resources discovered from this server.
    pub resources: Vec<McpResource>,
    /// Resource templates discovered from this server.
    pub resource_templates: Vec<McpResourceTemplate>,
    /// Server instructions (from initialize response).
    pub instructions: Option<String>,
}

impl McpServerState {
    /// Maximum number of error history entries to retain.
    pub const MAX_ERROR_HISTORY: usize = 100;
    /// Maximum length of a single error message.
    pub const MAX_ERROR_LENGTH: usize = 1000;

    /// Create a new server state with the given name, config, and source.
    pub fn new(name: String, config: String, source: McpSource) -> Self {
        Self {
            name,
            config,
            status: McpConnectionStatus::Disconnected,
            disabled: false,
            source,
            project_path: None,
            error: String::new(),
            error_history: Vec::new(),
            tools: Vec::new(),
            resources: Vec::new(),
            resource_templates: Vec::new(),
            instructions: None,
        }
    }

    /// Append an error message, truncating to max length and adding to history.
    ///
    /// Corresponds to TS: `McpHub.appendErrorMessage(connection, error, level)`.
    /// Adds "...(error message truncated)" suffix when truncated.
    /// Supports error levels: "error", "warn", "info" (matching TS).
    pub fn append_error(
        &mut self,
        error_msg: &str,
        level: crate::hub::ErrorLevel,
    ) {
        let truncated = if error_msg.len() > Self::MAX_ERROR_LENGTH {
            format!(
                "{}...(error message truncated)",
                &error_msg[..Self::MAX_ERROR_LENGTH]
            )
        } else {
            error_msg.to_string()
        };

        self.error = truncated.clone();

        self.error_history.push(McpErrorEntry {
            server_name: self.name.clone(),
            error: truncated,
            hints: Vec::new(),
        });

        // Keep only the last MAX_ERROR_HISTORY entries
        if self.error_history.len() > Self::MAX_ERROR_HISTORY {
            let drain_count = self.error_history.len() - Self::MAX_ERROR_HISTORY;
            self.error_history.drain(..drain_count);
        }

        // Log based on level
        match level {
            crate::hub::ErrorLevel::Error => {
                tracing::error!("[{}] {}", self.name, self.error);
            }
            crate::hub::ErrorLevel::Warn => {
                tracing::warn!("[{}] {}", self.name, self.error);
            }
            crate::hub::ErrorLevel::Info => {
                tracing::info!("[{}] {}", self.name, self.error);
            }
        }
    }
}

/// A connected MCP server with its client and transport.
pub struct ConnectedMcpConnection {
    /// Server state.
    pub server: McpServerState,
    /// The MCP JSON-RPC client.
    pub client: McpClient,
    /// The transport layer.
    pub transport: Box<dyn McpTransport>,
}

/// A disconnected MCP server placeholder.
pub struct DisconnectedMcpConnection {
    /// Server state.
    pub server: McpServerState,
}

/// An MCP connection, either connected or disconnected.
pub enum McpConnection {
    /// A fully connected server with client and transport.
    Connected(ConnectedMcpConnection),
    /// A disconnected or disabled server placeholder.
    Disconnected(DisconnectedMcpConnection),
}

impl McpConnection {
    /// Get a reference to the server state.
    pub fn server(&self) -> &McpServerState {
        match self {
            McpConnection::Connected(c) => &c.server,
            McpConnection::Disconnected(c) => &c.server,
        }
    }

    /// Get a mutable reference to the server state.
    pub fn server_mut(&mut self) -> &mut McpServerState {
        match self {
            McpConnection::Connected(c) => &mut c.server,
            McpConnection::Disconnected(c) => &mut c.server,
        }
    }

    /// Check if this connection is connected.
    pub fn is_connected(&self) -> bool {
        matches!(self, McpConnection::Connected(_))
    }

    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.server().name
    }

    /// Get the source of this connection.
    pub fn source(&self) -> McpSource {
        self.server().source
    }
}
