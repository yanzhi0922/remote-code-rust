use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};
use walkdir::WalkDir;

pub const DEFAULT_MCP_CONFIG_FILE: &str = "mcp.toml";
pub const DEFAULT_MCP_PROTOCOL_VERSION: &str = "2025-03-26";
pub const DEFAULT_STARTUP_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    Stdio,
    Http,
    WebSocket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpTransportConfig {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        cwd: Option<PathBuf>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    WebSocket {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl McpTransportConfig {
    #[must_use]
    pub fn kind(&self) -> McpTransport {
        match self {
            Self::Stdio { .. } => McpTransport::Stdio,
            Self::Http { .. } => McpTransport::Http,
            Self::WebSocket { .. } => McpTransport::WebSocket,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct McpCapabilityMatrix {
    #[serde(default)]
    pub supports_tools: bool,
    #[serde(default)]
    pub supports_prompts: bool,
    #[serde(default)]
    pub supports_resources: bool,
    #[serde(default)]
    pub supports_sampling: bool,
    #[serde(default)]
    pub supports_roots: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub transport: McpTransportConfig,
    #[serde(default)]
    pub capabilities: McpCapabilityMatrix,
    #[serde(default)]
    pub startup_timeout_secs: Option<u64>,
    #[serde(default)]
    pub request_timeout_secs: Option<u64>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct McpConfig {
    pub servers: BTreeMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredMcpConfig {
    pub path: PathBuf,
    pub config: McpConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpClientInfo {
    pub name: String,
    pub version: String,
}

impl McpClientInfo {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpPeerInfo {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDescriptor {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
    #[serde(default)]
    pub annotations: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInspection {
    pub server_name: String,
    pub protocol_version: String,
    #[serde(default)]
    pub server_info: Option<McpPeerInfo>,
    #[serde(default)]
    pub capabilities: Value,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub tools: Vec<McpToolDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolCallContent {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallResult {
    #[serde(default)]
    pub content: Vec<McpToolCallContent>,
    #[serde(default)]
    pub structured_content: Option<Value>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallResponse {
    pub server_name: String,
    pub tool_name: String,
    pub protocol_version: String,
    #[serde(default)]
    pub server_info: Option<McpPeerInfo>,
    pub result: McpToolCallResult,
}

#[derive(Debug, Error)]
pub enum McpConfigError {
    #[error("failed to read MCP config at `{path}`")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse MCP config TOML at `{path}`")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("MCP server `{name}` must define either `command` or `url`")]
    MissingTransport { name: String },
    #[error("MCP server `{name}` cannot define both `command` and `url`")]
    AmbiguousTransport { name: String },
    #[error("MCP server `{name}` uses unsupported url scheme `{scheme}`")]
    UnsupportedUrlScheme { name: String, scheme: String },
}

#[derive(Debug, Error)]
pub enum McpRuntimeError {
    #[error("MCP server `{server}` uses unsupported runtime transport `{transport:?}`")]
    UnsupportedTransport {
        server: String,
        transport: McpTransport,
    },
    #[error("failed to spawn MCP server `{server}` using `{command}`")]
    Spawn {
        server: String,
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("MCP server `{server}` did not expose {pipe}")]
    MissingPipe { server: String, pipe: &'static str },
    #[error("failed to serialize JSON-RPC payload for MCP server `{server}` during {phase}")]
    Serialize {
        server: String,
        phase: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to write to MCP server `{server}` during {phase}")]
    Write {
        server: String,
        phase: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read from MCP server `{server}` during {phase}")]
    Read {
        server: String,
        phase: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("timed out waiting for MCP server `{server}` during {phase} after {timeout_secs}s")]
    Timeout {
        server: String,
        phase: &'static str,
        timeout_secs: u64,
    },
    #[error("MCP server `{server}` closed stdout while waiting for {phase}")]
    Closed { server: String, phase: &'static str },
    #[error("failed to decode JSON from MCP server `{server}` during {phase}")]
    Decode {
        server: String,
        phase: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("MCP server `{server}` returned an invalid response during {phase}: {message}")]
    Protocol {
        server: String,
        phase: &'static str,
        message: String,
    },
    #[error("MCP server `{server}` returned JSON-RPC error {code}: {message}")]
    Rpc {
        server: String,
        code: i64,
        message: String,
    },
}

#[derive(Debug, Deserialize, Default)]
struct RawMcpConfig {
    #[serde(default, rename = "mcp_servers")]
    servers: BTreeMap<String, RawMcpServer>,
}

#[derive(Debug, Deserialize, Default)]
struct RawMcpServer {
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<PathBuf>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    url: Option<String>,
    #[serde(default, rename = "http_headers")]
    http_headers: BTreeMap<String, String>,
    enabled: Option<bool>,
    startup_timeout_secs: Option<u64>,
    request_timeout_secs: Option<u64>,
    #[serde(default)]
    capabilities: RawMcpCapabilities,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawMcpCapabilities {
    #[serde(default, alias = "tools")]
    supports_tools: bool,
    #[serde(default, alias = "prompts")]
    supports_prompts: bool,
    #[serde(default, alias = "resources")]
    supports_resources: bool,
    #[serde(default, alias = "sampling")]
    supports_sampling: bool,
    #[serde(default, alias = "roots")]
    supports_roots: bool,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: T,
}

#[derive(Debug, Serialize)]
struct JsonRpcNotification<T> {
    jsonrpc: &'static str,
    method: &'static str,
    params: T,
}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcErrorPayload>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcErrorPayload {
    code: i64,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams<'a> {
    protocol_version: &'a str,
    capabilities: Value,
    client_info: &'a McpClientInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpInitializeResult {
    protocol_version: String,
    #[serde(default)]
    capabilities: Value,
    #[serde(default)]
    server_info: Option<McpPeerInfo>,
    #[serde(default)]
    instructions: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpToolsListResult {
    #[serde(default)]
    tools: Vec<McpToolDescriptor>,
}

#[derive(Debug, Serialize)]
struct ToolCallParams<'a> {
    name: &'a str,
    arguments: Value,
}

struct StdioMcpSession {
    server_name: String,
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    initialized: McpInitializeResult,
    request_timeout_secs: u64,
}

fn default_enabled() -> bool {
    true
}

impl From<RawMcpCapabilities> for McpCapabilityMatrix {
    fn from(value: RawMcpCapabilities) -> Self {
        Self {
            supports_tools: value.supports_tools,
            supports_prompts: value.supports_prompts,
            supports_resources: value.supports_resources,
            supports_sampling: value.supports_sampling,
            supports_roots: value.supports_roots,
        }
    }
}

impl McpConfig {
    pub fn from_toml_str(input: &str) -> Result<Self, McpConfigError> {
        let raw: RawMcpConfig = toml::from_str(input).map_err(|source| McpConfigError::Parse {
            path: PathBuf::from("<memory>"),
            source,
        })?;
        Self::from_raw(raw)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, McpConfigError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|source| McpConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let raw: RawMcpConfig =
            toml::from_str(&content).map_err(|source| McpConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawMcpConfig) -> Result<Self, McpConfigError> {
        let mut servers = BTreeMap::new();

        for (name, raw_server) in raw.servers {
            let transport = match (&raw_server.command, &raw_server.url) {
                (Some(_), Some(_)) => {
                    return Err(McpConfigError::AmbiguousTransport { name });
                }
                (None, None) => return Err(McpConfigError::MissingTransport { name }),
                (Some(command), None) => McpTransportConfig::Stdio {
                    command: command.clone(),
                    args: raw_server.args,
                    cwd: raw_server.cwd,
                    env: raw_server.env,
                },
                (None, Some(url)) => {
                    let headers = raw_server.http_headers;
                    match infer_transport_kind(url) {
                        McpTransport::Http => McpTransportConfig::Http {
                            url: url.clone(),
                            headers,
                        },
                        McpTransport::WebSocket => McpTransportConfig::WebSocket {
                            url: url.clone(),
                            headers,
                        },
                        McpTransport::Stdio => {
                            let scheme = url
                                .split(':')
                                .next()
                                .map_or_else(String::new, str::to_owned);
                            return Err(McpConfigError::UnsupportedUrlScheme { name, scheme });
                        }
                    }
                }
            };

            let server_name = name.clone();
            servers.insert(
                name,
                McpServerConfig {
                    name: server_name,
                    enabled: raw_server.enabled.unwrap_or(true),
                    transport,
                    capabilities: raw_server.capabilities.into(),
                    startup_timeout_secs: raw_server.startup_timeout_secs,
                    request_timeout_secs: raw_server.request_timeout_secs,
                    metadata: raw_server.metadata,
                },
            );
        }

        Ok(Self { servers })
    }
}

pub async fn inspect_server(
    server: &McpServerConfig,
    client_info: &McpClientInfo,
) -> Result<McpServerInspection, McpRuntimeError> {
    match &server.transport {
        McpTransportConfig::Stdio {
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

pub async fn call_tool(
    server: &McpServerConfig,
    client_info: &McpClientInfo,
    tool_name: &str,
    arguments: Value,
) -> Result<McpToolCallResponse, McpRuntimeError> {
    match &server.transport {
        McpTransportConfig::Stdio {
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

pub fn discover_mcp_configs(root: &Path) -> Vec<PathBuf> {
    let mut configs = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == DEFAULT_MCP_CONFIG_FILE)
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    configs.sort();
    configs
}

pub fn load_discovered_mcp_configs(
    root: &Path,
) -> Result<Vec<DiscoveredMcpConfig>, McpConfigError> {
    discover_mcp_configs(root)
        .into_iter()
        .map(|path| {
            let config = McpConfig::load(&path)?;
            Ok(DiscoveredMcpConfig { path, config })
        })
        .collect()
}

fn infer_transport_kind(url: &str) -> McpTransport {
    if url.starts_with("http://") || url.starts_with("https://") {
        McpTransport::Http
    } else if url.starts_with("ws://") || url.starts_with("wss://") {
        McpTransport::WebSocket
    } else {
        McpTransport::Stdio
    }
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

impl StdioMcpSession {
    async fn connect(
        server: &McpServerConfig,
        command: &str,
        args: &[String],
        cwd: Option<&Path>,
        env: &BTreeMap<String, String>,
        client_info: &McpClientInfo,
    ) -> Result<Self, McpRuntimeError> {
        let mut process = Command::new(command);
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
            command: command.to_owned(),
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

fn rpc_id_matches(id: &Value, request_id: u64) -> bool {
    id.as_u64() == Some(request_id)
        || id.as_i64() == Some(request_id as i64)
        || id
            .as_str()
            .is_some_and(|value| value == request_id.to_string())
}

async fn shutdown_child(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as ProcessCommand;
    use tempfile::tempdir;

    fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn parses_stdio_and_http_servers() {
        let config = ok(McpConfig::from_toml_str(
            r#"
                [mcp_servers.brave]
                command = "npx"
                args = ["-y", "@modelcontextprotocol/server-brave-search"]
                startup_timeout_secs = 5

                [mcp_servers.brave.env]
                BRAVE_API_KEY = "secret"

                [mcp_servers.context7]
                url = "https://mcp.context7.com/mcp"
                enabled = false
                request_timeout_secs = 15

                [mcp_servers.context7.http_headers]
                Authorization = "Bearer test"

                [mcp_servers.context7.capabilities]
                tools = true
                resources = true
            "#,
        ));

        let brave = match config.servers.get("brave") {
            Some(server) => server,
            None => panic!("missing brave server"),
        };
        assert!(brave.enabled);
        assert_eq!(brave.transport.kind(), McpTransport::Stdio);
        assert_eq!(brave.startup_timeout_secs, Some(5));

        let context7 = match config.servers.get("context7") {
            Some(server) => server,
            None => panic!("missing context7 server"),
        };
        assert!(!context7.enabled);
        assert_eq!(context7.transport.kind(), McpTransport::Http);
        assert!(context7.capabilities.supports_tools);
        assert!(context7.capabilities.supports_resources);
    }

    #[test]
    fn parses_websocket_server() {
        let config = ok(McpConfig::from_toml_str(
            r#"
                [mcp_servers.relay]
                url = "wss://example.com/mcp"
            "#,
        ));

        let relay = match config.servers.get("relay") {
            Some(server) => server,
            None => panic!("missing relay server"),
        };
        assert_eq!(relay.transport.kind(), McpTransport::WebSocket);
    }

    #[test]
    fn rejects_server_without_transport() {
        let error = McpConfig::from_toml_str(
            r#"
                [mcp_servers.invalid]
                enabled = true
            "#,
        )
        .expect_err("server without transport should fail");

        assert!(matches!(
            error,
            McpConfigError::MissingTransport { ref name } if name == "invalid"
        ));
    }

    #[test]
    fn discovers_and_loads_configs() {
        let temp = ok(tempdir());
        let root = temp.path();
        let nested = root.join("plugins").join("example");
        ok(fs::create_dir_all(&nested));
        ok(fs::write(
            root.join(DEFAULT_MCP_CONFIG_FILE),
            "[mcp_servers.one]\ncommand = \"uvx\"\n",
        ));
        ok(fs::write(
            nested.join(DEFAULT_MCP_CONFIG_FILE),
            "[mcp_servers.two]\nurl = \"https://example.com/mcp\"\n",
        ));

        let discovered = discover_mcp_configs(root);
        assert_eq!(discovered.len(), 2);

        let loaded = ok(load_discovered_mcp_configs(root));
        assert_eq!(loaded.len(), 2);
        assert!(
            loaded
                .iter()
                .any(|entry| entry.config.servers.contains_key("one"))
        );
        assert!(
            loaded
                .iter()
                .any(|entry| entry.config.servers.contains_key("two"))
        );
    }

    #[tokio::test]
    async fn inspects_stdio_server_and_lists_tools() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping MCP stdio inspection test because Python is unavailable.");
            return;
        };

        let temp = ok(tempdir());
        let script = temp.path().join("mock_mcp.py");
        ok(fs::write(
            &script,
            r#"
import json
import sys

def send(payload):
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()

for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    message = json.loads(raw)
    method = message.get("method")
    message_id = message.get("id")
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": message_id,
            "result": {
                "protocolVersion": "2025-03-26",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mock-mcp", "version": "0.1.0"},
                "instructions": "Use mock tools"
            }
        })
    elif method == "notifications/initialized":
        continue
    elif method == "tools/list":
        send({
            "jsonrpc": "2.0",
            "id": message_id,
            "result": {
                "tools": [
                    {
                        "name": "search",
                        "description": "Search indexed documentation",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": {"type": "string"}
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "fetch",
                        "description": "Fetch a resource by URL",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": {"type": "string"}
                            }
                        }
                    }
                ]
            }
        })
    else:
        send({
            "jsonrpc": "2.0",
            "id": message_id,
            "error": {"code": -32601, "message": "unknown method"}
        })
"#,
        ));
        prefix_args.push(script.to_string_lossy().into_owned());

        let server = McpServerConfig {
            name: "mock".to_owned(),
            enabled: true,
            transport: McpTransportConfig::Stdio {
                command: python,
                args: prefix_args,
                cwd: Some(temp.path().to_path_buf()),
                env: BTreeMap::new(),
            },
            capabilities: McpCapabilityMatrix::default(),
            startup_timeout_secs: Some(3),
            request_timeout_secs: Some(3),
            metadata: BTreeMap::new(),
        };

        let inspection = inspect_server(&server, &McpClientInfo::new("remote-code-rust", "test"))
            .await
            .unwrap_or_else(|error| panic!("inspection failed: {error}"));

        assert_eq!(inspection.server_name, "mock");
        assert_eq!(inspection.protocol_version, "2025-03-26");
        assert_eq!(
            inspection
                .server_info
                .as_ref()
                .map(|info| info.name.as_str()),
            Some("mock-mcp")
        );
        assert_eq!(inspection.tools.len(), 2);
        assert_eq!(inspection.tools[0].name, "search");
        assert_eq!(
            inspection.tools[0].description.as_deref(),
            Some("Search indexed documentation")
        );
    }

    #[tokio::test]
    async fn calls_stdio_tool_and_returns_typed_result() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping MCP stdio tool call test because Python is unavailable.");
            return;
        };

        let temp = ok(tempdir());
        let script = temp.path().join("mock_tool_call.py");
        ok(fs::write(&script, mock_tool_call_server_script()));
        prefix_args.push(script.to_string_lossy().into_owned());
        prefix_args.push("success".to_owned());

        let server = McpServerConfig {
            name: "mock".to_owned(),
            enabled: true,
            transport: McpTransportConfig::Stdio {
                command: python,
                args: prefix_args,
                cwd: Some(temp.path().to_path_buf()),
                env: BTreeMap::new(),
            },
            capabilities: McpCapabilityMatrix::default(),
            startup_timeout_secs: Some(3),
            request_timeout_secs: Some(3),
            metadata: BTreeMap::new(),
        };

        let response = call_tool(
            &server,
            &McpClientInfo::new("remote-code-rust", "test"),
            "echo",
            serde_json::json!({"text": "hello"}),
        )
        .await
        .unwrap_or_else(|error| panic!("tool call failed: {error}"));

        assert_eq!(response.server_name, "mock");
        assert_eq!(response.tool_name, "echo");
        assert_eq!(response.protocol_version, "2025-03-26");
        assert_eq!(
            response.server_info.as_ref().map(|info| info.name.as_str()),
            Some("mock-mcp")
        );
        assert!(!response.result.is_error);
        assert_eq!(response.result.content.len(), 1);
        assert_eq!(response.result.content[0].kind, "text");
        assert_eq!(
            response.result.content[0]
                .fields
                .get("text")
                .and_then(Value::as_str),
            Some("echo: hello")
        );
        assert_eq!(
            response.result.structured_content,
            Some(serde_json::json!({"echoed": "hello"}))
        );
    }

    #[tokio::test]
    async fn preserves_tool_error_payloads() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping MCP tool error payload test because Python is unavailable.");
            return;
        };

        let temp = ok(tempdir());
        let script = temp.path().join("mock_tool_call.py");
        ok(fs::write(&script, mock_tool_call_server_script()));
        prefix_args.push(script.to_string_lossy().into_owned());
        prefix_args.push("tool_error".to_owned());

        let server = McpServerConfig {
            name: "mock".to_owned(),
            enabled: true,
            transport: McpTransportConfig::Stdio {
                command: python,
                args: prefix_args,
                cwd: Some(temp.path().to_path_buf()),
                env: BTreeMap::new(),
            },
            capabilities: McpCapabilityMatrix::default(),
            startup_timeout_secs: Some(3),
            request_timeout_secs: Some(3),
            metadata: BTreeMap::new(),
        };

        let response = call_tool(
            &server,
            &McpClientInfo::new("remote-code-rust", "test"),
            "echo",
            serde_json::json!({"text": "boom"}),
        )
        .await
        .unwrap_or_else(|error| panic!("tool error payload should remain typed: {error}"));

        assert!(response.result.is_error);
        assert_eq!(
            response.result.content[0]
                .fields
                .get("text")
                .and_then(Value::as_str),
            Some("tool execution failed")
        );
    }

    #[tokio::test]
    async fn surfaces_json_rpc_errors_from_tool_call() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping MCP JSON-RPC error test because Python is unavailable.");
            return;
        };

        let temp = ok(tempdir());
        let script = temp.path().join("mock_tool_call.py");
        ok(fs::write(&script, mock_tool_call_server_script()));
        prefix_args.push(script.to_string_lossy().into_owned());
        prefix_args.push("rpc_error".to_owned());

        let server = McpServerConfig {
            name: "mock".to_owned(),
            enabled: true,
            transport: McpTransportConfig::Stdio {
                command: python,
                args: prefix_args,
                cwd: Some(temp.path().to_path_buf()),
                env: BTreeMap::new(),
            },
            capabilities: McpCapabilityMatrix::default(),
            startup_timeout_secs: Some(3),
            request_timeout_secs: Some(3),
            metadata: BTreeMap::new(),
        };

        let error = call_tool(
            &server,
            &McpClientInfo::new("remote-code-rust", "test"),
            "echo",
            serde_json::json!({"text": "boom"}),
        )
        .await
        .expect_err("JSON-RPC tool call failure should surface as runtime error");

        assert!(matches!(
            error,
            McpRuntimeError::Rpc {
                code: -32001,
                ref message,
                ..
            } if message == "tool call failed"
        ));
    }

    #[tokio::test]
    async fn rejects_non_stdio_runtime_transports() {
        let server = McpServerConfig {
            name: "relay".to_owned(),
            enabled: true,
            transport: McpTransportConfig::Http {
                url: "https://example.com/mcp".to_owned(),
                headers: BTreeMap::new(),
            },
            capabilities: McpCapabilityMatrix::default(),
            startup_timeout_secs: None,
            request_timeout_secs: None,
            metadata: BTreeMap::new(),
        };

        let error = inspect_server(&server, &McpClientInfo::default())
            .await
            .expect_err("http runtime inspection should not be supported yet");
        assert!(matches!(
            error,
            McpRuntimeError::UnsupportedTransport {
                transport: McpTransport::Http,
                ..
            }
        ));
    }

    fn python_command() -> Option<(String, Vec<String>)> {
        if let Ok(path) = std::env::var("PYTHON")
            && ProcessCommand::new(&path)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
        {
            return Some((path, Vec::new()));
        }

        for candidate in ["python", "python3"] {
            if ProcessCommand::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
            {
                return Some((candidate.to_owned(), Vec::new()));
            }
        }

        if cfg!(windows)
            && ProcessCommand::new("py")
                .args(["-3", "--version"])
                .output()
                .is_ok_and(|output| output.status.success())
        {
            return Some(("py".to_owned(), vec!["-3".to_owned()]));
        }

        None
    }

    fn mock_tool_call_server_script() -> &'static str {
        r#"
import json
import sys

mode = sys.argv[1]

def send(payload):
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()

for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    message = json.loads(raw)
    method = message.get("method")
    message_id = message.get("id")
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": message_id,
            "result": {
                "protocolVersion": "2025-03-26",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mock-mcp", "version": "0.1.0"}
            }
        })
    elif method == "notifications/initialized":
        continue
    elif method == "tools/call":
        if mode == "rpc_error":
            send({
                "jsonrpc": "2.0",
                "id": message_id,
                "error": {"code": -32001, "message": "tool call failed"}
            })
        elif mode == "tool_error":
            send({
                "jsonrpc": "2.0",
                "id": message_id,
                "result": {
                    "content": [
                        {"type": "text", "text": "tool execution failed"}
                    ],
                    "isError": True
                }
            })
        else:
            text = message.get("params", {}).get("arguments", {}).get("text", "")
            send({
                "jsonrpc": "2.0",
                "id": message_id,
                "result": {
                    "content": [
                        {"type": "text", "text": f"echo: {text}"}
                    ],
                    "structuredContent": {
                        "echoed": text
                    },
                    "isError": False
                }
            })
"#
    }
}
