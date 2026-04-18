//! Stdio MCP session management.
//!
//! Handles spawning MCP server processes, performing the initialization
//! handshake, listing tools, and invoking tools over stdio JSON-RPC.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use walkdir::WalkDir;

use crate::config::{McpConfig, McpServerConfig};
use crate::error::{McpConfigError, McpRuntimeError};
use crate::jsonrpc::{
    InitializeParams, JsonRpcEnvelope, JsonRpcNotification, JsonRpcRequest, McpInitializeResult,
    McpResourceContent, McpResourceReadResult, McpResourcesListResult, McpToolsListResult,
    ResourceReadParams, ToolCallParams, rpc_id_matches,
};
use crate::resources::ServerResource;
use crate::types::{McpClientInfo, McpServerInspection, McpToolCallResponse, McpToolCallResult};

/// Default MCP protocol version used during initialisation.
pub const DEFAULT_MCP_PROTOCOL_VERSION: &str = "2025-03-26";
/// Default timeout for MCP server startup in seconds.
pub const DEFAULT_STARTUP_TIMEOUT_SECS: u64 = 10;
/// Default timeout for individual MCP requests in seconds.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 15;
/// Default legacy MCP config file name.
pub const DEFAULT_MCP_CONFIG_FILE: &str = "mcp.toml";
/// Default Claude-compatible project MCP config file name.
pub const DEFAULT_PROJECT_MCP_CONFIG_FILE: &str = ".mcp.json";

/// An active stdio MCP session managing a child process.
pub(crate) struct StdioMcpSession {
    server_name: String,
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    initialized: McpInitializeResult,
    request_timeout_secs: u64,
}

/// Inspect an MCP server: initialize, list tools, and return the inspection result.
pub async fn inspect_server(
    server: &McpServerConfig,
    client_info: &McpClientInfo,
) -> Result<McpServerInspection, McpRuntimeError> {
    match &server.transport {
        crate::transport::McpTransportConfig::Stdio {
            command,
            args,
            cwd,
            env,
        } => inspect_stdio_server(server, command, args, cwd.as_deref(), env, client_info).await,
        _ => Err(McpRuntimeError::UnsupportedTransport {
            server: server.name.clone(),
            transport: server.transport.kind(),
        }),
    }
}

/// Call a tool on an MCP server.
pub async fn call_tool(
    server: &McpServerConfig,
    client_info: &McpClientInfo,
    tool_name: &str,
    arguments: Value,
) -> Result<McpToolCallResponse, McpRuntimeError> {
    match &server.transport {
        crate::transport::McpTransportConfig::Stdio {
            command,
            args,
            cwd,
            env,
        } => {
            let mut session =
                StdioMcpSession::connect(server, command, args, cwd.as_deref(), env, client_info)
                    .await?;
            let result = session.call_tool(tool_name, arguments).await;
            session.shutdown().await;
            result
        }
        _ => Err(McpRuntimeError::UnsupportedTransport {
            server: server.name.clone(),
            transport: server.transport.kind(),
        }),
    }
}

/// List resources exposed by an MCP server.
///
/// Connects to the server via stdio, sends `resources/list`, and returns
/// the available resources.
pub async fn list_resources(
    server: &McpServerConfig,
    client_info: &McpClientInfo,
) -> Result<Vec<ServerResource>, McpRuntimeError> {
    match &server.transport {
        crate::transport::McpTransportConfig::Stdio {
            command,
            args,
            cwd,
            env,
        } => {
            let mut session =
                StdioMcpSession::connect(server, command, args, cwd.as_deref(), env, client_info)
                    .await?;
            let result = session.list_resources().await;
            session.shutdown().await;
            result
        }
        _ => Err(McpRuntimeError::UnsupportedTransport {
            server: server.name.clone(),
            transport: server.transport.kind(),
        }),
    }
}

/// Read a resource from an MCP server.
///
/// Connects to the server via stdio, sends `resources/read`, and returns
/// the resource content.
pub async fn read_resource(
    server: &McpServerConfig,
    client_info: &McpClientInfo,
    uri: &str,
) -> Result<Vec<McpResourceContent>, McpRuntimeError> {
    match &server.transport {
        crate::transport::McpTransportConfig::Stdio {
            command,
            args,
            cwd,
            env,
        } => {
            let mut session =
                StdioMcpSession::connect(server, command, args, cwd.as_deref(), env, client_info)
                    .await?;
            let result = session.read_resource(uri).await;
            session.shutdown().await;
            result
        }
        _ => Err(McpRuntimeError::UnsupportedTransport {
            server: server.name.clone(),
            transport: server.transport.kind(),
        }),
    }
}

/// Discover MCP configuration files under a root directory.
pub fn discover_mcp_configs(root: &Path) -> Vec<PathBuf> {
    let mut configs = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry.file_name() == DEFAULT_MCP_CONFIG_FILE
                || entry.file_name() == DEFAULT_PROJECT_MCP_CONFIG_FILE
        })
        .map(walkdir::DirEntry::into_path)
        .collect::<Vec<_>>();
    configs.sort();
    configs
}

