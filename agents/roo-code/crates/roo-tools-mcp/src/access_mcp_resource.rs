//! access_mcp_resource tool implementation.
//!
//! Provides both parameter validation functions and the full MCP resource
//! access via [`McpHub`].

use std::sync::Arc;

use crate::helpers::*;
use crate::types::*;
use roo_types::tool::AccessMcpResourceParams;

// ---------------------------------------------------------------------------
// Parameter validation (existing, unchanged)
// ---------------------------------------------------------------------------

/// Validate access_mcp_resource parameters.
pub fn validate_access_mcp_resource_params(
    params: &AccessMcpResourceParams,
) -> Result<(), McpToolError> {
    if params.server_name.trim().is_empty() {
        return Err(McpToolError::MissingParameter("server_name".to_string()));
    }

    if params.uri.trim().is_empty() {
        return Err(McpToolError::MissingParameter("uri".to_string()));
    }

    Ok(())
}

/// Process an MCP resource access request.
///
/// Note: The actual resource fetching is handled by the MCP client.
/// This function validates parameters and prepares the result structure.
pub fn prepare_resource_access(
    params: &AccessMcpResourceParams,
) -> Result<McpResourceResult, McpToolError> {
    validate_access_mcp_resource_params(params)?;

    Ok(McpResourceResult {
        server_name: params.server_name.clone(),
        uri: params.uri.clone(),
        content: String::new(), // Filled by actual MCP client
        content_type: "text/plain".to_string(),
    })
}

