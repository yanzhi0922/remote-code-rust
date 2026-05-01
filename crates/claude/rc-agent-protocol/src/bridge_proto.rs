//! Bridge protocol definitions for the Roo-code JSON-RPC adapter layer.
//!
//! This module defines the JSON-RPC method names, notification names, and
//! parameter types used by the [`SubprocessAdapter`](crate::adapters::SubprocessAdapter)
//! so the wire protocol stays in sync.

use serde::{Deserialize, Serialize};

// ===========================================================================
// Request method names  (host → bridge)
// ===========================================================================

/// Initialize a bridge session.
pub const METHOD_INITIALIZE: &str = "initialize";

/// Send a user message to the underlying agent.
pub const METHOD_SEND_MESSAGE: &str = "send_message";

/// Cancel the currently running agent task.
pub const METHOD_CANCEL: &str = "cancel";

/// Shut down the bridge process gracefully.
pub const METHOD_SHUTDOWN: &str = "shutdown";

/// Resolve a pending permission request.
pub const METHOD_RESOLVE_PERMISSION: &str = "resolve_permission";

// ===========================================================================
// Notification method names  (bridge → host)
// ===========================================================================

/// Bridge has started and is initialising.
pub const NOTIFY_STARTED: &str = "started";

/// Bridge is ready to accept requests.
pub const NOTIFY_READY: &str = "ready";

/// Streaming text delta from the agent.
pub const NOTIFY_MESSAGE_DELTA: &str = "message_delta";

/// A tool invocation has begun.
pub const NOTIFY_TOOL_CALL_STARTED: &str = "tool_call_started";

/// Progress update for an ongoing tool call.
pub const NOTIFY_TOOL_CALL_PROGRESS: &str = "tool_call_progress";

/// A tool invocation has finished.
pub const NOTIFY_TOOL_CALL_COMPLETED: &str = "tool_call_completed";

/// Agent is requesting permission for an operation.
pub const NOTIFY_PERMISSION_REQUEST: &str = "permission_request";

/// Context window usage report.
pub const NOTIFY_CONTEXT_USAGE: &str = "context_usage";

/// Agent has completed its current task.
pub const NOTIFY_DONE: &str = "done";

/// An error occurred inside the bridge / agent.
pub const NOTIFY_ERROR: &str = "error";

/// A subtask has been spawned.
pub const NOTIFY_SUBTASK_STARTED: &str = "subtask_started";

/// Progress update for a subtask.
pub const NOTIFY_SUBTASK_PROGRESS: &str = "subtask_progress";

/// A subtask has completed.
pub const NOTIFY_SUBTASK_COMPLETED: &str = "subtask_completed";

/// Context window overflow detected.
pub const NOTIFY_CONTEXT_OVERFLOW: &str = "context_overflow";

/// Context has been compacted to free up space.
pub const NOTIFY_CONTEXT_COMPACTED: &str = "context_compacted";

/// Agent / bridge has been stopped.
pub const NOTIFY_STOPPED: &str = "stopped";

// ===========================================================================
// Parameter types
// ===========================================================================

/// Parameters for the `initialize` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    /// Unique session identifier assigned by the host.
    pub session_id: String,
    /// Working directory for the agent process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    /// Model identifier to use (agent-specific).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// API key for the underlying LLM provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Parameters for the `send_message` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageParams {
    /// The user message / prompt to send.
    pub message: String,
    /// Optional session identifier (may be set during `initialize`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Parameters for the `cancel` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelParams {
    /// Session to cancel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Parameters for the `resolve_permission` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvePermissionParams {
    /// Session that owns the permission request.
    pub session_id: String,
    /// Identifier of the pending permission request.
    pub request_id: String,
    /// The user's decision.
    pub decision: String,
}

/// Parameters for the `shutdown` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownParams {}

/// Helper function for `#[serde(default = "...")]` on `serde_json::Value` fields
/// that should default to `Null`.
fn json_null() -> serde_json::Value {
    serde_json::Value::Null
}

/// Parameters for the `message_delta` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaParams {
    /// Session identifier.
    pub session_id: String,
    /// Text delta.
    pub delta: String,
}

/// Parameters for the `tool_call_started` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallStartedParams {
    /// Session identifier.
    pub session_id: String,
    /// Name of the tool being invoked.
    pub tool_name: String,
    /// Tool input parameters.
    #[serde(default = "json_null")]
    pub tool_input: serde_json::Value,
}

/// Parameters for the `tool_call_progress` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallProgressParams {
    /// Session identifier.
    pub session_id: String,
    /// Name of the tool.
    pub tool_name: String,
    /// Human-readable progress description.
    pub progress: String,
}

