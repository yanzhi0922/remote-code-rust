//! IDE bridge — main interface between remote-code and the IDE.
//!
//! [`IdeBridge`] is the primary entry point for sending notifications and
//! requesting actions from the connected IDE.

use crate::config::IdeConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Notification types
// ---------------------------------------------------------------------------

/// Kinds of notifications that can be sent to the IDE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// Inform the IDE of a status change.
    StatusChange,
    /// Display an informational message.
    Info,
    /// Display a warning message.
    Warning,
    /// Display an error message.
    Error,
    /// Notify the IDE that a file has been modified.
    FileModified,
}

/// A notification sent from remote-code to the IDE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeNotification {
    /// What kind of notification this is.
    pub kind: NotificationKind,
    /// Human-readable message.
    pub message: String,
    /// Additional metadata key-value pairs.
    pub metadata: HashMap<String, String>,
}

impl IdeNotification {
    /// Create a simple notification with no metadata.
    pub fn simple(kind: NotificationKind, message: String) -> Self {
        Self {
            kind,
            message,
            metadata: HashMap::new(),
        }
    }

    /// Create a notification with metadata.
    pub fn with_metadata(kind: NotificationKind, message: String, metadata: HashMap<String, String>) -> Self {
        Self { kind, message, metadata }
    }
}

// ---------------------------------------------------------------------------
// Action / Response types
// ---------------------------------------------------------------------------

/// Kinds of actions that can be requested from the IDE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Open a file in the editor.
    FileOpen,
    /// Apply a diff/patch to a file.
    DiffApply,
    /// Request diagnostics for a file.
    GetDiagnostics,
    /// Show a quick-pick or selection dialog.
    ShowQuickPick,
    /// Execute a command in the IDE.
    ExecuteCommand,
}

/// A request for the IDE to perform an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeAction {
    /// What kind of action to perform.
    pub kind: ActionKind,
    /// Parameters for the action (action-specific JSON).
    pub params: Value,
}

impl IdeAction {
    /// Create a new action request.
    pub fn new(kind: ActionKind, params: Value) -> Self {
        Self { kind, params }
    }
}

/// Response from the IDE to an action request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeResponse {
    /// Whether the action succeeded.
    pub success: bool,
    /// Optional response data.
    pub data: Option<Value>,
}

impl IdeResponse {
    /// Create a successful response.
    pub fn ok(data: Value) -> Self {
        Self { success: true, data: Some(data) }
    }

    /// Create a successful response with no data.
    pub fn ok_empty() -> Self {
        Self { success: true, data: None }
    }

