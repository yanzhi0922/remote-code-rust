//! MCP JSON-RPC client.
//!
//! Implements the MCP protocol client that communicates over a transport layer.

use std::sync::atomic::{AtomicU64, Ordering};

use roo_types::mcp::{
    McpResource, McpResourceContent, McpResourceResponse, McpResourceTemplate, McpTool,
    McpToolCallResponse, McpToolResultContent,
};
use serde::{Deserialize, Serialize};
use tracing;

use crate::error::{McpError, McpResult};
use crate::transport::{JsonRpcMessage, McpTransport};

/// Next request ID counter (global, shared across all clients).
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Client capabilities sent during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapability>,
}

/// Roots capability (list of workspace roots).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootsCapability {
    #[serde(default)]
    pub list_changed: bool,
}

/// Implementation info sent during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationInfo {
    pub name: String,
    pub version: String,
}

/// Server capabilities received during initialization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
}

/// Tools capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    #[serde(default)]
    pub list_changed: bool,
}

/// Resources capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesCapability {
    #[serde(default)]
    pub subscribe: bool,
    #[serde(default)]
    pub list_changed: bool,
}

/// Result of the initialize handshake.
#[derive(Debug, Clone)]
pub struct InitializeResult {
    /// Server capabilities.
    pub capabilities: ServerCapabilities,
    /// Server implementation info.
    pub server_info: ImplementationInfo,
    /// Protocol version.
    pub protocol_version: String,
    /// Server instructions (optional).
    pub instructions: Option<String>,
}

/// MCP JSON-RPC client.
///
/// Communicates with an MCP server over a transport layer using JSON-RPC 2.0.
pub struct McpClient {
    /// Client name.
    name: String,
    /// Client version.
    version: String,
    /// Whether the client has been initialized.
    initialized: bool,
    /// Server capabilities (set after initialization).
    server_capabilities: Option<ServerCapabilities>,
    /// Server instructions (set after initialization).
    instructions: Option<String>,
}

