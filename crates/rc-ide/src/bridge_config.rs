//! Bridge configuration for desktop-IDE communication.
//!
//! Corresponds to `.research/cc-haha/src/bridge/` configuration patterns.
//! Provides configuration loading, transport selection, and default values
//! for the bridge that connects remote-code to desktop IDEs.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Transport protocol used for bridge communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeTransport {
    /// Standard I/O (stdin/stdout) transport.
    Stdio,
    /// WebSocket transport for persistent connections.
    WebSocket,
    /// HTTP-based transport for request/response communication.
    Http,
}

impl std::fmt::Display for BridgeTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeTransport::Stdio => write!(f, "stdio"),
            BridgeTransport::WebSocket => write!(f, "websocket"),
            BridgeTransport::Http => write!(f, "http"),
        }
    }
}

/// Configuration for the desktop bridge connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Whether the bridge is enabled.
    pub enabled: bool,
    /// Transport protocol to use.
    pub transport: BridgeTransport,
    /// Endpoint URL (for WebSocket and HTTP transports).
    pub endpoint: Option<String>,
    /// Unique session identifier for this bridge connection.
    pub session_id: Option<String>,
    /// Reconnection interval in milliseconds.
    pub reconnect_interval_ms: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            transport: BridgeTransport::Stdio,
            endpoint: None,
            session_id: None,
            reconnect_interval_ms: 5000,
        }
    }
}

impl BridgeConfig {
    /// Create a new configuration with the specified transport.
    pub fn new(transport: BridgeTransport) -> Self {
        Self {
            enabled: true,
            transport,
            ..Self::default()
        }
    }

    /// Create a configuration with an explicit endpoint.
    pub fn with_endpoint(transport: BridgeTransport, endpoint: String) -> Self {
        Self {
            enabled: true,
            transport,
            endpoint: Some(endpoint),
            ..Self::default()
        }
    }

    /// Set the session ID.
    pub fn session_id(mut self, id: String) -> Self {
        self.session_id = Some(id);
        self
    }

    /// Set the reconnection interval.
    pub fn reconnect_interval(mut self, ms: u64) -> Self {
        self.reconnect_interval_ms = ms;
        self
    }

    /// Check whether this configuration uses a network-based transport.
    pub fn is_network_transport(&self) -> bool {
        matches!(self.transport, BridgeTransport::WebSocket | BridgeTransport::Http)
    }
}

// ---------------------------------------------------------------------------
// Configuration loading
// ---------------------------------------------------------------------------

/// Load bridge configuration from settings.
///
/// In a full implementation this would read from the user's settings file.
/// Currently returns the default configuration.
pub fn load_bridge_config() -> anyhow::Result<BridgeConfig> {
    // In a full implementation, this would:
    // 1. Check for bridge settings in the project config
    // 2. Check for environment variable overrides
    // 3. Fall back to defaults
    Ok(default_bridge_config())
}

/// Return the default bridge configuration.
pub fn default_bridge_config() -> BridgeConfig {
    BridgeConfig::default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let config = BridgeConfig::default();
        assert!(!config.enabled);
    }

    #[test]
    fn default_config_uses_stdio() {
        let config = BridgeConfig::default();
        assert_eq!(config.transport, BridgeTransport::Stdio);
    }

    #[test]
    fn default_config_has_no_endpoint() {
        let config = BridgeConfig::default();
        assert!(config.endpoint.is_none());
    }

    #[test]
    fn default_config_has_no_session_id() {
        let config = BridgeConfig::default();
        assert!(config.session_id.is_none());
    }

    #[test]
    fn default_reconnect_interval() {
        let config = BridgeConfig::default();
        assert_eq!(config.reconnect_interval_ms, 5000);
    }

    #[test]
    fn new_config_is_enabled() {
        let config = BridgeConfig::new(BridgeTransport::WebSocket);
        assert!(config.enabled);
        assert_eq!(config.transport, BridgeTransport::WebSocket);
    }

    #[test]
    fn with_endpoint_sets_endpoint() {
        let config = BridgeConfig::with_endpoint(
            BridgeTransport::Http,
            "http://localhost:8080".to_string(),
        );
        assert!(config.enabled);
        assert_eq!(
            config.endpoint.as_deref(),
            Some("http://localhost:8080")
        );
    }

    #[test]
    fn builder_pattern_session_id() {
        let config = BridgeConfig::new(BridgeTransport::Stdio)
            .session_id("sess-123".to_string());
        assert_eq!(config.session_id.as_deref(), Some("sess-123"));
    }

    #[test]
    fn builder_pattern_reconnect_interval() {
        let config = BridgeConfig::new(BridgeTransport::WebSocket)
            .reconnect_interval(10000);
        assert_eq!(config.reconnect_interval_ms, 10000);
    }

    #[test]
    fn is_network_transport_stdio() {
        let config = BridgeConfig::new(BridgeTransport::Stdio);
        assert!(!config.is_network_transport());
    }

    #[test]
    fn is_network_transport_websocket() {
        let config = BridgeConfig::new(BridgeTransport::WebSocket);
        assert!(config.is_network_transport());
    }

    #[test]
    fn is_network_transport_http() {
        let config = BridgeConfig::new(BridgeTransport::Http);
        assert!(config.is_network_transport());
    }

    #[test]
    fn transport_display() {
        assert_eq!(BridgeTransport::Stdio.to_string(), "stdio");
        assert_eq!(BridgeTransport::WebSocket.to_string(), "websocket");
        assert_eq!(BridgeTransport::Http.to_string(), "http");
    }

    #[test]
    fn load_bridge_config_returns_default() {
        let config = load_bridge_config().expect("load");
        assert_eq!(config.transport, BridgeTransport::Stdio);
        assert!(!config.enabled);
    }

    #[test]
    fn config_serde_roundtrip() {
        let config = BridgeConfig::with_endpoint(
            BridgeTransport::WebSocket,
            "ws://localhost:9090".to_string(),
        )
        .session_id("abc".to_string())
        .reconnect_interval(3000);

        let json = serde_json::to_string(&config).expect("serialize");
        let back: BridgeConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.enabled, config.enabled);
        assert_eq!(back.transport, config.transport);
        assert_eq!(back.endpoint, config.endpoint);
        assert_eq!(back.session_id, config.session_id);
        assert_eq!(back.reconnect_interval_ms, config.reconnect_interval_ms);
    }
}