/// Discover and load all MCP configuration files under a root directory.
pub fn load_discovered_mcp_configs(
    root: &Path,
) -> Result<Vec<crate::config::DiscoveredMcpConfig>, McpConfigError> {
    discover_mcp_configs(root)
        .into_iter()
        .map(|path| {
            let config = McpConfig::load(&path)?;
            Ok(crate::config::DiscoveredMcpConfig { path, config })
        })
        .collect()
}

async fn inspect_stdio_server(
    server: &McpServerConfig,
    command: &str,
    args: &[String],
    cwd: Option<&Path>,
    env: &BTreeMap<String, String>,
    client_info: &McpClientInfo,
) -> Result<McpServerInspection, McpRuntimeError> {
    let mut session =
        StdioMcpSession::connect(server, command, args, cwd, env, client_info).await?;
    let result = session.inspect_server().await;
    session.shutdown().await;
    result
}

fn resolve_stdio_command(command: &str) -> String {
    #[cfg(windows)]
    {
        let path = Path::new(command);
        if path.extension().is_some() || path.components().count() > 1 {
            return command.to_owned();
        }

        for candidate in [
            format!("{command}.exe"),
            format!("{command}.cmd"),
            format!("{command}.bat"),
        ] {
            if let Ok(output) = std::process::Command::new("where.exe")
                .arg(&candidate)
                .output()
                && output.status.success()
                && let Some(first_match) = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
            {
                return first_match.to_owned();
            }
        }
    }

    command.to_owned()
}

impl StdioMcpSession {
    async fn connect(
        server: &McpServerConfig,
        command: &str,
        args: &[String],
        cwd: Option<&Path>,
        env: &BTreeMap<String, String>,
        client_info: &McpClientInfo,
    ) -> Result<Self, McpRuntimeError> {
        let resolved_command = resolve_stdio_command(command);
        let mut process = Command::new(&resolved_command);
        process
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(cwd) = cwd {
            process.current_dir(cwd);
        }
        if !env.is_empty() {
            process.envs(env);
        }

        let mut child = process.spawn().map_err(|source| McpRuntimeError::Spawn {
            server: server.name.clone(),
            command: resolved_command.clone(),
            source,
        })?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpRuntimeError::MissingPipe {
                server: server.name.clone(),
                pipe: "stdin",
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpRuntimeError::MissingPipe {
                server: server.name.clone(),
                pipe: "stdout",
            })?;
        let mut lines = BufReader::new(stdout).lines();
        let startup_timeout = server
            .startup_timeout_secs
            .unwrap_or(DEFAULT_STARTUP_TIMEOUT_SECS);
        let request_timeout_secs = server
            .request_timeout_secs
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

        let initialize = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "initialize",
            params: InitializeParams {
                protocol_version: DEFAULT_MCP_PROTOCOL_VERSION,
                capabilities: serde_json::json!({}),
                client_info,
            },
        };
        write_message(&mut stdin, &server.name, "initialize request", &initialize).await?;
        let initialized: McpInitializeResult = wait_for_response(
            &mut lines,
            &server.name,
            1,
            "initialize response",
            startup_timeout,
        )
        .await?;

        let ready = JsonRpcNotification {
            jsonrpc: "2.0",
            method: "notifications/initialized",
            params: serde_json::json!({}),
        };
        write_message(&mut stdin, &server.name, "initialized notification", &ready).await?;