/// Parameters for the `tool_call_completed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallCompletedParams {
    /// Session identifier.
    pub session_id: String,
    /// Name of the tool.
    pub tool_name: String,
    /// Tool output.
    #[serde(default = "json_null")]
    pub result: serde_json::Value,
}

/// Parameters for the `permission_request` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequestParams {
    /// Session identifier.
    pub session_id: String,
    /// Unique identifier for this permission request.
    pub request_id: String,
    /// Name of the tool requesting permission.
    pub tool_name: String,
    /// Tool input that requires approval.
    #[serde(default = "json_null")]
    pub input: serde_json::Value,
}

/// Parameters for the `done` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneParams {
    /// Session identifier.
    pub session_id: String,
    /// The final result payload.
    pub result: serde_json::Value,
}

/// Parameters for the `error` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorParams {
    /// Session identifier (may be empty if the error is pre-session).
    #[serde(default)]
    pub session_id: String,
    /// Human-readable error message.
    pub message: String,
    /// Whether the error is recoverable.
    #[serde(default)]
    pub recoverable: bool,
}

/// Parameters for the `context_usage` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextUsageParams {
    /// Session identifier.
    pub session_id: String,
    /// Tokens used so far.
    pub used: u64,
    /// Total context window size.
    pub total: u64,
}

/// Parameters for the `subtask_started` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskStartedParams {
    /// Session identifier.
    pub session_id: String,
    /// Unique task identifier.
    pub task_id: String,
    /// Human-readable description.
    pub description: String,
}

/// Parameters for the `subtask_progress` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskProgressParams {
    /// Session identifier.
    pub session_id: String,
    /// Unique task identifier.
    pub task_id: String,
    /// Progress description.
    pub progress: String,
}

/// Parameters for the `subtask_completed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskCompletedParams {
    /// Session identifier.
    pub session_id: String,
    /// Unique task identifier.
    pub task_id: String,
    /// Task result.
    #[serde(default = "json_null")]
    pub result: serde_json::Value,
}

/// Parameters for the `context_overflow` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextOverflowParams {
    /// Session identifier.
    pub session_id: String,
    /// Tokens used at the time of overflow.
    #[serde(default)]
    pub used: u64,
    /// Maximum context window size.
    #[serde(default)]
    pub total: u64,
}

/// Parameters for the `context_compacted` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCompactedParams {
    /// Session identifier.
    pub session_id: String,
    /// Number of entries removed during compaction.
    #[serde(default)]
    pub entries_removed: u64,
    /// Usage ratio after compaction.
    #[serde(default)]
    pub usage_ratio: f64,
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_constants_are_consistent() {
        // Ensure none of the method constants collide.
        let methods = [
            METHOD_INITIALIZE,
            METHOD_SEND_MESSAGE,
            METHOD_CANCEL,
            METHOD_SHUTDOWN,
            METHOD_RESOLVE_PERMISSION,
        ];
        for (i, a) in methods.iter().enumerate() {
            for (j, b) in methods.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "duplicate method constant");
                }
            }
        }
    }

    #[test]
    fn notify_constants_are_consistent() {
        let notifs = [
            NOTIFY_STARTED,
            NOTIFY_READY,
            NOTIFY_MESSAGE_DELTA,
            NOTIFY_TOOL_CALL_STARTED,
            NOTIFY_TOOL_CALL_PROGRESS,
            NOTIFY_TOOL_CALL_COMPLETED,
            NOTIFY_PERMISSION_REQUEST,
            NOTIFY_CONTEXT_USAGE,
            NOTIFY_DONE,
            NOTIFY_ERROR,
            NOTIFY_SUBTASK_STARTED,
            NOTIFY_SUBTASK_PROGRESS,
            NOTIFY_SUBTASK_COMPLETED,
            NOTIFY_CONTEXT_OVERFLOW,
            NOTIFY_CONTEXT_COMPACTED,
            NOTIFY_STOPPED,
        ];
        for (i, a) in notifs.iter().enumerate() {
            for (j, b) in notifs.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "duplicate notify constant");
                }
            }
        }
    }

    #[test]
    fn initialize_params_roundtrip() {
        let params = InitializeParams {
            session_id: "sess-1".into(),
            working_dir: Some("/tmp".into()),
            model: Some("gpt-4".into()),
            api_key: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        let back: InitializeParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "sess-1");
        assert_eq!(back.working_dir.unwrap(), "/tmp");
        assert!(back.api_key.is_none());
    }

    #[test]
    fn send_message_params_roundtrip() {
        let params = SendMessageParams {
            message: "Hello".into(),
            session_id: Some("sess-1".into()),
        };
        let json = serde_json::to_string(&params).unwrap();
        let back: SendMessageParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back.message, "Hello");
    }
}
