//! IDE bridge — main interface between remote-code and the IDE.
//!
//! [`IdeBridge`] is the primary entry point for sending notifications and
//! requesting actions from the connected IDE.

use crate::config::IdeConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    pub fn with_metadata(
        kind: NotificationKind,
        message: String,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            kind,
            message,
            metadata,
        }
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
        Self {
            success: true,
            data: Some(data),
        }
    }

    /// Create a successful response with no data.
    pub fn ok_empty() -> Self {
        Self {
            success: true,
            data: None,
        }
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

/// JSON-RPC style request envelope for IDE communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: Option<u64>,
    method: String,
    params: Value,
}

/// JSON-RPC style response envelope from IDE.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcError {
    #[allow(dead_code)]
    code: i64,
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

/// The main bridge between remote-code and the IDE.
///
/// Manages the connection state and provides methods for sending notifications
/// and requesting actions. Uses a boxed [`IdeConnection`](crate::connection::IdeConnection)
/// trait object for actual I/O.
#[derive(Debug)]
pub struct IdeBridge {
    config: IdeConfig,
    connected: Arc<AtomicBool>,
    /// Pending responses from the IDE, keyed by request ID.
    pending_responses: Arc<std::sync::Mutex<HashMap<u64, IdeResponse>>>,
    /// Outgoing message queue — serialized JSON-RPC payloads waiting to be
    /// written to the transport by the caller.
    outgoing: Arc<std::sync::Mutex<Vec<String>>>,
    /// Next request ID for JSON-RPC.
    next_id: Arc<std::sync::atomic::AtomicU64>,
}