        Ok(Self {
            server_name: server.name.clone(),
            child,
            stdin,
            lines,
            initialized,
            request_timeout_secs,
        })
    }

    async fn inspect_server(&mut self) -> Result<McpServerInspection, McpRuntimeError> {
        let tools_request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "tools/list",
            params: serde_json::json!({}),
        };
        write_message(
            &mut self.stdin,
            &self.server_name,
            "tools/list request",
            &tools_request,
        )
        .await?;
        let tools: McpToolsListResult = wait_for_response(
            &mut self.lines,
            &self.server_name,
            2,
            "tools/list response",
            self.request_timeout_secs,
        )
        .await?;

        Ok(McpServerInspection {
            server_name: self.server_name.clone(),
            protocol_version: self.initialized.protocol_version.clone(),
            server_info: self.initialized.server_info.clone(),
            capabilities: self.initialized.capabilities.clone(),
            instructions: self.initialized.instructions.clone(),
            tools: tools.tools,
        })
    }

    async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolCallResponse, McpRuntimeError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "tools/call",
            params: ToolCallParams {
                name: tool_name,
                arguments,
            },
        };
        write_message(
            &mut self.stdin,
            &self.server_name,
            "tools/call request",
            &request,
        )
        .await?;
        let result: McpToolCallResult = wait_for_response(
            &mut self.lines,
            &self.server_name,
            2,
            "tools/call response",
            self.request_timeout_secs,
        )
        .await?;

        Ok(McpToolCallResponse {
            server_name: self.server_name.clone(),
            tool_name: tool_name.to_owned(),
            protocol_version: self.initialized.protocol_version.clone(),
            server_info: self.initialized.server_info.clone(),
            result,
        })
    }

    /// List resources exposed by this MCP server.
    async fn list_resources(&mut self) -> Result<Vec<ServerResource>, McpRuntimeError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 3,
            method: "resources/list",
            params: serde_json::json!({}),
        };
        write_message(
            &mut self.stdin,
            &self.server_name,
            "resources/list request",
            &request,
        )
        .await?;
        let result: McpResourcesListResult = wait_for_response(
            &mut self.lines,
            &self.server_name,
            3,
            "resources/list response",
            self.request_timeout_secs,
        )
        .await?;

        let resources = result
            .resources
            .into_iter()
            .map(|r| {
                let mut sr = ServerResource::new(r.uri, &self.server_name);
                sr.name = r.name;
                sr.description = r.description;
                sr.mime_type = r.mime_type;
                sr
            })
            .collect();
        Ok(resources)
    }

    /// Read a resource from this MCP server.
    async fn read_resource(
        &mut self,
        uri: &str,
    ) -> Result<Vec<McpResourceContent>, McpRuntimeError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 4,
            method: "resources/read",
            params: ResourceReadParams {
                uri: uri.to_owned(),
            },
        };
        write_message(
            &mut self.stdin,
            &self.server_name,
            "resources/read request",
            &request,
        )
        .await?;
        let result: McpResourceReadResult = wait_for_response(
            &mut self.lines,
            &self.server_name,
            4,
            "resources/read response",
            self.request_timeout_secs,
        )
        .await?;
        Ok(result.contents)
    }

    async fn shutdown(&mut self) {
        shutdown_child(&mut self.child).await;
    }
}

async fn write_message<T: Serialize>(
    stdin: &mut ChildStdin,
    server: &str,
    phase: &'static str,
    payload: &T,
) -> Result<(), McpRuntimeError> {
    let mut body = serde_json::to_vec(payload).map_err(|source| McpRuntimeError::Serialize {
        server: server.to_owned(),
        phase,
        source,
    })?;
    body.push(b'\n');
    stdin
        .write_all(&body)
        .await
        .map_err(|source| McpRuntimeError::Write {
            server: server.to_owned(),
            phase,
            source,
        })?;
    stdin
        .flush()
        .await
        .map_err(|source| McpRuntimeError::Write {
            server: server.to_owned(),
            phase,
            source,
        })
}

async fn wait_for_response<T: DeserializeOwned>(
    lines: &mut Lines<BufReader<ChildStdout>>,
    server: &str,
    request_id: u64,
    phase: &'static str,
    timeout_secs: u64,
) -> Result<T, McpRuntimeError> {
    timeout(Duration::from_secs(timeout_secs), async {
        loop {
            let line = lines
                .next_line()
                .await
                .map_err(|source| McpRuntimeError::Read {
                    server: server.to_owned(),
                    phase,
                    source,
                })?;
            let Some(line) = line else {
                return Err(McpRuntimeError::Closed {
                    server: server.to_owned(),
                    phase,
                });
            };
            if line.trim().is_empty() {
                continue;
            }
            let envelope: JsonRpcEnvelope =
                serde_json::from_str(&line).map_err(|source| McpRuntimeError::Decode {
                    server: server.to_owned(),
                    phase,
                    source,
                })?;
            let Some(id) = envelope.id.as_ref() else {
                continue;
            };
            if !rpc_id_matches(id, request_id) {
                continue;
            }
            if let Some(error) = envelope.error {
                return Err(McpRuntimeError::Rpc {
                    server: server.to_owned(),
                    code: error.code,
                    message: error.message,
                });
            }
            let result = envelope.result.ok_or_else(|| McpRuntimeError::Protocol {
                server: server.to_owned(),
                phase,
                message: "response did not include a result payload".to_owned(),
            })?;
            return serde_json::from_value(result).map_err(|source| McpRuntimeError::Decode {
                server: server.to_owned(),
                phase,
                source,
            });
        }
    })
    .await
    .map_err(|_| McpRuntimeError::Timeout {
        server: server.to_owned(),
        phase,
        timeout_secs,
    })?
}

async fn shutdown_child(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use super::resolve_stdio_command;

    #[test]
    fn resolve_stdio_command_preserves_explicit_extension() {
        assert_eq!(resolve_stdio_command("python.exe"), "python.exe");
    }

    #[test]
    fn resolve_stdio_command_preserves_relative_paths() {
        assert_eq!(
            resolve_stdio_command(".\\scripts\\server.cmd"),
            ".\\scripts\\server.cmd"
        );
    }

    #[cfg(windows)]
    #[test]
    fn resolve_stdio_command_prefers_windows_wrappers_when_available() {
        let resolved = resolve_stdio_command("npx");
        assert!(
            resolved.eq_ignore_ascii_case("npx")
                || resolved.to_ascii_lowercase().ends_with("npx.cmd")
                || resolved.to_ascii_lowercase().ends_with("npx.exe"),
            "unexpected resolved command: {resolved}"
        );
    }
}
