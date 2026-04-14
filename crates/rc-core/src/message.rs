use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{Attachment, ConversationEntry, ConversationRole, ToolCall};

/// Provenance of a v2 runtime message.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageOrigin {
    UserInput,
    Provider,
    Tool,
    System,
    Hook,
    Compact,
    Agent,
    Replay,
}

/// Metadata shared by every v2 runtime message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageBase {
    pub uuid: Uuid,
    pub parent_uuid: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub is_meta: bool,
    #[serde(default)]
    pub is_virtual: bool,
    #[serde(default)]
    pub is_compact_summary: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<MessageOrigin>,
}

impl Default for MessageBase {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            parent_uuid: None,
            timestamp: Utc::now(),
            is_meta: false,
            is_virtual: false,
            is_compact_summary: false,
            origin: None,
        }
    }
}

impl MessageBase {
    /// Create a base with the supplied origin.
    #[must_use]
    pub fn with_origin(origin: MessageOrigin) -> Self {
        Self {
            origin: Some(origin),
            ..Self::default()
        }
    }
}

/// Assistant content blocks aligned with Claude Code's richer message model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantContentBlock {
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    Text {
        text: String,
    },
    RedactedThinking {
        data: String,
    },
    Thinking {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    AdvisorToolResult {
        content: String,
    },
}

/// User-originated message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub base: MessageBase,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

/// Assistant-originated message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub base: MessageBase,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub blocks: Vec<AssistantContentBlock>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

/// Progress message emitted while work is underway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMessage {
    pub base: MessageBase,
    pub stage: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
}

/// System-message subtype aligned with the parity plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemMessageSubtype {
    LocalCommand,
    BridgeStatus,
    TurnDuration,
    Thinking,
    MemorySaved,
    StopHookSummary,
    Informational,
    CompactBoundary,
    MicrocompactBoundary,
    PermissionRetry,
    ScheduledTaskFire,
    AwaySummary,
    AgentsKilled,
    ApiMetrics,
    ApiError,
    FileSnapshot,
}

/// System-originated message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMessage {
    pub base: MessageBase,
    pub subtype: SystemMessageSubtype,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Attachment-only helper message for UI/event streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMessage {
    pub base: MessageBase,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

/// Result of a hook execution rendered into the transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResultMessage {
    pub base: MessageBase,
    pub hook_name: String,
    pub output: String,
    #[serde(default)]
    pub is_error: bool,
}

/// Summary of a tool invocation/result pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseSummaryMessage {
    pub base: MessageBase,
    pub tool_call_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub is_error: bool,
}

/// Placeholder/tombstone marker used during streaming recovery or compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TombstoneMessage {
    pub base: MessageBase,
    #[serde(default)]
    pub replaced_message_ids: Vec<Uuid>,
    pub summary: String,
}

/// Grouped rendering of multiple tool uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedToolUseMessage {
    pub base: MessageBase,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub summary: Option<String>,
}

/// Collapsed read/search results preserved as a compact summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollapsedReadSearchMessage {
    pub base: MessageBase,
    pub summary: String,
    #[serde(default)]
    pub items: Vec<String>,
}

/// Unified runtime message union for the v2 engine surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    Progress(ProgressMessage),
    System(SystemMessage),
    Attachment(AttachmentMessage),
    HookResult(HookResultMessage),
    ToolUseSummary(ToolUseSummaryMessage),
    Tombstone(TombstoneMessage),
    GroupedToolUse(GroupedToolUseMessage),
    CollapsedReadSearch(CollapsedReadSearchMessage),
}

impl Message {
    /// Borrow the message base metadata.
    #[must_use]
    pub fn base(&self) -> &MessageBase {
        match self {
            Self::User(message) => &message.base,
            Self::Assistant(message) => &message.base,
            Self::Progress(message) => &message.base,
            Self::System(message) => &message.base,
            Self::Attachment(message) => &message.base,
            Self::HookResult(message) => &message.base,
            Self::ToolUseSummary(message) => &message.base,
            Self::Tombstone(message) => &message.base,
            Self::GroupedToolUse(message) => &message.base,
            Self::CollapsedReadSearch(message) => &message.base,
        }
    }

    /// Return the primary UUID for the message.
    #[must_use]
    pub fn uuid(&self) -> Uuid {
        self.base().uuid
    }

