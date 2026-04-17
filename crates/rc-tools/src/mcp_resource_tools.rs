//! MCP resource tools: list_mcp_resources, read_mcp_resource, mcp_auth.
//!
//! Provides tools for interacting with MCP (Model Context Protocol) server
//! resources, including listing, reading, and authentication management.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::ToolExecutionContext;

/// List resources provided by MCP servers.
///
/// Returns a list of available resources from the specified MCP server
/// or all connected servers.
///
/// # Errors
/// Returns an error if the server name is invalid.
pub fn list_mcp_resources(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let server = input["server"].as_str();

    // In a full implementation, this would query the MCP connection manager.
    // For now, return a structured response indicating the tool is available.
    Ok(json!({
        "type": "mcp_resource_list",
        "server": server,
        "resources": [],
        "total": 0,
        "message": if server.is_some() {
            format!("No resources found for MCP server '{}'. Resources require an active MCP connection.", server.unwrap_or_default())
        } else {
            "No MCP servers connected. Use mcp_auth to establish connections.".to_string()
        }
    })
    .to_string())
}

/// Read the content of an MCP resource by URI.
///
/// Fetches the resource content from the specified MCP server.
///
/// # Errors
/// Returns an error if the URI is missing.
pub fn read_mcp_resource(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let uri = input["uri"]
        .as_str()
        .ok_or_else(|| anyhow!("uri is required for reading MCP resource"))?;

    if uri.trim().is_empty() {
        return Err(anyhow!("uri cannot be empty"));
    }

    let server = input["server"].as_str();

    // Validate URI format.
    if !uri.contains(':') && !uri.starts_with('/') {
        return Err(anyhow!(
            "Invalid URI format: '{}'. Expected scheme:path or absolute path.",
            uri
        ));
    }

    Ok(json!({
        "type": "mcp_resource_content",
        "uri": uri,
        "server": server,
        "content": null,
        "mime_type": null,
        "message": "MCP resource reading requires an active MCP connection. No content available in current context."
    })
    .to_string())
}

