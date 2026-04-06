use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;

pub const DEFAULT_MCP_CONFIG_FILE: &str = "mcp.toml";

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

#[cfg(test)]
mod tests {
    use super::*;
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
}