    /// Convert the message back into the legacy conversation format when possible.
    #[must_use]
    pub fn as_conversation_entry(&self) -> Option<ConversationEntry> {
        match self {
            Self::User(message) => Some(ConversationEntry {
                role: ConversationRole::User,
                text: message.text.clone(),
                history_text: None,
                content_blocks: Vec::new(),
                tool_calls: Vec::new(),
                attachments: message.attachments.clone(),
                tool_call_id: None,
                name: None,
                is_error: false,
            }),
            Self::Assistant(message) => Some(ConversationEntry {
                role: ConversationRole::Assistant,
                text: message.text.clone(),
                history_text: None,
                content_blocks: Vec::new(),
                tool_calls: message.tool_calls.clone(),
                attachments: Vec::new(),
                tool_call_id: None,
                name: None,
                is_error: false,
            }),
            Self::System(message) => Some(ConversationEntry {
                role: ConversationRole::System,
                text: message.text.clone(),
                history_text: None,
                content_blocks: Vec::new(),
                tool_calls: Vec::new(),
                attachments: Vec::new(),
                tool_call_id: None,
                name: None,
                is_error: matches!(message.subtype, SystemMessageSubtype::ApiError),
            }),
            Self::ToolUseSummary(message) => Some(ConversationEntry::tool(
                message.tool_call_id.clone(),
                message.tool_name.clone(),
                message.summary.clone(),
                message.is_error,
            )),
            Self::Attachment(message) => Some(ConversationEntry::user_with_attachments(
                message.label.clone().unwrap_or_default(),
                message.attachments.clone(),
            )),
            Self::Progress(_)
            | Self::HookResult(_)
            | Self::Tombstone(_)
            | Self::GroupedToolUse(_)
            | Self::CollapsedReadSearch(_) => None,
        }
    }
}

impl From<ConversationEntry> for Message {
    fn from(value: ConversationEntry) -> Self {
        match value.role {
            ConversationRole::System => Self::System(SystemMessage {
                base: MessageBase::with_origin(MessageOrigin::System),
                subtype: SystemMessageSubtype::Informational,
                text: value.text,
                error: value.is_error.then_some("system_error".to_owned()),
            }),
            ConversationRole::User => Self::User(UserMessage {
                base: MessageBase::with_origin(MessageOrigin::UserInput),
                text: value.text,
                attachments: value.attachments,
            }),
            ConversationRole::Assistant => Self::Assistant(AssistantMessage {
                base: MessageBase::with_origin(MessageOrigin::Provider),
                text: value.text,
                blocks: Vec::new(),
                tool_calls: value.tool_calls,
            }),
            ConversationRole::Tool => Self::ToolUseSummary(ToolUseSummaryMessage {
                base: MessageBase::with_origin(MessageOrigin::Tool),
                tool_call_id: value
                    .tool_call_id
                    .unwrap_or_else(|| "unknown-tool-call".to_owned()),
                tool_name: value.name.unwrap_or_else(|| "unknown".to_owned()),
                summary: value.text,
                is_error: value.is_error,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Message, MessageOrigin, SystemMessageSubtype};
    use crate::ConversationEntry;

    #[test]
    fn user_conversation_entry_round_trips_via_message() {
        let entry = ConversationEntry::user("ship it");
        let message = Message::from(entry.clone());
        let restored = message
            .as_conversation_entry()
            .expect("user message should down-convert");
        assert_eq!(restored.text, entry.text);
        assert_eq!(restored.role, entry.role);
    }

    #[test]
    fn assistant_tool_entry_becomes_tool_summary_message() {
        let message = Message::from(ConversationEntry::tool("tool-1", "bash", "ok", false));
        match message {
            Message::ToolUseSummary(summary) => {
                assert_eq!(summary.tool_call_id, "tool-1");
                assert_eq!(summary.tool_name, "bash");
                assert_eq!(summary.summary, "ok");
                assert_eq!(summary.base.origin, Some(MessageOrigin::Tool));
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn system_messages_mark_api_errors() {
        let message = Message::System(super::SystemMessage {
            base: super::MessageBase::with_origin(MessageOrigin::System),
            subtype: SystemMessageSubtype::ApiError,
            text: "api failed".to_owned(),
            error: Some("bad request".to_owned()),
        });
        let entry = message
            .as_conversation_entry()
            .expect("system message should down-convert");
        assert!(entry.is_error);
    }
}