impl McpClient {
    /// Create a new MCP client.
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            initialized: false,
            server_capabilities: None,
            instructions: None,
        }
    }

    /// Perform the MCP initialize handshake.
    ///
    /// Sends an `initialize` request and then a `notifications/initialized` notification.
    pub async fn initialize(
        &mut self,
        transport: &mut dyn McpTransport,
    ) -> McpResult<InitializeResult> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "experimental": {},
                "roots": { "listChanged": true }
            },
            "clientInfo": {
                "name": self.name,
                "version": self.version
            }
        });

        let request = JsonRpcMessage::request(id, "initialize", params);
        transport.send(&request).await?;

        // Read the response
        let response = Self::read_response(transport, id).await?;

        // Parse the initialize result
        let result_obj = response.result.ok_or_else(|| {
            McpError::ConnectionFailed("Initialize response missing 'result' field".to_string())
        })?;

        let capabilities: ServerCapabilities = serde_json::from_value(
            result_obj
                .get("capabilities")
                .cloned()
                .unwrap_or(serde_json::Value::Object(Default::default())),
        )?;

        let server_info: ImplementationInfo = serde_json::from_value(
            result_obj.get("serverInfo").cloned().unwrap_or_else(|| {
                serde_json::json!({"name": "unknown", "version": "0.0.0"})
            }),
        )?;

        let protocol_version = result_obj
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("2024-11-05")
            .to_string();

        let instructions = result_obj
            .get("instructions")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Send initialized notification
        let notification =
            JsonRpcMessage::notification("notifications/initialized", serde_json::json!({}));
        transport.send(&notification).await?;

        self.server_capabilities = Some(capabilities.clone());
        self.instructions = instructions.clone();
        self.initialized = true;

        tracing::info!(
            "MCP client initialized: server={} v{}, protocol={}",
            server_info.name,
            server_info.version,
            protocol_version
        );

        Ok(InitializeResult {
            capabilities,
            server_info,
            protocol_version,
            instructions,
        })
    }

    /// List tools available on the server.
    pub async fn list_tools(
        &mut self,
        transport: &mut dyn McpTransport,
    ) -> McpResult<Vec<McpTool>> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcMessage::request(id, "tools/list", serde_json::json!({}));
        transport.send(&request).await?;

        let response = Self::read_response(transport, id).await?;
        let result = response.result.ok_or_else(|| {
            McpError::ConnectionFailed("tools/list response missing 'result'".to_string())
        })?;

        let tools: Vec<McpTool> = serde_json::from_value(
            result
                .get("tools")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )?;

        tracing::debug!("Listed {} tools from server", tools.len());
        Ok(tools)
    }

    /// Call a tool on the server.
    pub async fn call_tool(
        &mut self,
        transport: &mut dyn McpTransport,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> McpResult<McpToolCallResponse> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments.unwrap_or(serde_json::Value::Object(Default::default()))
        });

        let request = JsonRpcMessage::request(id, "tools/call", params);
        transport.send(&request).await?;

        let response = Self::read_response(transport, id).await?;

        // Check for error
        if let Some(error) = response.error {
            return Err(McpError::ToolCallFailed {
                server_name: String::new(),
                tool_name: name.to_string(),
                error: error.message,
            });
        }

        let result = response.result.ok_or_else(|| {
            McpError::ToolCallFailed {
                server_name: String::new(),
                tool_name: name.to_string(),
                error: "Response missing 'result' field".to_string(),
            }
        })?;

        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let content: Vec<McpToolResultContent> = serde_json::from_value(
            result
                .get("content")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )?;

        Ok(McpToolCallResponse {
            content,
            is_error: if is_error { Some(true) } else { None },
        })
    }

    /// List resources available on the server.
    pub async fn list_resources(
        &mut self,
        transport: &mut dyn McpTransport,
    ) -> McpResult<Vec<McpResource>> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcMessage::request(id, "resources/list", serde_json::json!({}));
        transport.send(&request).await?;

        let response = Self::read_response(transport, id).await?;
        let result = response.result.ok_or_else(|| {
            McpError::ConnectionFailed("resources/list response missing 'result'".to_string())
        })?;

        let resources: Vec<McpResource> = serde_json::from_value(
            result
                .get("resources")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )?;

        tracing::debug!("Listed {} resources from server", resources.len());
        Ok(resources)
    }

    /// Read a resource from the server.
    pub async fn read_resource(
        &mut self,
        transport: &mut dyn McpTransport,
        uri: &str,
    ) -> McpResult<McpResourceResponse> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let params = serde_json::json!({ "uri": uri });
        let request = JsonRpcMessage::request(id, "resources/read", params);
        transport.send(&request).await?;

        let response = Self::read_response(transport, id).await?;

        if let Some(error) = response.error {
            return Err(McpError::ResourceReadFailed {
                server_name: String::new(),
                uri: uri.to_string(),
                error: error.message,
            });
        }

        let result = response.result.ok_or_else(|| {
            McpError::ResourceReadFailed {
                server_name: String::new(),
                uri: uri.to_string(),
                error: "Response missing 'result' field".to_string(),
            }
        })?;

        let contents: Vec<McpResourceContent> = serde_json::from_value(
            result
                .get("contents")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )?;

        Ok(McpResourceResponse { contents })
    }

    /// List resource templates available on the server.
    pub async fn list_resource_templates(
        &mut self,
        transport: &mut dyn McpTransport,
    ) -> McpResult<Vec<McpResourceTemplate>> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let request =
            JsonRpcMessage::request(id, "resources/templates/list", serde_json::json!({}));
        transport.send(&request).await?;

        let response = Self::read_response(transport, id).await?;
        let result = response.result.ok_or_else(|| {
            McpError::ConnectionFailed(
                "resources/templates/list response missing 'result'".to_string(),
            )
        })?;

        let templates: Vec<McpResourceTemplate> = serde_json::from_value(
            result
                .get("resourceTemplates")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )?;

        tracing::debug!("Listed {} resource templates from server", templates.len());
        Ok(templates)
    }

    /// Get server instructions (from the initialize response).
    pub fn get_instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Check if the client has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the server capabilities (if initialized).
    pub fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    /// Read a response from the transport, matching the expected request ID.
    ///
    /// Skips any notifications or responses with different IDs.
    async fn read_response(
        transport: &mut dyn McpTransport,
        expected_id: u64,
    ) -> McpResult<JsonRpcMessage> {
        // Try to read messages until we find the one with our expected ID
        let mut attempts = 0;
        loop {
            match transport.receive().await? {
                Some(msg) => {
                    // Check if this is a response to our request
                    if let Some(msg_id) = msg.id_as_u64() {
                        if msg_id == expected_id {
                            // Check for JSON-RPC error
                            if let Some(error) = &msg.error {
                                return Err(McpError::JsonRpcError {
                                    code: error.code,
                                    message: error.message.clone(),
                                });
                            }
                            return Ok(msg);
                        }
                    }
                    // Not our response, skip it (could be a notification)
                    tracing::trace!("Skipping message with unexpected ID");
                }
                None => {
                    return Err(McpError::ConnectionFailed(
                        "Transport closed while waiting for response".to_string(),
                    ));
                }
            }
            attempts += 1;
            if attempts > 1000 {
                return Err(McpError::ConnectionFailed(
                    "Too many messages without matching response".to_string(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = McpClient::new("Roo Code", "1.0.0");
        assert_eq!(client.name, "Roo Code");
        assert_eq!(client.version, "1.0.0");
        assert!(!client.is_initialized());
        assert!(client.server_capabilities().is_none());
        assert!(client.get_instructions().is_none());
    }

    #[test]
    fn test_initialize_request_format() {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "TestClient",
                "version": "0.1.0"
            }
        });
        let msg = JsonRpcMessage::request(id, "initialize", params);

        assert_eq!(msg.method.as_deref(), Some("initialize"));
        assert!(msg.params.is_some());

        let params = msg.params.unwrap();
        assert_eq!(params["protocolVersion"], "2024-11-05");
        assert_eq!(params["clientInfo"]["name"], "TestClient");
    }

    #[test]
    fn test_server_capabilities_deserialization() {
        let json = serde_json::json!({
            "tools": { "listChanged": true },
            "resources": { "subscribe": false, "listChanged": true },
            "instructions": true
        });
        let caps: ServerCapabilities = serde_json::from_value(json).unwrap();
        assert!(caps.tools.is_some());
        assert!(caps.tools.unwrap().list_changed);
        assert!(caps.resources.is_some());
        assert!(caps.instructions.unwrap());
    }

    #[test]
    fn test_server_capabilities_default() {
        let caps = ServerCapabilities::default();
        assert!(caps.tools.is_none());
        assert!(caps.resources.is_none());
    }
}
