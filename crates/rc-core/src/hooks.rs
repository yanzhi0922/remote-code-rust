use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Expanded hook event catalog used by the v2 engine surface.
///
/// This supplements the legacy `rc_core::HookEvent` enum without changing its
/// existing wire contract, so current apps keep compiling while v2 systems can
/// depend on a broader event taxonomy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HookEventKind {
    SessionStart,
    SessionEnd,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Notification,
    UserPromptSubmit,
    AssistantMessageStart,
    AssistantMessageDelta,
    AssistantMessageStop,
    PermissionRequest,
    PermissionResolved,
    CompactStarted,
    CompactCompleted,
    AgentStarted,
    AgentCompleted,
    AgentFailed,
    McpConnectionOpened,
    McpConnectionClosed,
    BackgroundTaskStarted,
    BackgroundTaskCompleted,
    ReviewRequested,
    ReviewCompleted,
    MemoryLoaded,
    MemorySaved,
    StopHookSummary,
}

impl HookEventKind {
    /// Return the upstream-style event name used for prompts/logging.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::Notification => "Notification",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::AssistantMessageStart => "AssistantMessageStart",
            Self::AssistantMessageDelta => "AssistantMessageDelta",
            Self::AssistantMessageStop => "AssistantMessageStop",
            Self::PermissionRequest => "PermissionRequest",
            Self::PermissionResolved => "PermissionResolved",
            Self::CompactStarted => "CompactStarted",
            Self::CompactCompleted => "CompactCompleted",
            Self::AgentStarted => "AgentStarted",
            Self::AgentCompleted => "AgentCompleted",
            Self::AgentFailed => "AgentFailed",
            Self::McpConnectionOpened => "McpConnectionOpened",
            Self::McpConnectionClosed => "McpConnectionClosed",
            Self::BackgroundTaskStarted => "BackgroundTaskStarted",
            Self::BackgroundTaskCompleted => "BackgroundTaskCompleted",
            Self::ReviewRequested => "ReviewRequested",
            Self::ReviewCompleted => "ReviewCompleted",
            Self::MemoryLoaded => "MemoryLoaded",
            Self::MemorySaved => "MemorySaved",
            Self::StopHookSummary => "StopHookSummary",
        }
    }
}

/// Structured hook event envelope for future transcript/event-stream usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEventEnvelope {
    pub event: HookEventKind,
    #[serde(default)]
    pub payload: Value,
}

/// Decision returned by a structured hook handler.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HookDecision {
    Continue,
    Block,
    Retry,
}

/// Hook-specific structured output payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HookSpecificOutput {
    Message { text: String },
    PermissionRule { tool_name: String, rule: String },
    Context { summary: String },
}

/// Structured hook response used by future SDK/runtime integrations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookResponse {
    pub decision: HookDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub outputs: Vec<HookSpecificOutput>,
}

#[cfg(test)]
mod tests {
    use super::{HookDecision, HookEventEnvelope, HookEventKind, HookResponse, HookSpecificOutput};

    #[test]
    fn expanded_hook_event_catalog_uses_pascal_case_names() {
        assert_eq!(HookEventKind::CompactCompleted.as_str(), "CompactCompleted");
        assert_eq!(HookEventKind::MemorySaved.as_str(), "MemorySaved");
    }

    #[test]
    fn hook_event_envelope_round_trips() {
        let encoded = serde_json::to_string(&HookEventEnvelope {
            event: HookEventKind::PermissionResolved,
            payload: serde_json::json!({"allowed": true}),
        })
        .expect("hook event should serialize");
        let decoded: HookEventEnvelope =
            serde_json::from_str(&encoded).expect("hook event should deserialize");
        assert_eq!(decoded.event, HookEventKind::PermissionResolved);
        assert_eq!(decoded.payload["allowed"], true);
    }

    #[test]
    fn hook_response_preserves_decision_and_outputs() {
        let response = HookResponse {
            decision: HookDecision::Retry,
            reason: Some("need more context".to_owned()),
            outputs: vec![HookSpecificOutput::Message {
                text: "retrying".to_owned(),
            }],
        };

        let encoded = serde_json::to_string(&response).expect("response should serialize");
        let decoded: HookResponse =
            serde_json::from_str(&encoded).expect("response should deserialize");
        assert_eq!(decoded.decision, HookDecision::Retry);
        assert_eq!(decoded.outputs.len(), 1);
    }
}
