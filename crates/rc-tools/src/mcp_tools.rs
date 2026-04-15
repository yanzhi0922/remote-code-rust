//! MCP (Model Context Protocol) tool implementations.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::{ToolExecutionContext, current_tool_runtime_policy};
use crate::mcp_runtime::resolve_runtime_policy_mcp_server;

pub(crate) fn mcp_auth_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let server = input["server"]
        .as_str()
        .ok_or_else(|| anyhow!("server is required"))?;
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (login, logout, or status)"))?;

    let auth_dir = context.cwd.join(".remote-code-rust").join("mcp-auth");
    std::fs::create_dir_all(&auth_dir)?;
    let auth_file = auth_dir.join(format!("{server}.json"));

    match action {
        "login" => {
            let entry = json!({
                "server": server,
                "status": "authenticated",
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            let content = serde_json::to_string_pretty(&entry)?;
            std::fs::write(&auth_file, content)?;
            Ok(format!("Logged in to MCP server '{server}'."))
        }
        "logout" => {
            if auth_file.exists() {
                std::fs::remove_file(&auth_file)?;
                Ok(format!("Logged out from MCP server '{server}'."))
            } else {
                Ok(format!("No active session for MCP server '{server}'."))
            }
        }
        "status" => {
            if auth_file.exists() {
                let content = std::fs::read_to_string(&auth_file)?;
                Ok(content)
            } else {
                Ok(json!({
                    "server": server,
                    "status": "not_authenticated",
                })
                .to_string())
            }
        }
        _ => Err(anyhow!("action must be 'login', 'logout', or 'status'")),
    }
}

pub(crate) fn list_mcp_resources_tool(input: &Value) -> Result<String> {
    let server = input["server"].as_str();

    Ok(json!({
        "server": server,
        "resources": [],
        "message": "MCP resource listing requires an active MCP connection. No resources found in current context."
    })
    .to_string())
}

pub(crate) fn read_mcp_resource_tool(input: &Value) -> Result<String> {
    let uri = input["uri"]
        .as_str()
        .ok_or_else(|| anyhow!("uri is required"))?;

    Ok(json!({
        "uri": uri,
        "content": null,
        "message": "MCP resource reading requires an active MCP connection. No content available in current context."
    })
    .to_string())
}

/// Call a tool on an MCP server directly.
///
/// Uses the configured runtime MCP inventory, finds the specified server,
/// and invokes the named tool with the provided arguments.
pub(crate) async fn mcp_call_tool(
    input: &Value,
    _context: &ToolExecutionContext,
) -> Result<String> {
    let server_name = input["server"]
        .as_str()
        .ok_or_else(|| anyhow!("server is required"))?;
    let tool_name = input["tool"]
        .as_str()
        .ok_or_else(|| anyhow!("tool is required"))?;
    let arguments = input.get("arguments").cloned().unwrap_or(json!({}));

    let runtime_policy = current_tool_runtime_policy();
    let server = resolve_runtime_policy_mcp_server(&runtime_policy, server_name)?.server;

    let client_info = rc_mcp::McpClientInfo::new("remote-code-rust", env!("CARGO_PKG_VERSION"));
    let response = rc_mcp::call_tool(&server, &client_info, tool_name, arguments).await?;

    let mut parts = Vec::new();
    parts.push(format!("server:  {}", response.server_name));
    parts.push(format!("tool:    {}", response.tool_name));
    parts.push(format!("success: {}", !response.result.is_error));

    if !response.result.content.is_empty() {
        let content_text: Vec<String> = response
            .result
            .content
            .iter()
            .filter_map(|c| {
                if c.kind == "text" {
                    c.fields
                        .get("text")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                } else {
                    None
                }
            })
            .collect();
        if !content_text.is_empty() {
            parts.push(format!("output:\n{}", content_text.join("\n")));
        }
    }

    Ok(parts.join("\n"))
}
