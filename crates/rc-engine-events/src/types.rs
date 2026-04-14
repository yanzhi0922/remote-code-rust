use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub type SessionId = Uuid;
pub type AgentId = String;

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
    #[must_use]
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

/// Token accounting emitted by providers, compactors, and the query engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}

/// Streaming content block types mirrored from provider protocols.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentBlockType {
    ToolUse,
    ServerToolUse,
    Text,
    Thinking,
}

/// Incremental updates emitted while a provider stream is in flight.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    InputJsonDelta { partial_json: String },
    TextDelta { text: String },
    SignatureDelta { signature: String },
    ThinkingDelta { thinking: String },
}

/// Structured progress data for long-running tool calls.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolProgress {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_time_seconds: Option<u64>,
}

/// Canonical tool completion payload for the engine event layer.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResult {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Structured tool failure payload for the engine event layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolError {
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

/// Summary emitted when context compaction completes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompactionResult {
    pub strategy: String,
    pub before_messages: usize,
    pub after_messages: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Lightweight engine state surface for observers such as TUI and bridges.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineStateSnapshot {
    #[serde(default)]
    pub turn: u32,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub tool_call_count: usize,
    #[serde(default)]
    pub usage: Usage,
}

/// Unified engine event stream used by Query Engine V2 and downstream consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EngineEvent {
    QueryStarted {
        session_id: SessionId,
    },
    QueryCompleted {
        session_id: SessionId,
        duration_ms: u64,
    },
    QueryAborted {
        session_id: SessionId,
    },
    StreamStarted {
        request_id: String,
    },
    StreamMessageStart {
        model: String,
        usage: Usage,
    },
    StreamContentBlockStart {
        index: usize,
        block_type: ContentBlockType,
    },
    StreamContentBlockDelta {
        index: usize,
        delta: ContentBlockDelta,
    },
    StreamContentBlockStop {
        index: usize,
    },
    StreamMessageDelta {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
        usage: Usage,
    },
    StreamMessageStop,
    StreamError {
        error: String,
    },
    ToolUseStarted {
        tool_use_id: String,
        tool_name: String,
        input: Value,
    },
    ToolUseProgress {
        tool_use_id: String,
        progress: ToolProgress,
    },
    ToolUseCompleted {
        tool_use_id: String,
        result: ToolResult,
    },
    ToolUseError {
        tool_use_id: String,
        error: ToolError,
    },
    ToolUseRejected {
        tool_use_id: String,
        reason: String,
    },
    CompactStarted {
        strategy: String,
    },
    CompactProgress {
        status: String,
    },
    CompactCompleted {
        result: CompactionResult,
    },
    AgentStarted {
        agent_id: AgentId,
        agent_type: String,
    },
    AgentCompleted {
        agent_id: AgentId,
    },
    AgentFailed {
        agent_id: AgentId,
        error: String,
    },
    StateUpdated {
        state_snapshot: EngineStateSnapshot,
    },
    CostUpdated {
        total_cost_usd: f64,
    },
    UsageUpdated {
        usage: Usage,
    },
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

    #[test]
    fn engine_event_serializes_with_nested_content_delta() {
        let event = EngineEvent::StreamContentBlockDelta {
            index: 1,
            delta: ContentBlockDelta::TextDelta {
                text: "hello".to_owned(),
            },
        };

        let value = serde_json::to_value(&event).expect("event should serialize");
        assert_eq!(
            value,
            json!({
                "kind": "stream_content_block_delta",
                "index": 1,
                "delta": {
                    "kind": "text_delta",
                    "text": "hello"
                }
            })
        );
    }

    #[test]
    fn engine_event_round_trips_with_tool_payloads() {
        let event = EngineEvent::ToolUseStarted {
            tool_use_id: "toolu_123".to_owned(),
            tool_name: "read_file".to_owned(),
            input: json!({ "path": "src/lib.rs" }),
        };

        let encoded = serde_json::to_string(&event).expect("event should serialize");
        let decoded: EngineEvent =
            serde_json::from_str(&encoded).expect("event should deserialize");
        assert_eq!(decoded, event);
    }

    #[test]
    fn runtime_event_kind_remains_stable() {
        let event = RuntimeEventDetail::RuntimeError {
            message: "boom".to_owned(),
        };
        assert_eq!(event.kind(), "runtime_error");
    }
}
