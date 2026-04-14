//! Shared runtime event contract for engine, runner, headless, and remote surfaces.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Role associated with a runtime message event.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    Assistant,
    User,
    System,
}

/// Presence state emitted by a local runtime daemon.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonPresenceState {
    Online,
    Offline,
    Reconnecting,
}

/// Shared runtime event payload used by remote transport surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeEventDetail {
    MessageDelta {
        role: MessageRole,
        delta: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    MessageCommitted {
        role: MessageRole,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    ToolStarted {
        tool_call_id: String,
        tool_name: String,
    },
    ToolProgress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delta: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_time_seconds: Option<u64>,
    },
    ToolFinished {
        tool_call_id: String,
        tool_name: String,
        is_error: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    ArtifactManifest {
        artifact_ids: Vec<Uuid>,
    },
    RuntimeError {
        message: String,
    },
    DaemonPresenceChanged {
        state: DaemonPresenceState,
    },
}

impl RuntimeEventDetail {
    /// Return the stable snake_case kind name used by control-plane APIs.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::MessageDelta { .. } => "message_delta",
            Self::MessageCommitted { .. } => "message_committed",
            Self::ToolStarted { .. } => "tool_started",
            Self::ToolProgress { .. } => "tool_progress",
            Self::ToolFinished { .. } => "tool_finished",
            Self::ArtifactManifest { .. } => "artifact_manifest",
            Self::RuntimeError { .. } => "runtime_error",
            Self::DaemonPresenceChanged { .. } => "daemon_presence_changed",
        }
    }
}

/// API request envelope used when publishing runtime events to the control plane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeEventCreateRequest {
    pub detail: RuntimeEventDetail,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_event_serialization_matches_control_plane_shape() {
        let detail = RuntimeEventDetail::ToolProgress {
            tool_call_id: Some("tool-1".to_owned()),
            tool_name: Some("bash".to_owned()),
            delta: Some("{\"command\":\"ls\"}".to_owned()),
            elapsed_time_seconds: Some(2),
        };

        let value = serde_json::to_value(&detail).expect("detail should serialize");
        assert_eq!(
            value,
            json!({
                "kind": "tool_progress",
                "tool_call_id": "tool-1",
                "tool_name": "bash",
                "delta": "{\"command\":\"ls\"}",
                "elapsed_time_seconds": 2
            })
        );
    }

    #[test]
    fn runtime_event_omits_optional_fields_when_absent() {
        let detail = RuntimeEventDetail::MessageCommitted {
            role: MessageRole::Assistant,
            text: "done".to_owned(),
            message_id: None,
        };

        let value = serde_json::to_value(&detail).expect("detail should serialize");
        assert_eq!(
            value,
            json!({
                "kind": "message_committed",
                "role": "assistant",
                "text": "done"
            })
        );
    }

    #[test]
    fn runtime_event_request_round_trips() {
        let request = RuntimeEventCreateRequest {
            detail: RuntimeEventDetail::DaemonPresenceChanged {
                state: DaemonPresenceState::Reconnecting,
            },
        };

        let encoded = serde_json::to_string(&request).expect("request should serialize");
        let decoded: RuntimeEventCreateRequest =
            serde_json::from_str(&encoded).expect("request should deserialize");
        assert_eq!(decoded, request);
    }
}
