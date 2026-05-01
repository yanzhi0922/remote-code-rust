//! use_mcp_tool tool implementation.
//!
//! Provides both parameter validation functions and the full MCP tool
//! execution via [`McpHub`].

use std::sync::Arc;

use crate::helpers::*;
use crate::types::*;
use roo_types::mcp::McpToolResultContent;
use roo_types::tool::UseMcpToolParams;

// ---------------------------------------------------------------------------
// Parameter validation (existing, unchanged)
// ---------------------------------------------------------------------------

/// Validate use_mcp_tool parameters.
pub fn validate_use_mcp_tool_params(
    params: &UseMcpToolParams,
) -> Result<McpToolValidation, McpToolError> {
    if params.server_name.trim().is_empty() {
        return Err(McpToolError::MissingParameter("server_name".to_string()));
    }

    if params.tool_name.trim().is_empty() {
        return Err(McpToolError::MissingParameter("tool_name".to_string()));
    }

    validate_mcp_arguments(&params.arguments)?;

    Ok(McpToolValidation {
        server_name: params.server_name.clone(),
        tool_name: params.tool_name.clone(),
        is_valid: true,
        error: None,
    })
}

/// Check if a tool exists in the available tools list.
pub fn find_tool<'a>(available_tools: &'a [String], tool_name: &str) -> Option<&'a String> {
    available_tools
        .iter()
        .find(|t| tool_names_match(t, tool_name))
}

/// Prepare an MCP tool call.
pub fn prepare_mcp_tool_call(
    params: &UseMcpToolParams,
    available_tools: &[String],
) -> Result<McpToolResult, McpToolError> {
    let validation = validate_use_mcp_tool_params(params)?;

    // Check if tool exists
    if !available_tools.is_empty() {
        let found = find_tool(available_tools, &params.tool_name);
        if found.is_none() {
            return Err(McpToolError::ToolNotFound(format!(
                "Tool '{}' not found on server '{}'. Available: [{}]",
                params.tool_name,
                params.server_name,
                available_tools.join(", ")
            )));
        }
    }

    Ok(McpToolResult {
        server_name: validation.server_name,
        tool_name: validation.tool_name,
        result: params.arguments.clone(),
        is_error: false,
    })
}

// ---------------------------------------------------------------------------
// Full MCP tool execution via McpHub
// ---------------------------------------------------------------------------

