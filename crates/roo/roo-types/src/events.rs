//! Event type definitions.
//!
//! Derived from `packages/types/src/events.ts`.
//! Defines all RooCode event names and their payload types.

use serde::{Deserialize, Serialize};

use crate::message::{ClineMessage, QueuedMessage, TokenUsage};
use crate::tool::ToolUsage;
use crate::vscode_extension_host::Command;

// ---------------------------------------------------------------------------
// RooCodeEventName
// ---------------------------------------------------------------------------

/// All event names used in the Roo Code event system.
///
/// Source: `packages/types/src/events.ts` — `RooCodeEventName`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RooCodeEventName {
    // Task Provider Lifecycle
    TaskCreated,

    // Task Lifecycle
    TaskStarted,
    TaskCompleted,
    TaskAborted,
    TaskFocused,
    TaskUnfocused,
    TaskActive,
    TaskInteractive,
    TaskResumable,
    TaskIdle,

    // Subtask Lifecycle
    TaskPaused,
    TaskUnpaused,
    TaskSpawned,
    TaskDelegated,
    TaskDelegationCompleted,
    TaskDelegationResumed,

    // Task Execution
    Message,
    TaskModeSwitched,
    TaskAskResponded,
    TaskUserMessage,
    QueuedMessagesUpdated,

    // Task Analytics
    TaskTokenUsageUpdated,
    TaskToolFailed,

    // Configuration Changes
    ModeChanged,
    ProviderProfileChanged,

    // Query Responses
    CommandsResponse,
    ModesResponse,
    ModelsResponse,

    // Evals
    EvalPass,
    EvalFail,
}

// ---------------------------------------------------------------------------
// Event Payloads
// ---------------------------------------------------------------------------

/// Payload for TaskCompleted event.
///
/// TS: `z.tuple([z.string(), tokenUsageSchema, toolUsageSchema, z.object({ isSubtask: z.boolean() })])`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCompletedPayload {
    pub task_id: String,
    pub token_usage: TokenUsage,
    pub tool_usage: ToolUsage,
    pub is_subtask: bool,
}

/// Payload for TaskDelegated event.
///
/// TS: `z.tuple([z.string(), z.string()])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDelegatedPayload {
    pub parent_task_id: String,
    pub child_task_id: String,
}

/// Payload for TaskDelegationCompleted event.
///
/// TS: `z.tuple([z.string(), z.string(), z.string()])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDelegationCompletedPayload {
    pub parent_task_id: String,
    pub child_task_id: String,
    pub completion_result_summary: String,
}

/// Payload for TaskDelegationResumed event.
///
/// TS: `z.tuple([z.string(), z.string()])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDelegationResumedPayload {
    pub parent_task_id: String,
    pub child_task_id: String,
}

/// Action for Message event payload.
///
/// TS: `z.union([z.literal("created"), z.literal("updated")])`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageAction {
    Created,
    Updated,
}

/// Payload for Message event.
///
/// TS: `z.tuple([z.object({ taskId, action, message })])`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePayload {
    pub task_id: String,
    pub action: MessageAction,
    pub message: Option<ClineMessage>,
}

/// Payload for TaskModeSwitched event.
///
/// TS: `z.tuple([z.string(), z.string()])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskModeSwitchedPayload {
    pub task_id: String,
    pub mode: String,
}

/// Payload for TaskAskResponded event.
///
/// TS: `z.tuple([z.string()])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAskRespondedPayload {
    pub task_id: String,
}

/// Payload for TaskUserMessage event.
///
/// TS: `z.tuple([z.string()])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUserMessagePayload {
    pub task_id: String,
}

/// Payload for QueuedMessagesUpdated event.
///
/// TS: `z.tuple([z.string(), z.array(queuedMessageSchema)])`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedMessagesUpdatedPayload {
    pub task_id: String,
    pub queued_messages: Vec<QueuedMessage>,
}

/// Payload for TaskTokenUsageUpdated event.
///
/// TS: `z.tuple([z.string(), tokenUsageSchema, toolUsageSchema])`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskTokenUsageUpdatedPayload {
    pub task_id: String,
    pub token_usage: TokenUsage,
    pub tool_usage: ToolUsage,
}

/// Payload for TaskToolFailed event.
///
/// TS: `z.tuple([z.string(), toolNamesSchema, z.string()])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskToolFailedPayload {
    pub task_id: String,
    pub tool: String,
    pub error: String,
}

/// Payload for ModeChanged event.
///
/// TS: `z.tuple([z.string()])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeChangedPayload {
    pub task_id: String,
}

/// Payload for ProviderProfileChanged event.
///
/// TS: `z.tuple([z.object({ name: z.string(), provider: z.string() })])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfileChangedPayload {
    pub name: String,
    pub provider: String,
}

/// Payload for CommandsResponse event.
///
/// TS: `z.tuple([z.array({ name, source, filePath?, description?, argumentHint? })])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandsResponsePayload {
    pub commands: Vec<Command>,
}

/// A single mode entry for ModesResponse.
///
/// TS: `{ slug: z.string(), name: z.string() }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeEntry {
    pub slug: String,
    pub name: String,
}

/// Payload for ModesResponse event.
///
/// TS: `z.tuple([z.array({ slug: z.string(), name: z.string() })])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModesResponsePayload {
    pub modes: Vec<ModeEntry>,
}

/// Payload for ModelsResponse event.
///
/// TS: `z.tuple([z.record(z.string(), modelInfoSchema)])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponsePayload {
    pub models: serde_json::Value,
}

/// Payload for EvalPass / EvalFail events.
///
/// TS: `payload: z.undefined(), taskId: z.number()`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalPayload {
    pub task_id: u64,
}
