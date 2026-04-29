//! Bridge messaging types for desktop-IDE communication.
//!
//! Corresponds to `.research/cc-haha/src/bridge/` messaging patterns.
//! Defines the message types exchanged between remote-code and the IDE,
//! along with binary serialization/deserialization support.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Types of messages exchanged over the bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeMessageType {
    /// A message from the user.
    UserMessage,
    /// A response from the assistant.
    AssistantMessage,
    /// Result from a tool execution.
    ToolResult,
    /// Request for user permission to perform an action.
    PermissionRequest,
    /// Response to a permission request.
    PermissionResponse,
    /// Session state update notification.
    SessionUpdate,
    /// Connection status update.
    StatusUpdate,
}

impl std::fmt::Display for BridgeMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeMessageType::UserMessage => write!(f, "user_message"),
            BridgeMessageType::AssistantMessage => write!(f, "assistant_message"),
            BridgeMessageType::ToolResult => write!(f, "tool_result"),
            BridgeMessageType::PermissionRequest => write!(f, "permission_request"),
            BridgeMessageType::PermissionResponse => write!(f, "permission_response"),
            BridgeMessageType::SessionUpdate => write!(f, "session_update"),
            BridgeMessageType::StatusUpdate => write!(f, "status_update"),
        }
    }
}

/// A message exchanged over the bridge.
///
/// This is a higher-level message type than the payload-level [`BridgePayload`]
/// in [`crate::messaging`], encompassing the full range of interaction types
/// including user/assistant conversation, tool results, and permission flows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeMessage {
    /// A message from the user.
    UserMessage {
        /// Unique message identifier.
        id: String,
        /// The text content of the message.
        content: String,
    },
    /// A response from the assistant.
    AssistantMessage {
        /// Unique message identifier.
        id: String,
        /// The text content of the response.
        content: String,
        /// Whether the response is complete (streaming support).
        complete: bool,
    },
    /// Result from a tool execution.
    ToolResult {
        /// Unique result identifier.
        id: String,
        /// The tool call ID this result corresponds to.
        tool_call_id: String,
        /// The output of the tool execution.
        output: String,
        /// Whether the tool execution resulted in an error.
        is_error: bool,
    },
    /// Request for user permission to perform an action.
    PermissionRequest {
        /// Unique request identifier.
        id: String,
        /// Description of the action requiring permission.
        action: String,
        /// The tool or operation that needs permission.
        tool_name: String,
    },
    /// Response to a permission request.
    PermissionResponse {
        /// The permission request ID being responded to.
        request_id: String,
        /// Whether the permission was granted.
        granted: bool,
    },
    /// Session state update notification.
    SessionUpdate {
        /// Session identifier.
        session_id: String,
        /// The updated state as a JSON value.
        state: serde_json::Value,
    },
    /// Connection status update.
    StatusUpdate {
        /// The new status string.
        status: String,
        /// Optional detail about the status change.
        detail: Option<String>,
    },
}

impl BridgeMessage {
    /// Return the message type for this message.
    pub fn message_type(&self) -> BridgeMessageType {
        match self {
            BridgeMessage::UserMessage { .. } => BridgeMessageType::UserMessage,
            BridgeMessage::AssistantMessage { .. } => BridgeMessageType::AssistantMessage,
            BridgeMessage::ToolResult { .. } => BridgeMessageType::ToolResult,
            BridgeMessage::PermissionRequest { .. } => BridgeMessageType::PermissionRequest,
            BridgeMessage::PermissionResponse { .. } => BridgeMessageType::PermissionResponse,
            BridgeMessage::SessionUpdate { .. } => BridgeMessageType::SessionUpdate,
            BridgeMessage::StatusUpdate { .. } => BridgeMessageType::StatusUpdate,
        }
    }

    /// Return the message ID, if available.
    pub fn id(&self) -> Option<&str> {
        match self {
            BridgeMessage::UserMessage { id, .. }
            | BridgeMessage::AssistantMessage { id, .. }
            | BridgeMessage::ToolResult { id, .. }
            | BridgeMessage::PermissionRequest { id, .. } => Some(id.as_str()),
            BridgeMessage::PermissionResponse { request_id, .. } => Some(request_id.as_str()),
            BridgeMessage::SessionUpdate { .. } => None,
            BridgeMessage::StatusUpdate { .. } => None,
        }
    }

    /// Create a user message.
    pub fn user(id: String, content: String) -> Self {
        Self::UserMessage { id, content }
    }

    /// Create an assistant message.
    pub fn assistant(id: String, content: String, complete: bool) -> Self {
        Self::AssistantMessage {
            id,
            content,
            complete,
        }
    }

    /// Create a tool result.
    pub fn tool_result(id: String, tool_call_id: String, output: String, is_error: bool) -> Self {
        Self::ToolResult {
            id,
            tool_call_id,
            output,
            is_error,
        }
    }

    /// Create a permission request.
    pub fn permission_request(id: String, action: String, tool_name: String) -> Self {
        Self::PermissionRequest {
            id,
            action,
            tool_name,
        }
    }

    /// Create a permission response.
    pub fn permission_response(request_id: String, granted: bool) -> Self {
        Self::PermissionResponse {
            request_id,
            granted,
        }
    }