/// Execute an MCP tool call via [`roo_mcp::McpHub`].
///
/// This is the primary entry point for the `use_mcp_tool` tool handler.
/// It validates parameters, resolves the server name, calls the tool via
/// the hub, and formats the response.
///
/// # Arguments
/// * `hub` — Shared reference to the MCP hub.
/// * `params` — The tool call parameters (server_name, tool_name, arguments).
///
/// # Returns
/// An [`McpToolExecutionResult`] containing the formatted text output,
/// any images returned by the tool, and an error flag.
pub async fn execute_mcp_tool(
    hub: &Arc<roo_mcp::McpHub>,
    params: &UseMcpToolParams,
) -> McpToolExecutionResult {
    // 1. Validate parameters
    if let Err(e) = validate_use_mcp_tool_params(params) {
        return McpToolExecutionResult::error(format!("Parameter validation failed: {}", e));
    }

    // 2. Resolve server name (handle sanitized names)
    let server_name = match hub
        .find_server_name_by_sanitized_name(&params.server_name)
        .await
    {
        Some(name) => name,
        None => params.server_name.clone(),
    };

    // 3. Check that the server is connected and the tool exists
    let servers = hub.get_all_servers().await;
    let server = match servers.iter().find(|s| s.name == server_name) {
        Some(s) => s,
        None => {
            return McpToolExecutionResult::error(format!(
                "Server '{}' not found. Available servers: [{}]",
                server_name,
                servers.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
    };

    // Check connection status
    use roo_types::mcp::McpConnectionStatus;
    if server.status != McpConnectionStatus::Connected {
        return McpToolExecutionResult::error(format!(
            "Server '{}' is not connected (current status: {:?}). \
             Possible causes: \
             (1) The MCP server process crashed or was stopped. \
             (2) The server configuration is incorrect. \
             (3) The server is still starting up. \
             Suggestions: \
             - Check the server logs for errors. \
             - Verify the server command and arguments in your MCP configuration. \
             - Try restarting the server.",
            server_name, server.status
        ));
    }

    // Check tool exists (normalize comparison)
    let tool_exists = server
        .tools
        .iter()
        .any(|t| tool_names_match(&t.name, &params.tool_name));
    if !tool_exists {
        let available: Vec<&str> = server.tools.iter().map(|t| t.name.as_str()).collect();
        return McpToolExecutionResult::error(format!(
            "Tool '{}' not found on server '{}'. Available tools: [{}]",
            params.tool_name,
            server_name,
            available.join(", ")
        ));
    }

    // 4. Call the tool via hub
    tracing::info!(
        "Calling MCP tool '{}' on server '{}'",
        params.tool_name,
        server_name
    );

    let arguments = if params.arguments.is_null() {
        None
    } else {
        Some(params.arguments.clone())
    };

    match hub
        .call_tool(&server_name, &params.tool_name, arguments)
        .await
    {
        Ok(response) => format_tool_call_response(&server_name, &params.tool_name, response),
        Err(e) => {
            tracing::error!(
                "MCP tool call '{}' on '{}' failed: {}",
                params.tool_name,
                server_name,
                e
            );
            McpToolExecutionResult::error(format!(
                "Tool call '{}' on server '{}' failed: {}. \
                 This may indicate a problem with the MCP server or the tool implementation. \
                 Suggestions: \
                 - Check that the tool arguments are correct. \
                 - Verify the MCP server is running and responsive. \
                 - Review the server logs for detailed error information.",
                params.tool_name, server_name, e
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Response formatting
// ---------------------------------------------------------------------------

/// Format an [`McpToolCallResponse`] into an [`McpToolExecutionResult`].
///
/// Extracts text content and images from the response, handling each
/// content type appropriately:
/// - Text → appended to output
/// - Image → collected as base64 data
/// - Resource → formatted as text reference
fn format_tool_call_response(
    server_name: &str,
    tool_name: &str,
    response: roo_types::mcp::McpToolCallResponse,
) -> McpToolExecutionResult {
    let is_error = response.is_error.unwrap_or(false);
    let mut text_parts: Vec<String> = Vec::new();
    let mut images: Vec<String> = Vec::new();

    for content in &response.content {
        match content {
            McpToolResultContent::Text { text } => {
                text_parts.push(text.clone());
            }
            McpToolResultContent::Image { data, mime_type } => {
                // Store the base64 data URI for images
                let data_uri = format!("data:{};base64,{}", mime_type, data);
                images.push(data_uri);
                text_parts.push(format!("[Image: {} ({} bytes)]", mime_type, data.len()));
            }
            McpToolResultContent::Resource { resource } => {
                text_parts.push(format!(
                    "[Resource: {} ({})]\n{}",
                    resource.uri,
                    resource.mime_type.as_deref().unwrap_or("unknown"),
                    resource.text
                ));
            }
        }
    }

    let output = if text_parts.is_empty() {
        format!(
            "Tool '{}' on '{}' returned no content.",
            tool_name, server_name
        )
    } else {
        text_parts.join("\n")
    };

    if is_error {
        McpToolExecutionResult::error(output)
    } else if images.is_empty() {
        McpToolExecutionResult::success(output)
    } else {
        McpToolExecutionResult::success_with_images(output, images)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- Validation tests (existing) ----

    #[test]
    fn test_validate_missing_server_name() {
        let params = UseMcpToolParams {
            server_name: "".to_string(),
            tool_name: "tool".to_string(),
            arguments: json!({}),
        };
        assert!(validate_use_mcp_tool_params(&params).is_err());
    }

    #[test]
    fn test_validate_missing_tool_name() {
        let params = UseMcpToolParams {
            server_name: "server".to_string(),
            tool_name: "".to_string(),
            arguments: json!({}),
        };
        assert!(validate_use_mcp_tool_params(&params).is_err());
    }

    #[test]
    fn test_validate_invalid_arguments() {
        let params = UseMcpToolParams {
            server_name: "server".to_string(),
            tool_name: "tool".to_string(),
            arguments: json!("not an object"),
        };
        assert!(validate_use_mcp_tool_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_params() {
        let params = UseMcpToolParams {
            server_name: "server".to_string(),
            tool_name: "tool".to_string(),
            arguments: json!({"key": "value"}),
        };
        let result = validate_use_mcp_tool_params(&params).unwrap();
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_null_arguments() {
        let params = UseMcpToolParams {
            server_name: "server".to_string(),
            tool_name: "tool".to_string(),
            arguments: json!(null),
        };
        assert!(validate_use_mcp_tool_params(&params).is_ok());
    }

    #[test]
    fn test_find_tool_exact() {
        let tools = vec!["tool-a".to_string(), "tool-b".to_string()];
        assert!(find_tool(&tools, "tool-a").is_some());
    }

    #[test]
    fn test_find_tool_normalized() {
        let tools = vec!["my_tool".to_string()];
        assert!(find_tool(&tools, "my-tool").is_some());
    }

    #[test]
    fn test_find_tool_not_found() {
        let tools = vec!["tool-a".to_string()];
        assert!(find_tool(&tools, "tool-b").is_none());
    }

    #[test]
    fn test_prepare_mcp_tool_call_success() {
        let params = UseMcpToolParams {
            server_name: "server".to_string(),
            tool_name: "my-tool".to_string(),
            arguments: json!({"x": 1}),
        };
        let tools = vec!["my_tool".to_string()];
        let result = prepare_mcp_tool_call(&params, &tools).unwrap();
        assert!(!result.is_error);
        assert_eq!(result.tool_name, "my-tool");
    }

    #[test]
    fn test_prepare_mcp_tool_call_not_found() {
        let params = UseMcpToolParams {
            server_name: "server".to_string(),
            tool_name: "missing".to_string(),
            arguments: json!({}),
        };
        let tools = vec!["other".to_string()];
        assert!(prepare_mcp_tool_call(&params, &tools).is_err());
    }

    #[test]
    fn test_prepare_mcp_tool_call_empty_tools() {
        let params = UseMcpToolParams {
            server_name: "server".to_string(),
            tool_name: "any-tool".to_string(),
            arguments: json!({}),
        };
        // Empty tools list means skip tool existence check
        let result = prepare_mcp_tool_call(&params, &[]).unwrap();
        assert!(!result.is_error);
    }

    // ---- Execution tests ----

    #[tokio::test]
    async fn test_execute_mcp_tool_missing_server_name() {
        let hub = Arc::new(roo_mcp::McpHub::new());
        let params = UseMcpToolParams {
            server_name: "".to_string(),
            tool_name: "tool".to_string(),
            arguments: json!({}),
        };
        let result = execute_mcp_tool(&hub, &params).await;
        assert!(result.is_error);
        assert!(result.text.contains("Parameter validation failed"));
    }

    #[tokio::test]
    async fn test_execute_mcp_tool_missing_tool_name() {
        let hub = Arc::new(roo_mcp::McpHub::new());
        let params = UseMcpToolParams {
            server_name: "server".to_string(),
            tool_name: "".to_string(),
            arguments: json!({}),
        };
        let result = execute_mcp_tool(&hub, &params).await;
        assert!(result.is_error);
        assert!(result.text.contains("Parameter validation failed"));
    }

    #[tokio::test]
    async fn test_execute_mcp_tool_server_not_found() {
        let hub = Arc::new(roo_mcp::McpHub::new());
        let params = UseMcpToolParams {
            server_name: "nonexistent".to_string(),
            tool_name: "tool".to_string(),
            arguments: json!({}),
        };
        let result = execute_mcp_tool(&hub, &params).await;
        assert!(result.is_error);
        assert!(result.text.contains("not found"));
    }

    // ---- Response formatting tests ----

    #[test]
    fn test_format_tool_call_response_text_only() {
        let response = roo_types::mcp::McpToolCallResponse {
            content: vec![McpToolResultContent::Text {
                text: "Hello, world!".to_string(),
            }],
            is_error: None,
        };
        let result = format_tool_call_response("server", "tool", response);
        assert!(!result.is_error);
        assert_eq!(result.text, "Hello, world!");
        assert!(result.images.is_empty());
    }

    #[test]
    fn test_format_tool_call_response_error() {
        let response = roo_types::mcp::McpToolCallResponse {
            content: vec![McpToolResultContent::Text {
                text: "Something went wrong".to_string(),
            }],
            is_error: Some(true),
        };
        let result = format_tool_call_response("server", "tool", response);
        assert!(result.is_error);
        assert_eq!(result.text, "Something went wrong");
    }

    #[test]
    fn test_format_tool_call_response_with_image() {
        let response = roo_types::mcp::McpToolCallResponse {
            content: vec![
                McpToolResultContent::Text {
                    text: "Here is an image:".to_string(),
                },
                McpToolResultContent::Image {
                    data: "base64data".to_string(),
                    mime_type: "image/png".to_string(),
                },
            ],
            is_error: None,
        };
        let result = format_tool_call_response("server", "tool", response);
        assert!(!result.is_error);
        assert!(result.text.contains("Here is an image:"));
        assert!(result.text.contains("[Image:"));
        assert_eq!(result.images.len(), 1);
        assert!(result.images[0].starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_format_tool_call_response_with_resource() {
        let response = roo_types::mcp::McpToolCallResponse {
            content: vec![McpToolResultContent::Resource {
                resource: roo_types::mcp::McpResourceContent {
                    uri: "file:///test.txt".to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: "file content".to_string(),
                },
            }],
            is_error: None,
        };
        let result = format_tool_call_response("server", "tool", response);
        assert!(!result.is_error);
        assert!(result.text.contains("file:///test.txt"));
        assert!(result.text.contains("file content"));
    }

    #[test]
    fn test_format_tool_call_response_empty() {
        let response = roo_types::mcp::McpToolCallResponse {
            content: vec![],
            is_error: None,
        };
        let result = format_tool_call_response("server", "tool", response);
        assert!(!result.is_error);
        assert!(result.text.contains("returned no content"));
    }

    #[test]
    fn test_format_tool_call_response_multiple_text() {
        let response = roo_types::mcp::McpToolCallResponse {
            content: vec![
                McpToolResultContent::Text {
                    text: "Line 1".to_string(),
                },
                McpToolResultContent::Text {
                    text: "Line 2".to_string(),
                },
            ],
            is_error: None,
        };
        let result = format_tool_call_response("server", "tool", response);
        assert!(!result.is_error);
        assert!(result.text.contains("Line 1"));
        assert!(result.text.contains("Line 2"));
    }
}