/// Process raw resource content into a formatted result.
pub fn process_resource_content(
    params: &AccessMcpResourceParams,
    raw_content: &str,
    content_type: &str,
) -> McpResourceResult {
    let formatted = format_resource_content(raw_content, content_type);

    McpResourceResult {
        server_name: params.server_name.clone(),
        uri: params.uri.clone(),
        content: formatted,
        content_type: content_type.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Full MCP resource access via McpHub
// ---------------------------------------------------------------------------

/// Access an MCP resource via [`roo_mcp::McpHub`].
///
/// This is the primary entry point for the `access_mcp_resource` tool handler.
/// It validates parameters, resolves the server name, reads the resource via
/// the hub, and formats the response.
///
/// # Arguments
/// * `hub` — Shared reference to the MCP hub.
/// * `params` — The resource access parameters (server_name, uri).
///
/// # Returns
/// An [`McpResourceExecutionResult`] containing the formatted text output
/// and an error flag.
pub async fn access_mcp_resource(
    hub: &Arc<roo_mcp::McpHub>,
    params: &AccessMcpResourceParams,
) -> McpResourceExecutionResult {
    // 1. Validate parameters
    if let Err(e) = validate_access_mcp_resource_params(params) {
        return McpResourceExecutionResult::error(format!(
            "Parameter validation failed: {}",
            e
        ));
    }

    // 2. Resolve server name (handle sanitized names)
    let server_name = match hub
        .find_server_name_by_sanitized_name(&params.server_name)
        .await
    {
        Some(name) => name,
        None => params.server_name.clone(),
    };

    // 3. Check that the server is connected
    let servers = hub.get_all_servers().await;
    let server = match servers.iter().find(|s| s.name == server_name) {
        Some(s) => s,
        None => {
            return McpResourceExecutionResult::error(format!(
                "Server '{}' not found. Available servers: [{}]",
                server_name,
                servers.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
    };

    // Check connection status
    use roo_types::mcp::McpConnectionStatus;
    if server.status != McpConnectionStatus::Connected {
        return McpResourceExecutionResult::error(format!(
            "Server '{}' is not connected (status: {:?})",
            server_name, server.status
        ));
    }

    // 4. Read the resource via hub
    tracing::info!(
        "Reading MCP resource '{}' on server '{}'",
        params.uri,
        server_name
    );

    match hub.read_resource(&server_name, &params.uri).await {
        Ok(response) => format_resource_response(&server_name, &params.uri, response),
        Err(e) => {
            tracing::error!(
                "MCP resource read '{}' on '{}' failed: {}",
                params.uri,
                server_name,
                e
            );
            McpResourceExecutionResult::error(format!(
                "Resource read '{}' on '{}' failed: {}",
                params.uri, server_name, e
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Response formatting
// ---------------------------------------------------------------------------

/// Format an [`McpResourceResponse`] into an [`McpResourceExecutionResult`].
///
/// Concatenates all content items into a single text output.
fn format_resource_response(
    server_name: &str,
    uri: &str,
    response: roo_types::mcp::McpResourceResponse,
) -> McpResourceExecutionResult {
    if response.contents.is_empty() {
        return McpResourceExecutionResult::success(format!(
            "Resource '{}' on server '{}' returned no content.",
            uri, server_name
        ));
    }

    let mut parts: Vec<String> = Vec::new();

    for content in &response.contents {
        let mime = content.mime_type.as_deref().unwrap_or("text/plain");

        if mime.starts_with("image/") {
            // For image content, show a placeholder since the text field
            // may contain base64 data
            parts.push(format!(
                "[Image resource: {} ({}, {} bytes)]",
                content.uri,
                mime,
                content.text.len()
            ));
        } else {
            if !parts.is_empty() {
                parts.push(String::new()); // blank line separator
            }
            parts.push(content.text.clone());
        }
    }

    McpResourceExecutionResult::success(parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Validation tests (existing) ----

    #[test]
    fn test_validate_missing_server_name() {
        let params = AccessMcpResourceParams {
            server_name: "".to_string(),
            uri: "file:///test".to_string(),
        };
        assert!(validate_access_mcp_resource_params(&params).is_err());
    }

    #[test]
    fn test_validate_missing_uri() {
        let params = AccessMcpResourceParams {
            server_name: "server".to_string(),
            uri: "".to_string(),
        };
        assert!(validate_access_mcp_resource_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_params() {
        let params = AccessMcpResourceParams {
            server_name: "server".to_string(),
            uri: "file:///test".to_string(),
        };
        assert!(validate_access_mcp_resource_params(&params).is_ok());
    }

    #[test]
    fn test_prepare_resource_access() {
        let params = AccessMcpResourceParams {
            server_name: "my-server".to_string(),
            uri: "resource://test".to_string(),
        };
        let result = prepare_resource_access(&params).unwrap();
        assert_eq!(result.server_name, "my-server");
        assert_eq!(result.uri, "resource://test");
    }

    #[test]
    fn test_process_text_content() {
        let params = AccessMcpResourceParams {
            server_name: "server".to_string(),
            uri: "file:///test.txt".to_string(),
        };
        let result = process_resource_content(&params, "hello world", "text/plain");
        assert_eq!(result.content, "hello world");
    }

    #[test]
    fn test_process_image_content() {
        let params = AccessMcpResourceParams {
            server_name: "server".to_string(),
            uri: "file:///image.png".to_string(),
        };
        let result = process_resource_content(&params, "base64data", "image/png");
        assert!(result.content.contains("Image content"));
    }

    // ---- Execution tests ----

    #[tokio::test]
    async fn test_access_mcp_resource_missing_server_name() {
        let hub = Arc::new(roo_mcp::McpHub::new());
        let params = AccessMcpResourceParams {
            server_name: "".to_string(),
            uri: "file:///test".to_string(),
        };
        let result = access_mcp_resource(&hub, &params).await;
        assert!(result.is_error);
        assert!(result.text.contains("Parameter validation failed"));
    }

    #[tokio::test]
    async fn test_access_mcp_resource_missing_uri() {
        let hub = Arc::new(roo_mcp::McpHub::new());
        let params = AccessMcpResourceParams {
            server_name: "server".to_string(),
            uri: "".to_string(),
        };
        let result = access_mcp_resource(&hub, &params).await;
        assert!(result.is_error);
        assert!(result.text.contains("Parameter validation failed"));
    }

    #[tokio::test]
    async fn test_access_mcp_resource_server_not_found() {
        let hub = Arc::new(roo_mcp::McpHub::new());
        let params = AccessMcpResourceParams {
            server_name: "nonexistent".to_string(),
            uri: "file:///test".to_string(),
        };
        let result = access_mcp_resource(&hub, &params).await;
        assert!(result.is_error);
        assert!(result.text.contains("not found"));
    }

    // ---- Response formatting tests ----

    #[test]
    fn test_format_resource_response_text() {
        let response = roo_types::mcp::McpResourceResponse {
            contents: vec![roo_types::mcp::McpResourceContent {
                uri: "file:///test.txt".to_string(),
                mime_type: Some("text/plain".to_string()),
                text: "Hello, world!".to_string(),
            }],
        };
        let result = format_resource_response("server", "file:///test.txt", response);
        assert!(!result.is_error);
        assert_eq!(result.text, "Hello, world!");
    }

    #[test]
    fn test_format_resource_response_empty() {
        let response = roo_types::mcp::McpResourceResponse {
            contents: vec![],
        };
        let result = format_resource_response("server", "file:///test.txt", response);
        assert!(!result.is_error);
        assert!(result.text.contains("no content"));
    }

    #[test]
    fn test_format_resource_response_image() {
        let response = roo_types::mcp::McpResourceResponse {
            contents: vec![roo_types::mcp::McpResourceContent {
                uri: "file:///image.png".to_string(),
                mime_type: Some("image/png".to_string()),
                text: "base64data".to_string(),
            }],
        };
        let result = format_resource_response("server", "file:///image.png", response);
        assert!(!result.is_error);
        assert!(result.text.contains("[Image resource:"));
    }

    #[test]
    fn test_format_resource_response_multiple() {
        let response = roo_types::mcp::McpResourceResponse {
            contents: vec![
                roo_types::mcp::McpResourceContent {
                    uri: "file:///a.txt".to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: "Content A".to_string(),
                },
                roo_types::mcp::McpResourceContent {
                    uri: "file:///b.txt".to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: "Content B".to_string(),
                },
            ],
        };
        let result = format_resource_response("server", "file:///test", response);
        assert!(!result.is_error);
        assert!(result.text.contains("Content A"));
        assert!(result.text.contains("Content B"));
    }
}