    /// Create a failure response.
    pub fn fail(message: &str) -> Self {
        Self {
            success: false,
            data: Some(Value::String(message.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Bridge
// ---------------------------------------------------------------------------

/// The main bridge between remote-code and the IDE.
///
/// Manages the connection state and provides methods for sending notifications
/// and requesting actions.
#[derive(Debug)]
pub struct IdeBridge {
    config: IdeConfig,
    connected: Arc<AtomicBool>,
}

impl IdeBridge {
    /// Create a new bridge with the given configuration.
    pub fn new(config: IdeConfig) -> Self {
        let connected = Arc::new(AtomicBool::new(false));
        Self { config, connected }
    }

    /// Return a reference to the bridge configuration.
    pub fn config(&self) -> &IdeConfig {
        &self.config
    }

    /// Simulate connecting to the IDE.
    pub fn connect(&self) -> anyhow::Result<()> {
        debug!(ide = %self.config.ide_type, "Connecting to IDE");
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Disconnect from the IDE.
    pub fn disconnect(&self) {
        debug!("Disconnecting from IDE");
        self.connected.store(false, Ordering::SeqCst);
    }

    /// Check whether the bridge is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Send a notification to the IDE.
    pub fn send_notification(&self, notification: &IdeNotification) -> anyhow::Result<()> {
        if !self.is_connected() {
            warn!("Attempted to send notification while disconnected");
            return Err(anyhow::anyhow!("Not connected to IDE"));
        }
        debug!(kind = ?notification.kind, msg = %notification.message, "Sent notification");
        Ok(())
    }

    /// Request an action from the IDE and return the response.
    pub fn request_action(&self, action: &IdeAction) -> anyhow::Result<IdeResponse> {
        if !self.is_connected() {
            warn!("Attempted to request action while disconnected");
            return Err(anyhow::anyhow!("Not connected to IDE"));
        }
        debug!(kind = ?action.kind, "Requested action");
        // In a real implementation this would send the request over the wire.
        Ok(IdeResponse::ok_empty())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConnectionMode, IdeType};

    fn test_config() -> IdeConfig {
        IdeConfig::new(IdeType::VsCode, ConnectionMode::Stdio)
    }

    #[test]
    fn bridge_new_is_disconnected() {
        let bridge = IdeBridge::new(test_config());
        assert!(!bridge.is_connected());
    }

    #[test]
    fn bridge_connect() {
        let bridge = IdeBridge::new(test_config());
        bridge.connect().expect("connect");
        assert!(bridge.is_connected());
    }

    #[test]
    fn bridge_disconnect() {
        let bridge = IdeBridge::new(test_config());
        bridge.connect().expect("connect");
        bridge.disconnect();
        assert!(!bridge.is_connected());
    }

    #[test]
    fn send_notification_when_connected() {
        let bridge = IdeBridge::new(test_config());
        bridge.connect().expect("connect");
        let n = IdeNotification::simple(NotificationKind::Info, "hello".into());
        bridge.send_notification(&n).expect("send");
    }

    #[test]
    fn send_notification_when_disconnected_fails() {
        let bridge = IdeBridge::new(test_config());
        let n = IdeNotification::simple(NotificationKind::Info, "hello".into());
        let result = bridge.send_notification(&n);
        assert!(result.is_err());
    }

    #[test]
    fn request_action_when_connected() {
        let bridge = IdeBridge::new(test_config());
        bridge.connect().expect("connect");
        let action = IdeAction::new(ActionKind::FileOpen, serde_json::json!({"path": "/tmp/a.rs"}));
        let response = bridge.request_action(&action).expect("request");
        assert!(response.success);
    }

    #[test]
    fn request_action_when_disconnected_fails() {
        let bridge = IdeBridge::new(test_config());
        let action = IdeAction::new(ActionKind::FileOpen, serde_json::json!({}));
        let result = bridge.request_action(&action);
        assert!(result.is_err());
    }

    #[test]
    fn notification_simple() {
        let n = IdeNotification::simple(NotificationKind::Warning, "careful!".into());
        assert_eq!(n.kind, NotificationKind::Warning);
        assert!(n.metadata.is_empty());
    }

    #[test]
    fn notification_with_metadata() {
        let mut meta = HashMap::new();
        meta.insert("file".to_string(), "/tmp/x.rs".to_string());
        let n = IdeNotification::with_metadata(NotificationKind::FileModified, "changed".into(), meta);
        assert_eq!(n.metadata.get("file").map(|s| s.as_str()), Some("/tmp/x.rs"));
    }

    #[test]
    fn response_ok() {
        let r = IdeResponse::ok(serde_json::json!("done"));
        assert!(r.success);
        assert!(r.data.is_some());
    }

    #[test]
    fn response_fail() {
        let r = IdeResponse::fail("oops");
        assert!(!r.success);
    }

    #[test]
    fn notification_serde_roundtrip() {
        let n = IdeNotification::simple(NotificationKind::Error, "boom".into());
        let json = serde_json::to_string(&n).expect("serialize");
        let back: IdeNotification = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.kind, NotificationKind::Error);
        assert_eq!(back.message, "boom");
    }

    #[test]
    fn action_serde_roundtrip() {
        let a = IdeAction::new(ActionKind::DiffApply, serde_json::json!({"diff": "+++ "}));
        let json = serde_json::to_string(&a).expect("serialize");
        let back: IdeAction = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.kind, ActionKind::DiffApply);
    }
}
