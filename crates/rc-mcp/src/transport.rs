//! Transport types for MCP communication.
//!
//! Includes the original `McpTransport` / `McpTransportConfig` (backward-compatible)
//! plus the extended `McpTransportKind` (8 variants), `McpOAuthConfig`, and
//! detailed `TransportConfig` variants for future HTTP/SSE/WS support.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Original transport types (backward-compatible) ──────────────────────────

/// Transport protocol for MCP communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    /// Standard I/O (child process).
    Stdio,
    /// HTTP-based transport.
    Http,
    /// WebSocket transport.
    WebSocket,
}

/// Configuration for a specific MCP transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpTransportConfig {
    /// Launch a child process and communicate over stdio.
    Stdio {
        /// Command to execute.
        command: String,
        /// Command-line arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Working directory for the child process.
        #[serde(default)]
        cwd: Option<PathBuf>,
        /// Environment variables to set.
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    /// Connect to an HTTP endpoint.
    Http {
        /// Server URL.
        url: String,
        /// Additional HTTP headers.
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    /// Connect via WebSocket.
    WebSocket {
        /// WebSocket URL.
        url: String,
        /// Additional HTTP headers.
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl McpTransportConfig {
    /// Return the transport kind for this configuration.
    #[must_use]
    pub fn kind(&self) -> McpTransport {
        match self {
            Self::Stdio { .. } => McpTransport::Stdio,
            Self::Http { .. } => McpTransport::Http,
            Self::WebSocket { .. } => McpTransport::WebSocket,
        }
    }
}

// ── Extended transport kinds (8 variants) ───────────────────────────────────

/// Extended transport kind covering all MCP transport variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpTransportKind {
    /// Standard I/O (child process).
    #[serde(rename = "stdio")]
    Stdio,
    /// Server-Sent Events.
    #[serde(rename = "sse")]
    Sse,
    /// SSE via IDE integration.
    #[serde(rename = "sse-ide")]
    SseIde,
    /// HTTP streamable transport.
    #[serde(rename = "http")]
    Http,
    /// WebSocket.
    #[serde(rename = "ws")]
    WebSocket,
    /// WebSocket via IDE integration.
    #[serde(rename = "ws-ide")]
    WsIde,
    /// In-process SDK transport.
    #[serde(rename = "sdk")]
    Sdk,
    /// Claude.ai proxy transport.
    #[serde(rename = "claudeai-proxy")]
    ClaudeAiProxy,
}

impl McpTransportKind {
    /// Convert to the legacy [`McpTransport`] if possible.
    #[must_use]
    pub fn to_legacy(self) -> Option<McpTransport> {
        match self {
            Self::Stdio => Some(McpTransport::Stdio),
            Self::Http | Self::Sse | Self::SseIde => Some(McpTransport::Http),
            Self::WebSocket | Self::WsIde => Some(McpTransport::WebSocket),
            Self::Sdk | Self::ClaudeAiProxy => None,
        }
    }
}

impl std::fmt::Display for McpTransportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdio => write!(f, "stdio"),
            Self::Sse => write!(f, "sse"),
            Self::SseIde => write!(f, "sse-ide"),
            Self::Http => write!(f, "http"),
            Self::WebSocket => write!(f, "ws"),
            Self::WsIde => write!(f, "ws-ide"),
            Self::Sdk => write!(f, "sdk"),
            Self::ClaudeAiProxy => write!(f, "claudeai-proxy"),
        }
    }
}

// ── OAuth configuration ─────────────────────────────────────────────────────

/// OAuth configuration for MCP transports that require authentication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOAuthConfig {
    /// OAuth client ID.
    #[serde(default)]
    pub client_id: Option<String>,
    /// Callback port for the OAuth redirect URI.
    #[serde(default)]
    pub callback_port: Option<u16>,
    /// Authorization server metadata URL.
    #[serde(default)]
    pub auth_server_metadata_url: Option<String>,
    /// Whether to use X-Authorization-Header flow.
    #[serde(default)]
    pub xaa: Option<bool>,
}

// ── Detailed transport configuration ────────────────────────────────────────