    /// Create a session update.
    pub fn session_update(session_id: String, state: serde_json::Value) -> Self {
        Self::SessionUpdate { session_id, state }
    }

    /// Create a status update.
    pub fn status_update(status: String, detail: Option<String>) -> Self {
        Self::StatusUpdate { status, detail }
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

/// Serialize a [`BridgeMessage`] into a byte vector using JSON encoding.
pub fn serialize_bridge_message(msg: &BridgeMessage) -> anyhow::Result<Vec<u8>> {
    let json = serde_json::to_vec(msg)?;
    Ok(json)
}

/// Deserialize a [`BridgeMessage`] from a byte slice.
pub fn deserialize_bridge_message(data: &[u8]) -> anyhow::Result<BridgeMessage> {
    let msg = serde_json::from_slice(data)?;
    Ok(msg)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_all_message_types() -> Vec<BridgeMessage> {
        vec![
            BridgeMessage::user("u1".into(), "hello".into()),
            BridgeMessage::assistant("a1".into(), "world".into(), true),
            BridgeMessage::tool_result("t1".into(), "tc-1".into(), "ok".into(), false),
            BridgeMessage::permission_request("p1".into(), "edit file".into(), "write_file".into()),
            BridgeMessage::permission_response("p1".into(), true),
            BridgeMessage::session_update("sess-1".into(), serde_json::json!({"key": "val"})),
            BridgeMessage::status_update("connected".into(), Some("ready".into())),
        ]
    }

    #[test]
    fn message_type_user() {
        let msg = BridgeMessage::user("u".into(), "hi".into());
        assert_eq!(msg.message_type(), BridgeMessageType::UserMessage);
    }

    #[test]
    fn message_type_assistant() {
        let msg = BridgeMessage::assistant("a".into(), "hey".into(), false);
        assert_eq!(msg.message_type(), BridgeMessageType::AssistantMessage);
    }

    #[test]
    fn message_type_tool_result() {
        let msg = BridgeMessage::tool_result("t".into(), "tc".into(), "out".into(), false);
        assert_eq!(msg.message_type(), BridgeMessageType::ToolResult);
    }

    #[test]
    fn message_type_permission_request() {
        let msg = BridgeMessage::permission_request("p".into(), "act".into(), "tool".into());
        assert_eq!(msg.message_type(), BridgeMessageType::PermissionRequest);
    }

    #[test]
    fn message_type_permission_response() {
        let msg = BridgeMessage::permission_response("p".into(), false);
        assert_eq!(msg.message_type(), BridgeMessageType::PermissionResponse);
    }

    #[test]
    fn message_type_session_update() {
        let msg = BridgeMessage::session_update("s".into(), serde_json::json!({}));
        assert_eq!(msg.message_type(), BridgeMessageType::SessionUpdate);
    }

    #[test]
    fn message_type_status_update() {
        let msg = BridgeMessage::status_update("ok".into(), None);
        assert_eq!(msg.message_type(), BridgeMessageType::StatusUpdate);
    }

    #[test]
    fn serialize_deserialize_roundtrip_all() {
        for msg in make_all_message_types() {
            let bytes = serialize_bridge_message(&msg).expect("serialize");
            let back = deserialize_bridge_message(&bytes).expect("deserialize");
            assert_eq!(back, msg);
        }
    }

    #[test]
    fn deserialize_invalid_data() {
        let result = deserialize_bridge_message(b"not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn message_id_extraction() {
        let msg = BridgeMessage::user("u-123".into(), "hi".into());
        assert_eq!(msg.id(), Some("u-123"));

        let msg = BridgeMessage::permission_response("pr-456".into(), true);
        assert_eq!(msg.id(), Some("pr-456"));

        let msg = BridgeMessage::status_update("ok".into(), None);
        assert_eq!(msg.id(), None);
    }

    #[test]
    fn tool_result_with_error() {
        let msg = BridgeMessage::tool_result(
            "t-err".into(),
            "tc-err".into(),
            "permission denied".into(),
            true,
        );
        if let BridgeMessage::ToolResult { is_error, .. } = msg {
            assert!(is_error);
        } else {
            panic!("expected ToolResult");
        }
    }

    #[test]
    fn assistant_streaming() {
        let msg = BridgeMessage::assistant("a-stream".into(), "partial...".into(), false);
        if let BridgeMessage::AssistantMessage { complete, .. } = &msg {
            assert!(!complete);
        } else {
            panic!("expected AssistantMessage");
        }
    }

    #[test]
    fn message_type_display() {
        assert_eq!(BridgeMessageType::UserMessage.to_string(), "user_message");
        assert_eq!(
            BridgeMessageType::AssistantMessage.to_string(),
            "assistant_message"
        );
        assert_eq!(BridgeMessageType::ToolResult.to_string(), "tool_result");
        assert_eq!(
            BridgeMessageType::PermissionRequest.to_string(),
            "permission_request"
        );
        assert_eq!(
            BridgeMessageType::PermissionResponse.to_string(),
            "permission_response"
        );
        assert_eq!(
            BridgeMessageType::SessionUpdate.to_string(),
            "session_update"
        );
        assert_eq!(BridgeMessageType::StatusUpdate.to_string(), "status_update");
    }
}
