//! MCP configuration loading, parsing, and saving.
//!
//! Handles TOML-based configuration files with support for stdio, HTTP,
//! and WebSocket transports. Includes raw intermediate types for TOML
//! serialization/deserialization.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::McpConfigError;
use crate::transport::{infer_transport_kind, McpTransport, McpTransportConfig};

/// Capability flags reported by an MCP server during initialisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct McpCapabilityMatrix {
    /// Server supports the `tools` capability.
    #[serde(default)]
    pub supports_tools: bool,
    /// Server supports the `prompts` capability.
    #[serde(default)]
    pub supports_prompts: bool,
    /// Server supports the `resources` capability.
    #[serde(default)]
    pub supports_resources: bool,
    /// Server supports the `sampling` capability.
    #[serde(default)]
    pub supports_sampling: bool,
    /// Server supports the `roots` capability.
    #[serde(default)]
    pub supports_roots: bool,
}

/// Configuration for a single MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name (used as a key in the config map).
    pub name: String,
    /// Whether the server is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Transport configuration.
    pub transport: McpTransportConfig,
    /// Reported capabilities.
    #[serde(default)]
    pub capabilities: McpCapabilityMatrix,
    /// Startup timeout override in seconds.
    #[serde(default)]
    pub startup_timeout_secs: Option<u64>,
    /// Request timeout override in seconds.
    #[serde(default)]
    pub request_timeout_secs: Option<u64>,
    /// Arbitrary metadata.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

/// Top-level MCP configuration containing all servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct McpConfig {
    /// Map of server name → server configuration.
    pub servers: BTreeMap<String, McpServerConfig>,
}

/// An MCP configuration file discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredMcpConfig {
    /// Path to the configuration file.
    pub path: PathBuf,
    /// Parsed configuration.
    pub config: McpConfig,
}

// ── Raw TOML intermediate types ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct RawMcpConfig {
    #[serde(default, rename = "mcp_servers", alias = "servers")]
    pub(crate) servers: BTreeMap<String, RawMcpServer>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct RawMcpServer {
    pub(crate) command: Option<String>,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
    #[serde(default)]
    pub(crate) env: BTreeMap<String, String>,
    pub(crate) url: Option<String>,
    #[serde(default, rename = "http_headers")]
    pub(crate) http_headers: BTreeMap<String, String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) startup_timeout_secs: Option<u64>,
    pub(crate) request_timeout_secs: Option<u64>,
    #[serde(default)]
    pub(crate) capabilities: RawMcpCapabilities,
    #[serde(default)]
    pub(crate) metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct RawMcpCapabilities {
    #[serde(default, alias = "tools")]
    pub(crate) supports_tools: bool,
    #[serde(default, alias = "prompts")]
    pub(crate) supports_prompts: bool,
    #[serde(default, alias = "resources")]
    pub(crate) supports_resources: bool,
    #[serde(default, alias = "sampling")]
    pub(crate) supports_sampling: bool,
    #[serde(default, alias = "roots")]
    pub(crate) supports_roots: bool,
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
    /// Parse an MCP configuration from a TOML string.
    pub fn from_toml_str(input: &str) -> Result<Self, McpConfigError> {
        let raw: RawMcpConfig = toml::from_str(input).map_err(|source| McpConfigError::Parse {
            path: PathBuf::from("<memory>"),
            source,
        })?;
        Self::from_raw(raw)
    }

    /// Load an MCP configuration from a file on disk.
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

    /// Serialize the configuration to a TOML string.
    pub fn to_toml_string(&self) -> Result<String, McpConfigError> {
        toml::to_string_pretty(&RawMcpConfig::from(self))
            .map_err(|source| McpConfigError::Serialize { source })
    }

    /// Save the configuration to a file on disk.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), McpConfigError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| McpConfigError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let contents = self.to_toml_string()?;
        fs::write(path, contents).map_err(|source| McpConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }
}

impl From<&McpConfig> for RawMcpConfig {
    fn from(value: &McpConfig) -> Self {
        let servers = value
            .servers
            .iter()
            .map(|(name, server)| {
                let (command, args, cwd, env, url, http_headers) = match &server.transport {
                    McpTransportConfig::Stdio {
                        command,
                        args,
                        cwd,
                        env,
                    } => (
                        Some(command.clone()),
                        args.clone(),
                        cwd.clone(),
                        env.clone(),
                        None,
                        BTreeMap::new(),
                    ),
                    McpTransportConfig::Http { url, headers } => (
                        None,
                        Vec::new(),
                        None,
                        BTreeMap::new(),
                        Some(url.clone()),
                        headers.clone(),
                    ),
                    McpTransportConfig::WebSocket { url, headers } => (
                        None,
                        Vec::new(),
                        None,
                        BTreeMap::new(),
                        Some(url.clone()),
                        headers.clone(),
                    ),
                };

                (
                    name.clone(),
                    RawMcpServer {
                        command,
                        args,
                        cwd,
                        env,
                        url,
                        http_headers,
                        enabled: Some(server.enabled),
                        startup_timeout_secs: server.startup_timeout_secs,
                        request_timeout_secs: server.request_timeout_secs,
                        capabilities: RawMcpCapabilities {
                            supports_tools: server.capabilities.supports_tools,
                            supports_prompts: server.capabilities.supports_prompts,
                            supports_resources: server.capabilities.supports_resources,
                            supports_sampling: server.capabilities.supports_sampling,
                            supports_roots: server.capabilities.supports_roots,
                        },
                        metadata: server.metadata.clone(),
                    },
                )
            })
            .collect();
        Self { servers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_matrix_default() {
        let caps = McpCapabilityMatrix::default();
        assert!(!caps.supports_tools);
        assert!(!caps.supports_prompts);
        assert!(!caps.supports_resources);
        assert!(!caps.supports_sampling);
        assert!(!caps.supports_roots);
    }

    #[test]
    fn config_default_is_empty() {
        let config = McpConfig::default();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn parses_minimal_stdio() {
        let config = McpConfig::from_toml_str(
            r#"[mcp_servers.echo]
command = "echo""#,
        )
        .expect("should parse");
        assert_eq!(config.servers.len(), 1);
        let echo = &config.servers["echo"];
        assert!(echo.enabled);
        assert_eq!(echo.transport.kind(), McpTransport::Stdio);
    }

    #[test]
    fn rejects_ambiguous_transport() {
        let err = McpConfig::from_toml_str(
            r#"[mcp_servers.bad]
command = "echo"
url = "https://example.com""#,
        )
        .expect_err("should fail");
        assert!(matches!(err, McpConfigError::AmbiguousTransport { .. }));
    }

    #[test]
    fn toml_roundtrip() {
        let config = McpConfig {
            servers: BTreeMap::from([(
                "demo".to_owned(),
                McpServerConfig {
                    name: "demo".to_owned(),
                    enabled: true,
                    transport: McpTransportConfig::Stdio {
                        command: "echo".to_owned(),
                        args: vec![],
                        cwd: None,
                        env: BTreeMap::new(),
                    },
                    capabilities: McpCapabilityMatrix::default(),
                    startup_timeout_secs: None,
                    request_timeout_secs: None,
                    metadata: BTreeMap::new(),
                },
            )]),
        };
        let toml_str = config.to_toml_string().expect("serialize");
        let back = McpConfig::from_toml_str(&toml_str).expect("deserialize");
        assert_eq!(config, back);
    }
}