/// Detailed transport configuration covering all MCP transport variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TransportConfig {
    /// Standard I/O transport.
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: Option<BTreeMap<String, String>>,
        #[serde(default)]
        cwd: Option<PathBuf>,
    },
    /// Server-Sent Events transport.
    Sse {
        url: String,
        #[serde(default)]
        headers: Option<BTreeMap<String, String>>,
        #[serde(default)]
        headers_helper: Option<String>,
        #[serde(default)]
        oauth: Option<McpOAuthConfig>,
    },
    /// SSE via IDE integration.
    #[serde(rename = "sse-ide")]
    SseIde {
        url: String,
        ide_name: String,
        #[serde(default)]
        ide_running_in_windows: Option<bool>,
    },
    /// HTTP streamable transport.
    Http {
        url: String,
        #[serde(default)]
        headers: Option<BTreeMap<String, String>>,
        #[serde(default)]
        headers_helper: Option<String>,
        #[serde(default)]
        oauth: Option<McpOAuthConfig>,
    },
    /// WebSocket transport.
    #[serde(rename = "ws")]
    WebSocket {
        url: String,
        #[serde(default)]
        headers: Option<BTreeMap<String, String>>,
        #[serde(default)]
        headers_helper: Option<String>,
    },
    /// WebSocket via IDE integration.
    #[serde(rename = "ws-ide")]
    WsIde {
        url: String,
        ide_name: String,
        #[serde(default)]
        auth_token: Option<String>,
        #[serde(default)]
        ide_running_in_windows: Option<bool>,
    },
    /// In-process SDK transport.
    Sdk {
        name: String,
    },
    /// Claude.ai proxy transport.
    #[serde(rename = "claudeai-proxy")]
    ClaudeAiProxy {
        url: String,
        id: String,
    },
}

impl TransportConfig {
    /// Return the transport kind for this configuration.
    #[must_use]
    pub fn kind(&self) -> McpTransportKind {
        match self {
            Self::Stdio { .. } => McpTransportKind::Stdio,
            Self::Sse { .. } => McpTransportKind::Sse,
            Self::SseIde { .. } => McpTransportKind::SseIde,
            Self::Http { .. } => McpTransportKind::Http,
            Self::WebSocket { .. } => McpTransportKind::WebSocket,
            Self::WsIde { .. } => McpTransportKind::WsIde,
            Self::Sdk { .. } => McpTransportKind::Sdk,
            Self::ClaudeAiProxy { .. } => McpTransportKind::ClaudeAiProxy,
        }
    }
}

/// Infer the legacy transport kind from a URL scheme.
pub(crate) fn infer_transport_kind(url: &str) -> McpTransport {
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

    #[test]
    fn legacy_transport_config_kind() {
        let stdio = McpTransportConfig::Stdio {
            command: "echo".to_owned(),
            args: vec![],
            cwd: None,
            env: BTreeMap::new(),
        };
        assert_eq!(stdio.kind(), McpTransport::Stdio);

        let http = McpTransportConfig::Http {
            url: "https://example.com".to_owned(),
            headers: BTreeMap::new(),
        };
        assert_eq!(http.kind(), McpTransport::Http);
    }

    #[test]
    fn transport_kind_to_legacy() {
        assert_eq!(McpTransportKind::Stdio.to_legacy(), Some(McpTransport::Stdio));
        assert_eq!(McpTransportKind::Http.to_legacy(), Some(McpTransport::Http));
        assert_eq!(McpTransportKind::Sse.to_legacy(), Some(McpTransport::Http));
        assert_eq!(McpTransportKind::WebSocket.to_legacy(), Some(McpTransport::WebSocket));
        assert_eq!(McpTransportKind::Sdk.to_legacy(), None);
        assert_eq!(McpTransportKind::ClaudeAiProxy.to_legacy(), None);
    }

    #[test]
    fn transport_kind_display() {
        assert_eq!(McpTransportKind::Stdio.to_string(), "stdio");
        assert_eq!(McpTransportKind::SseIde.to_string(), "sse-ide");
        assert_eq!(McpTransportKind::ClaudeAiProxy.to_string(), "claudeai-proxy");
    }

    #[test]
    fn infer_transport_kind_urls() {
        assert_eq!(infer_transport_kind("https://example.com"), McpTransport::Http);
        assert_eq!(infer_transport_kind("http://localhost:8080"), McpTransport::Http);
        assert_eq!(infer_transport_kind("wss://example.com/ws"), McpTransport::WebSocket);
        assert_eq!(infer_transport_kind("ws://localhost:9090"), McpTransport::WebSocket);
        assert_eq!(infer_transport_kind("custom://other"), McpTransport::Stdio);
    }

    #[test]
    fn transport_config_kind_matches() {
        let ws = TransportConfig::WebSocket {
            url: "wss://example.com".to_owned(),
            headers: None,
            headers_helper: None,
        };
        assert_eq!(ws.kind(), McpTransportKind::WebSocket);

        let sdk = TransportConfig::Sdk {
            name: "my-sdk".to_owned(),
        };
        assert_eq!(sdk.kind(), McpTransportKind::Sdk);
    }

    #[test]
    fn oauth_config_serialization() {
        let oauth = McpOAuthConfig {
            client_id: Some("abc".to_owned()),
            callback_port: Some(8080),
            auth_server_metadata_url: None,
            xaa: None,
        };
        let json = serde_json::to_string(&oauth).expect("serialize");
        let back: McpOAuthConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(oauth, back);
    }
}
