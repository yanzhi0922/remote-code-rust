//! Bridge connection status tracking.
//!
//! Corresponds to `.research/cc-haha/src/bridge/` status patterns.
//! Provides types for tracking bridge connection state, latency,
//! and formatting status information for display.

use crate::bridge_config::BridgeTransport;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Current status of a bridge connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatus {
    /// Not connected to any bridge endpoint.
    Disconnected,
    /// Connection is being established.
    Connecting,
    /// Successfully connected and ready for communication.
    Connected,
    /// Connection was lost; attempting to reconnect.
    Reconnecting,
    /// Connection has encountered an error.
    Error,
}

impl BridgeStatus {
    /// Check whether the status represents an active connection.
    pub fn is_connected(&self) -> bool {
        matches!(self, BridgeStatus::Connected)
    }

    /// Check whether the status represents a transient (in-progress) state.
    pub fn is_transient(&self) -> bool {
        matches!(self, BridgeStatus::Connecting | BridgeStatus::Reconnecting)
    }
}

impl std::fmt::Display for BridgeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeStatus::Disconnected => write!(f, "disconnected"),
            BridgeStatus::Connecting => write!(f, "connecting"),
            BridgeStatus::Connected => write!(f, "connected"),
            BridgeStatus::Reconnecting => write!(f, "reconnecting"),
            BridgeStatus::Error => write!(f, "error"),
        }
    }
}

/// Information about the current bridge connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConnectionInfo {
    /// Current connection status.
    pub status: BridgeStatus,
    /// Timestamp when the connection was established (ISO 8601).
    pub connected_since: Option<String>,
    /// Session identifier for this connection.
    pub session_id: Option<String>,
    /// Transport protocol in use.
    pub transport: BridgeTransport,
    /// Measured round-trip latency in milliseconds.
    pub latency_ms: Option<u64>,
}

impl BridgeConnectionInfo {
    /// Create a new connection info with the given status and transport.
    pub fn new(status: BridgeStatus, transport: BridgeTransport) -> Self {
        Self {
            status,
            connected_since: None,
            session_id: None,
            transport,
            latency_ms: None,
        }
    }

    /// Create a connection info representing a disconnected state.
    pub fn disconnected() -> Self {
        Self::new(BridgeStatus::Disconnected, BridgeTransport::Stdio)
    }

    /// Set the connected-since timestamp.
    pub fn with_connected_since(mut self, ts: String) -> Self {
        self.connected_since = Some(ts);
        self
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, id: String) -> Self {
        self.session_id = Some(id);
        self
    }

    /// Set the measured latency.
    pub fn with_latency(mut self, ms: u64) -> Self {
        self.latency_ms = Some(ms);
        self
    }
}

/// Format a [`BridgeConnectionInfo`] into a human-readable status string.
pub fn format_bridge_status(info: &BridgeConnectionInfo) -> String {
    let status_str = info.status.to_string();
    let transport_str = info.transport.to_string();

    let mut parts = vec![format!("status={status_str}"), format!("transport={transport_str}")];

    if let Some(ref since) = info.connected_since {
        parts.push(format!("since={since}"));
    }
    if let Some(ref sid) = info.session_id {
        parts.push(format!("session={sid}"));
    }
    if let Some(latency) = info.latency_ms {
        parts.push(format!("latency={latency}ms"));
    }

    format!("Bridge({})", parts.join(", "))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_display() {
        assert_eq!(BridgeStatus::Disconnected.to_string(), "disconnected");
        assert_eq!(BridgeStatus::Connecting.to_string(), "connecting");
        assert_eq!(BridgeStatus::Connected.to_string(), "connected");
        assert_eq!(BridgeStatus::Reconnecting.to_string(), "reconnecting");
        assert_eq!(BridgeStatus::Error.to_string(), "error");
    }

    #[test]
    fn status_is_connected() {
        assert!(BridgeStatus::Connected.is_connected());
        assert!(!BridgeStatus::Disconnected.is_connected());
        assert!(!BridgeStatus::Connecting.is_connected());
        assert!(!BridgeStatus::Reconnecting.is_connected());
        assert!(!BridgeStatus::Error.is_connected());
    }

    #[test]
    fn status_is_transient() {
        assert!(BridgeStatus::Connecting.is_transient());
        assert!(BridgeStatus::Reconnecting.is_transient());
        assert!(!BridgeStatus::Connected.is_transient());
        assert!(!BridgeStatus::Disconnected.is_transient());
        assert!(!BridgeStatus::Error.is_transient());
    }

    #[test]
    fn connection_info_disconnected() {
        let info = BridgeConnectionInfo::disconnected();
        assert_eq!(info.status, BridgeStatus::Disconnected);
        assert!(info.connected_since.is_none());
        assert!(info.session_id.is_none());
        assert!(info.latency_ms.is_none());
    }

    #[test]
    fn connection_info_builder() {
        let info = BridgeConnectionInfo::new(BridgeStatus::Connected, BridgeTransport::WebSocket)
            .with_connected_since("2026-01-01T00:00:00Z".to_string())
            .with_session_id("sess-abc".to_string())
            .with_latency(42);

        assert_eq!(info.status, BridgeStatus::Connected);
        assert_eq!(info.transport, BridgeTransport::WebSocket);
        assert_eq!(info.connected_since.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert_eq!(info.session_id.as_deref(), Some("sess-abc"));
        assert_eq!(info.latency_ms, Some(42));
    }

    #[test]
    fn format_status_minimal() {
        let info = BridgeConnectionInfo::disconnected();
        let formatted = format_bridge_status(&info);
        assert!(formatted.contains("status=disconnected"));
        assert!(formatted.contains("transport=stdio"));
        assert!(!formatted.contains("since="));
        assert!(!formatted.contains("session="));
        assert!(!formatted.contains("latency="));
    }

    #[test]
    fn format_status_full() {
        let info = BridgeConnectionInfo::new(BridgeStatus::Connected, BridgeTransport::WebSocket)
            .with_connected_since("2026-04-23T10:00:00Z".to_string())
            .with_session_id("sess-xyz".to_string())
            .with_latency(128);

        let formatted = format_bridge_status(&info);
        assert!(formatted.contains("status=connected"));
        assert!(formatted.contains("transport=websocket"));
        assert!(formatted.contains("since=2026-04-23T10:00:00Z"));
        assert!(formatted.contains("session=sess-xyz"));
        assert!(formatted.contains("latency=128ms"));
    }

    #[test]
    fn connection_info_serde_roundtrip() {
        let info = BridgeConnectionInfo::new(BridgeStatus::Connected, BridgeTransport::Http)
            .with_session_id("s1".to_string())
            .with_latency(50);

        let json = serde_json::to_string(&info).expect("serialize");
        let back: BridgeConnectionInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.status, info.status);
        assert_eq!(back.transport, info.transport);
        assert_eq!(back.session_id, info.session_id);
        assert_eq!(back.latency_ms, info.latency_ms);
    }

    #[test]
    fn status_serde_roundtrip() {
        for status in [
            BridgeStatus::Disconnected,
            BridgeStatus::Connecting,
            BridgeStatus::Connected,
            BridgeStatus::Reconnecting,
            BridgeStatus::Error,
        ] {
            let json = serde_json::to_string(&status).expect("serialize");
            let back: BridgeStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, status);
        }
    }
}
