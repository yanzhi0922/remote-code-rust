//! MCP (Model Context Protocol) type definitions.
//!
//! Derived from `packages/types/src/mcp.ts`.
//! Defines MCP server, tool, resource, and response types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// McpServerUse
// ---------------------------------------------------------------------------

/// How an MCP server is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpServerUse {
    Stdio,
    Sse,
    StreamableHttp,
}

// ---------------------------------------------------------------------------
// McpTransportConfig
// ---------------------------------------------------------------------------

/// Transport configuration for an MCP server connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpTransportConfig {
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: std::collections::HashMap<String, String>,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        timeout: Option<u64>,
    },
    #[serde(rename = "sse")]
    Sse {
        url: String,
        #[serde(default)]
        headers: std::collections::HashMap<String, String>,
        #[serde(default)]
        timeout: Option<u64>,
    },
    #[serde(rename = "streamable-http")]
    StreamableHttp {
        url: String,
        #[serde(default)]
        headers: std::collections::HashMap<String, String>,
        #[serde(default)]
        timeout: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// McpServer
// ---------------------------------------------------------------------------

/// Configuration for an MCP server.
///
/// Source: `packages/types/src/mcp.ts` — `McpServer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    /// Unique name for this MCP server.
    pub name: String,
    /// Transport configuration.
    #[serde(flatten)]
    pub transport: McpTransportConfig,
    /// Whether this server is disabled.
    #[serde(default)]
    pub disabled: bool,
    /// Whether to always allow tool calls without prompting.
    #[serde(default)]
    pub always_allow: Vec<String>,
    /// Tools that are explicitly disabled.
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    /// Timeout for tool calls in milliseconds.
    #[serde(default)]
    pub timeout: Option<u64>,
}

// ---------------------------------------------------------------------------
// McpTool
// ---------------------------------------------------------------------------

/// An MCP tool definition.
///
/// Source: `packages/types/src/mcp.ts` — `McpTool`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    /// Whether this tool is always allowed (auto-approved) without user confirmation.
    /// Set based on the server's `alwaysAllow` configuration.
    #[serde(default)]
    pub always_allow: bool,
    /// Whether this tool is enabled for inclusion in prompts.
    /// `true` unless the tool is in the server's `disabledTools` list.
    #[serde(default = "default_true")]
    pub enabled_for_prompt: bool,
}

/// Default value for `enabled_for_prompt` (true).
fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// McpResource
// ---------------------------------------------------------------------------

/// An MCP resource definition.
///
/// Source: `packages/types/src/mcp.ts` — `McpResource`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
}

// ---------------------------------------------------------------------------
// McpResourceTemplate
// ---------------------------------------------------------------------------

/// An MCP resource template.
///
/// Source: `packages/types/src/mcp.ts` — `McpResourceTemplate`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceTemplate {
    pub uri_template: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
}

// ---------------------------------------------------------------------------
// McpResourceResponse
// ---------------------------------------------------------------------------

/// Response from accessing an MCP resource.
///
/// Source: `packages/types/src/mcp.ts` — `McpResourceResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceResponse {
    pub contents: Vec<McpResourceContent>,
}

/// A single content item in an MCP resource response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    pub text: String,
}

// ---------------------------------------------------------------------------
// McpToolCallResponse
// ---------------------------------------------------------------------------

/// Response from calling an MCP tool.
///
/// Source: `packages/types/src/mcp.ts` — `McpToolCallResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResponse {
    #[serde(default)]
    pub content: Vec<McpToolResultContent>,
    #[serde(default)]
    pub is_error: Option<bool>,
}

/// Content in an MCP tool call response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpToolResultContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        data: String,
        mime_type: String,
    },
    #[serde(rename = "resource")]
    Resource {
        resource: McpResourceContent,
    },
}

// ---------------------------------------------------------------------------
// McpErrorEntry
// ---------------------------------------------------------------------------

/// An MCP error entry.
///
/// Source: `packages/types/src/mcp.ts` — `McpErrorEntry`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpErrorEntry {
    pub server_name: String,
    pub error: String,
    pub hints: Vec<String>,
}

// ---------------------------------------------------------------------------
// EnabledMcpToolsCount
// ---------------------------------------------------------------------------

/// Count of enabled MCP tools per server.
///
/// Source: `packages/types/src/mcp.ts` — `EnabledMcpToolsCount`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnabledMcpToolsCount {
    pub total: usize,
    pub by_server: std::collections::HashMap<String, usize>,
}

/// Counts enabled MCP tools across all servers.
///
/// Source: `packages/types/src/mcp.ts` — `countEnabledMcpTools`
pub fn count_enabled_mcp_tools(servers: &[McpServerConnection]) -> EnabledMcpToolsCount {
    let mut count = EnabledMcpToolsCount::default();
    for server in servers {
        if server.status != McpConnectionStatus::Connected {
            continue;
        }
        let enabled = server
            .tools
            .iter()
            .filter(|t| !server.disabled_tools.contains(&t.name))
            .count();
        count.by_server.insert(server.name.clone(), enabled);
        count.total += enabled;
    }
    count
}

// ---------------------------------------------------------------------------
// McpConnectionStatus / McpServerConnection
// ---------------------------------------------------------------------------

/// Status of an MCP server connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

/// A connected MCP server with its tools and resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConnection {
    pub name: String,
    pub status: McpConnectionStatus,
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub resource_templates: Vec<McpResourceTemplate>,
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    #[serde(default)]
    pub errors: Vec<McpErrorEntry>,
}