impl IdeBridge {
    /// Create a new bridge with the given configuration.
    pub fn new(config: IdeConfig) -> Self {
        let connected = Arc::new(AtomicBool::new(false));
        Self {
            config,
            connected,
            pending_responses: Arc::new(std::sync::Mutex::new(HashMap::new())),
            outgoing: Arc::new(std::sync::Mutex::new(Vec::new())),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Return a reference to the bridge configuration.
    pub fn config(&self) -> &IdeConfig {
        &self.config
    }

    /// Connect to the IDE.
    ///
    /// Establishes the underlying transport connection and performs
    /// any necessary handshake.
    pub fn connect(&self) -> anyhow::Result<()> {
        debug!(ide = %self.config.ide_type, "Connecting to IDE");
        // In production, this would initialize the IdeConnection transport.
        // For now, we mark as connected and the actual transport is managed
        // by the caller (e.g., the TUI or control plane).
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
    ///
    /// Serializes the notification as a JSON-RPC notification and enqueues it
    /// in the outgoing message buffer. Call [`Self::drain_outgoing`] to
    /// retrieve and flush the buffered messages to the transport.
    pub fn send_notification(&self, notification: &IdeNotification) -> anyhow::Result<()> {
        if !self.is_connected() {
            warn!("Attempted to send notification while disconnected");
            return Err(anyhow::anyhow!("Not connected to IDE"));
        }

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: None, // Notifications have no ID
            method: format!("notification/{:?}", notification.kind).to_lowercase(),
            params: serde_json::to_value(notification)?,
        };

        let payload = serde_json::to_string(&request)?;
        debug!(
            kind = ?notification.kind,
            msg = %notification.message,
            bytes = payload.len(),
            "Queued notification"
        );

        if let Ok(mut queue) = self.outgoing.lock() {
            queue.push(payload);
        }
        Ok(())
    }

    /// Send a raw notification payload to the IDE.
    ///
    /// Returns the serialized JSON-RPC envelope as a string, which the caller
    /// can write to any transport.
    pub fn serialize_notification(&self, notification: &IdeNotification) -> anyhow::Result<String> {
        if !self.is_connected() {
            return Err(anyhow::anyhow!("Not connected to IDE"));
        }

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: None,
            method: format!("notification/{:?}", notification.kind).to_lowercase(),
            params: serde_json::to_value(notification)?,
        };

        Ok(serde_json::to_string(&request)?)
    }

    /// Request an action from the IDE and return the response.
    ///
    /// Serializes the action as a JSON-RPC request, assigns a unique ID,
    /// enqueues the payload, and checks for a previously stored response
    /// with the matching ID (set via [`Self::handle_response`]).
    /// If no response is available yet, returns a pending response
    /// (`success: false` with a descriptive message) so callers can
    /// distinguish "no response yet" from a genuine success.
    pub fn request_action(&self, action: &IdeAction) -> anyhow::Result<IdeResponse> {
        if !self.is_connected() {
            warn!("Attempted to request action while disconnected");
            return Err(anyhow::anyhow!("Not connected to IDE"));
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: Some(id),
            method: format!("action/{:?}", action.kind).to_lowercase(),
            params: action.params.clone(),
        };

        let payload = serde_json::to_string(&request)?;
        debug!(
            kind = ?action.kind,
            id = id,
            bytes = payload.len(),
            "Queued action request"
        );

        // Enqueue the serialized request for the transport layer.
        if let Ok(mut queue) = self.outgoing.lock() {
            queue.push(payload);
        }

        // Check if a response was already received for this request ID.
        if let Ok(mut pending) = self.pending_responses.lock()
            && let Some(response) = pending.remove(&id)
        {
            return Ok(response);
        }

        // No response yet — return a pending indicator so callers can
        // distinguish "awaiting response" from a genuine empty success.
        // Use `take_response(id)` or `handle_response` to collect the
        // actual reply when the transport delivers it.
        Ok(IdeResponse::fail(
            "response pending — not yet received from IDE",
        ))
    }

    /// Serialize an action request and return the JSON-RPC envelope.
    ///
    /// The caller is responsible for writing this to the transport and
    /// reading the response.
    pub fn serialize_action_request(&self, action: &IdeAction) -> anyhow::Result<(u64, String)> {
        if !self.is_connected() {
            return Err(anyhow::anyhow!("Not connected to IDE"));
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: Some(id),
            method: format!("action/{:?}", action.kind).to_lowercase(),
            params: action.params.clone(),
        };

        Ok((id, serde_json::to_string(&request)?))
    }

    /// Process a raw response from the IDE transport.
    ///
    /// Parses the JSON-RPC response and stores it for the matching request.
    pub fn handle_response(&self, raw: &str) -> anyhow::Result<()> {
        let response: JsonRpcResponse = serde_json::from_str(raw)?;

        if let Some(error) = &response.error {
            warn!(
                code = error.code,
                message = %error.message,
                "IDE returned error response"
            );
        }

        // Store the response for the matching request ID.
        if let Ok(mut pending) = self.pending_responses.lock()
            && let Some(id) = response.id
        {
            let ide_response = if let Some(error) = response.error {
                IdeResponse::fail(&error.message)
            } else {
                match response.result {
                    Some(val) => IdeResponse::ok(val),
                    None => IdeResponse::ok_empty(),
                }
            };
            pending.insert(id, ide_response);
        }

        Ok(())
    }

    /// Drain all queued outgoing messages.
    ///
    /// Returns the serialized JSON-RPC payloads that were enqueued by
    /// [`Self::send_notification`] and [`Self::request_action`], clearing
    /// the internal buffer. The caller should write each payload to the
    /// transport (e.g., via LSP-style Content-Length framing over stdio).
    pub fn drain_outgoing(&self) -> Vec<String> {
        match self.outgoing.lock() {
            Ok(mut queue) => std::mem::take(&mut *queue),
            Err(_) => Vec::new(),
        }
    }

    /// Take the stored response for a specific request ID, if any.
    ///
    /// Returns `Some(response)` if a response was previously received via
    /// [`Self::handle_response`] for the given ID, removing it from the
    /// pending map. Returns `None` if no response is available yet.
    pub fn take_response(&self, id: u64) -> Option<IdeResponse> {
        self.pending_responses
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&id))
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
    fn request_action_when_connected_returns_pending() {
        let bridge = IdeBridge::new(test_config());
        bridge.connect().expect("connect");
        let action = IdeAction::new(
            ActionKind::FileOpen,
            serde_json::json!({"path": "/tmp/a.rs"}),
        );
        let response = bridge.request_action(&action).expect("request");
        // Without a pre-stored response, the bridge returns a pending indicator.
        assert!(!response.success);
        assert!(
            response
                .data
                .as_ref()
                .unwrap()
                .as_str()
                .unwrap()
                .contains("pending")
        );
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
        let n =
            IdeNotification::with_metadata(NotificationKind::FileModified, "changed".into(), meta);
        assert_eq!(
            n.metadata.get("file").map(|s| s.as_str()),
            Some("/tmp/x.rs")
        );
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
