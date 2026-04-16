//! Core MCP type definitions for client info, tool descriptors, and server inspection.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Client identification sent during MCP initialisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpClientInfo {
    /// Client name.
    pub name: String,
    /// Client version.
    pub version: String,
}

impl McpClientInfo {
    /// Create a new client info with the given name and version.
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

impl Default for McpClientInfo {
    fn default() -> Self {
        Self::new("remote-code-rust", env!("CARGO_PKG_VERSION"))
    }
}

/// Peer (server) identification returned during MCP initialisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpPeerInfo {
    /// Server name.
    pub name: String,
    /// Human-readable title.
    #[serde(default)]
    pub title: Option<String>,
    /// Server version.
    #[serde(default)]
    pub version: Option<String>,
}

/// A tool descriptor returned by an MCP server via `tools/list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDescriptor {
    /// Tool name (unique within the server).
    pub name: String,
    /// Human-readable title.
    #[serde(default)]
    pub title: Option<String>,
    /// Tool description.
    #[serde(default)]
    pub description: Option<String>,
    /// JSON Schema for tool input.
    #[serde(default)]
    pub input_schema: Value,
    /// Tool annotations (e.g. `readOnlyHint`).
    #[serde(default)]
    pub annotations: Value,
}

/// Full inspection result from an MCP server (initialize + tools/list).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInspection {
    /// Server name from the config key.
    pub server_name: String,
    /// Negotiated protocol version.
    pub protocol_version: String,
    /// Server identification.
    #[serde(default)]
    pub server_info: Option<McpPeerInfo>,
    /// Server capabilities (raw JSON).
    #[serde(default)]
    pub capabilities: Value,
    /// Server instructions for the client.
    #[serde(default)]
    pub instructions: Option<String>,
    /// Available tools.
    #[serde(default)]
    pub tools: Vec<McpToolDescriptor>,
}

/// A single content block in a tool call result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolCallContent {
    /// Content block type (e.g. `"text"`, `"image"`).
    #[serde(rename = "type")]
    pub kind: String,
    /// Additional fields for the content block.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// The result payload of a `tools/call` invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallResult {
    /// Content blocks returned by the tool.
    #[serde(default)]
    pub content: Vec<McpToolCallContent>,
    /// Optional structured content (JSON).
    #[serde(default)]
    pub structured_content: Option<Value>,
    /// Whether the tool invocation resulted in an error.
    #[serde(default)]
    pub is_error: bool,
}

/// Full response from a `tools/call` invocation including server metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallResponse {
    /// Server name from the config key.
    pub server_name: String,
    /// Tool name that was invoked.
    pub tool_name: String,
    /// Negotiated protocol version.
    pub protocol_version: String,
    /// Server identification.
    #[serde(default)]
    pub server_info: Option<McpPeerInfo>,
    /// Tool call result.
    pub result: McpToolCallResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_info_default_uses_crate_name() {
        let info = McpClientInfo::default();
        assert_eq!(info.name, "remote-code-rust");
        assert!(!info.version.is_empty());
    }

    #[test]
    fn client_info_new_custom() {
        let info = McpClientInfo::new("my-app", "1.0.0");
        assert_eq!(info.name, "my-app");
        assert_eq!(info.version, "1.0.0");
    }

    #[test]
    fn peer_info_serialization_roundtrip() {
        let peer = McpPeerInfo {
            name: "test-server".to_owned(),
            title: Some("Test Server".to_owned()),
            version: Some("2.0".to_owned()),
        };
        let json = serde_json::to_string(&peer).expect("serialize");
        let back: McpPeerInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(peer, back);
    }

    #[test]
    fn tool_descriptor_deserialization() {
        let json = r#"{"name":"search","description":"Search","inputSchema":{},"annotations":{}}"#;
        let tool: McpToolDescriptor = serde_json::from_str(json).expect("deserialize");
        assert_eq!(tool.name, "search");
        assert_eq!(tool.description.as_deref(), Some("Search"));
    }

    #[test]
    fn tool_call_content_kind_and_fields() {
        let content = McpToolCallContent {
            kind: "text".to_owned(),
            fields: BTreeMap::from([("text".to_owned(), Value::String("hello".to_owned()))]),
        };
        let json = serde_json::to_string(&content).expect("serialize");
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"hello\""));
    }

    #[test]
    fn tool_call_result_is_error_default_false() {
        let result = McpToolCallResult {
            content: vec![],
            structured_content: None,
            is_error: false,
        };
        assert!(!result.is_error);
    }

    #[test]
    fn server_inspection_serialization() {
        let inspection = McpServerInspection {
            server_name: "test".to_owned(),
            protocol_version: "2025-03-26".to_owned(),
            server_info: None,
            capabilities: serde_json::json!({}),
            instructions: Some("Use carefully".to_owned()),
            tools: vec![],
        };
        let json = serde_json::to_string(&inspection).expect("serialize");
        let back: McpServerInspection = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(inspection, back);
    }
}