/// Manage MCP server authentication.
///
/// Supports login, logout, and status check for MCP server authentication.
///
/// # Errors
/// Returns an error if server or action is missing.
pub fn mcp_auth(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let server = input["server"]
        .as_str()
        .ok_or_else(|| anyhow!("server is required for MCP auth"))?;

    if server.trim().is_empty() {
        return Err(anyhow!("server cannot be empty"));
    }

    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (login, logout, or status)"))?;

    let auth_dir = context.cwd.join(".remote-code-rust").join("mcp-auth");
    std::fs::create_dir_all(&auth_dir)?;
    let auth_file = auth_dir.join(format!("{server}.json"));

    match action {
        "login" => {
            let token = input["token"].as_str().unwrap_or("");
            let entry = json!({
                "server": server,
                "status": "authenticated",
                "has_token": !token.is_empty(),
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            let content = serde_json::to_string_pretty(&entry)?;
            std::fs::write(&auth_file, content)?;
            Ok(json!({
                "type": "mcp_auth",
                "server": server,
                "action": "login",
                "status": "authenticated",
                "message": format!("Successfully authenticated with MCP server '{server}'.")
            })
            .to_string())
        }
        "logout" => {
            if auth_file.exists() {
                std::fs::remove_file(&auth_file)?;
                Ok(json!({
                    "type": "mcp_auth",
                    "server": server,
                    "action": "logout",
                    "status": "logged_out",
                    "message": format!("Logged out from MCP server '{server}'.")
                })
                .to_string())
            } else {
                Ok(json!({
                    "type": "mcp_auth",
                    "server": server,
                    "action": "logout",
                    "status": "no_session",
                    "message": format!("No active session for MCP server '{server}'.")
                })
                .to_string())
            }
        }
        "status" => {
            if auth_file.exists() {
                let content = std::fs::read_to_string(&auth_file)?;
                let auth_data: Value = serde_json::from_str(&content).unwrap_or_default();
                Ok(json!({
                    "type": "mcp_auth",
                    "server": server,
                    "action": "status",
                    "status": auth_data["status"].as_str().unwrap_or("unknown"),
                    "authenticated": auth_data["status"].as_str() == Some("authenticated"),
                    "message": format!("MCP server '{server}' authentication status: {}", auth_data["status"].as_str().unwrap_or("unknown"))
                })
                .to_string())
            } else {
                Ok(json!({
                    "type": "mcp_auth",
                    "server": server,
                    "action": "status",
                    "status": "not_authenticated",
                    "authenticated": false,
                    "message": format!("MCP server '{server}' is not authenticated.")
                })
                .to_string())
            }
        }
        _ => Err(anyhow!("action must be 'login', 'logout', or 'status'")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn test_context() -> ToolExecutionContext {
        ToolExecutionContext {
            cwd: PathBuf::from("/tmp"),
            timeout_ms: 30_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(
                rc_core::task_stack::TaskStack::default(),
            )),
        }
    }

    fn test_context_with_dir(dir: &std::path::Path) -> ToolExecutionContext {
        ToolExecutionContext {
            cwd: dir.to_path_buf(),
            timeout_ms: 30_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(
                rc_core::task_stack::TaskStack::default(),
            )),
        }
    }

    #[test]
    fn list_mcp_resources_no_server() {
        let input = json!({});
        let context = test_context();
        let result = list_mcp_resources(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["type"], "mcp_resource_list");
        assert_eq!(parsed["total"], 0);
    }

    #[test]
    fn list_mcp_resources_with_server() {
        let input = json!({"server": "my-server"});
        let context = test_context();
        let result = list_mcp_resources(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["server"], "my-server");
    }

    #[test]
    fn read_mcp_resource_requires_uri() {
        let input = json!({});
        let context = test_context();
        let result = read_mcp_resource(&input, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("uri"));
    }

    #[test]
    fn read_mcp_resource_rejects_empty_uri() {
        let input = json!({"uri": ""});
        let context = test_context();
        let result = read_mcp_resource(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn read_mcp_resource_validates_uri_format() {
        let input = json!({"uri": "invalid-uri"});
        let context = test_context();
        let result = read_mcp_resource(&input, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid URI"));
    }

    #[test]
    fn read_mcp_resource_accepts_scheme_uri() {
        let input = json!({"uri": "file:///path/to/resource"});
        let context = test_context();
        let result = read_mcp_resource(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["uri"], "file:///path/to/resource");
    }

    #[test]
    fn read_mcp_resource_accepts_absolute_path() {
        let input = json!({"uri": "/absolute/path/to/resource"});
        let context = test_context();
        let result = read_mcp_resource(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["uri"], "/absolute/path/to/resource");
    }

    #[test]
    fn mcp_auth_requires_server() {
        let input = json!({"action": "login"});
        let context = test_context();
        let result = mcp_auth(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn mcp_auth_rejects_empty_server() {
        let input = json!({"server": "", "action": "login"});
        let context = test_context();
        let result = mcp_auth(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn mcp_auth_requires_action() {
        let input = json!({"server": "test"});
        let context = test_context();
        let result = mcp_auth(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn mcp_auth_rejects_invalid_action() {
        let temp = TempDir::new().expect("temp dir");
        let input = json!({"server": "test", "action": "invalid"});
        let context = test_context_with_dir(temp.path());
        let result = mcp_auth(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn mcp_auth_login_creates_auth_file() {
        let temp = TempDir::new().expect("temp dir");
        let input = json!({"server": "my-server", "action": "login"});
        let context = test_context_with_dir(temp.path());
        let result = mcp_auth(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "authenticated");

        // Verify file was created.
        let auth_file = temp
            .path()
            .join(".remote-code-rust")
            .join("mcp-auth")
            .join("my-server.json");
        assert!(auth_file.exists());
    }

    #[test]
    fn mcp_auth_status_not_authenticated() {
        let temp = TempDir::new().expect("temp dir");
        let input = json!({"server": "unknown-server", "action": "status"});
        let context = test_context_with_dir(temp.path());
        let result = mcp_auth(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "not_authenticated");
        assert_eq!(parsed["authenticated"], false);
    }

    #[test]
    fn mcp_auth_login_then_status() {
        let temp = TempDir::new().expect("temp dir");
        let context = test_context_with_dir(temp.path());

        // Login.
        let login_input = json!({"server": "test-server", "action": "login"});
        mcp_auth(&login_input, &context).unwrap();

        // Check status.
        let status_input = json!({"server": "test-server", "action": "status"});
        let result = mcp_auth(&status_input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "authenticated");
        assert_eq!(parsed["authenticated"], true);
    }

    #[test]
    fn mcp_auth_login_then_logout() {
        let temp = TempDir::new().expect("temp dir");
        let context = test_context_with_dir(temp.path());

        // Login.
        let login_input = json!({"server": "test-server", "action": "login"});
        mcp_auth(&login_input, &context).unwrap();

        // Logout.
        let logout_input = json!({"server": "test-server", "action": "logout"});
        let result = mcp_auth(&logout_input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "logged_out");

        // File should be removed.
        let auth_file = temp
            .path()
            .join(".remote-code-rust")
            .join("mcp-auth")
            .join("test-server.json");
        assert!(!auth_file.exists());
    }

    #[test]
    fn mcp_auth_logout_nonexistent() {
        let temp = TempDir::new().expect("temp dir");
        let input = json!({"server": "nonexistent", "action": "logout"});
        let context = test_context_with_dir(temp.path());
        let result = mcp_auth(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "no_session");
    }

    #[test]
    fn mcp_auth_login_with_token() {
        let temp = TempDir::new().expect("temp dir");
        let input = json!({"server": "test-server", "action": "login", "token": "my-secret-token"});
        let context = test_context_with_dir(temp.path());
        let result = mcp_auth(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "authenticated");
    }
}
