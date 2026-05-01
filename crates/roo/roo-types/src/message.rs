//! Message type definitions.
//!
//! Derived from `packages/types/src/message.ts`.
//! Defines ClineAsk, ClineSay, ClineMessage, TokenUsage, and related types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ClineAsk — types of questions the assistant can ask
// ---------------------------------------------------------------------------

/// All possible ask types that the LLM can use to request user interaction or approval.
///
/// Source: `packages/types/src/message.ts` — `clineAsks`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClineAsk {
    /// LLM asks a clarifying question.
    Followup,
    /// Permission to execute a terminal/shell command.
    Command,
    /// Permission to read output from a previously executed command.
    CommandOutput,
    /// Task has been completed, awaiting user feedback.
    CompletionResult,
    /// Permission to use a tool for file operations.
    Tool,
    /// API request failed, asking user whether to retry.
    ApiReqFailed,
    /// Confirmation needed to resume a previously paused task.
    ResumeTask,
    /// Confirmation needed to resume a task that was already completed.
    ResumeCompletedTask,
    /// Too many errors encountered, needs user guidance.
    MistakeLimitReached,
    /// Permission to use MCP server functionality.
    UseMcpServer,
    /// Auto-approval limit reached, manual approval required.
    AutoApprovalMaxReqReached,
}

impl ClineAsk {
    /// Asks that put the task into an "idle" state.
    pub fn is_idle(&self) -> bool {
        matches!(
            self,
            Self::CompletionResult
                | Self::ApiReqFailed
                | Self::ResumeCompletedTask
                | Self::MistakeLimitReached
                | Self::AutoApprovalMaxReqReached
        )
    }

    /// Asks that put the task into a "resumable" state.
    pub fn is_resumable(&self) -> bool {
        matches!(self, Self::ResumeTask)
    }

    /// Asks that require user interaction.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Self::Followup | Self::Command | Self::Tool | Self::UseMcpServer
        )
    }

    /// Asks that are not associated with an actual approval.
    pub fn is_non_blocking(&self) -> bool {
        matches!(self, Self::CommandOutput)
    }
}

// ---------------------------------------------------------------------------
// ClineSay — types of informational messages from the assistant
// ---------------------------------------------------------------------------

/// All possible say types that represent different kinds of messages.
///
/// Source: `packages/types/src/message.ts` — `clineSays`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClineSay {
    /// General error message.
    Error,
    /// API request has been initiated.
    ApiReqStarted,
    /// API request has completed successfully.
    ApiReqFinished,
    /// API request is being retried after a failure.
    ApiReqRetried,
    /// API request retry has been delayed.
    ApiReqRetryDelayed,
    /// Configured rate-limit wait (not an error).
    ApiReqRateLimitWait,
    /// API request has been deleted/cancelled.
    ApiReqDeleted,
    /// General text message or assistant response.
    Text,
    /// Image content.
    Image,
    /// Assistant's reasoning or thought process.
    Reasoning,
    /// Final result of task completion.
    CompletionResult,
    /// Message containing user feedback.
    UserFeedback,
    /// Diff-formatted feedback from user.
    UserFeedbackDiff,
    /// Output from an executed command.
    CommandOutput,
    /// Warning about shell integration issues.
    ShellIntegrationWarning,
    /// MCP server request has been initiated.
    McpServerRequestStarted,
    /// Response received from MCP server.
    McpServerResponse,
    /// Result of a completed subtask.
    SubtaskResult,
    /// A checkpoint has been saved.
    CheckpointSaved,
    /// Error related to .rooignore file processing.
    RooignoreError,
    /// Error occurred while applying a diff/patch.
    DiffError,
    /// Context condensation has started.
    CondenseContext,
    /// Error during context condensation.
    CondenseContextError,
    /// Sliding window truncation occurred.
    SlidingWindowTruncation,
    /// Results from codebase search.
    CodebaseSearchResult,
    /// User edited todos.
    UserEditTodos,
    /// Too many MCP tools warning.
    TooManyToolsWarning,
    /// Tool operation.
    Tool,
}

// ---------------------------------------------------------------------------
// ToolProgressStatus
// ---------------------------------------------------------------------------

/// Progress status for a tool operation.
///
/// Source: `packages/types/src/message.ts` — `toolProgressStatusSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProgressStatus {
    pub icon: Option<String>,
    pub text: Option<String>,
}

// ---------------------------------------------------------------------------
// ContextCondense
// ---------------------------------------------------------------------------

/// Data associated with a successful context condensation event.
///
/// Source: `packages/types/src/message.ts` — `contextCondenseSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextCondense {
    /// The API cost incurred for the condensation operation.
    pub cost: f64,
    /// Token count before condensation.
    pub prev_context_tokens: u64,
    /// Token count after condensation.
    pub new_context_tokens: u64,
    /// The condensed summary that replaced the original context.
    pub summary: String,
    /// Optional unique identifier for this condensation operation.
    pub condense_id: Option<String>,
}

// ---------------------------------------------------------------------------
// ContextTruncation
// ---------------------------------------------------------------------------

/// Data associated with a sliding window truncation event.
///
/// Source: `packages/types/src/message.ts` — `contextTruncationSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextTruncation {
    /// Unique identifier for this truncation operation.
    pub truncation_id: String,
    /// Number of conversation messages that were removed.
    pub messages_removed: u64,
    /// Token count before truncation.
    pub prev_context_tokens: u64,
    /// Token count after truncation.
    pub new_context_tokens: u64,
}

// ---------------------------------------------------------------------------
// ClineMessage
// ---------------------------------------------------------------------------

/// The main message type used for communication between the extension and webview.
///
/// Messages can either be "ask" (requiring user response) or "say" (informational).
///
/// Source: `packages/types/src/message.ts` — `clineMessageSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClineMessage {
    /// Timestamp (epoch ms).
    pub ts: f64,
    /// Whether this is an "ask" or "say" message.
    pub r#type: MessageType,
    /// The ask type (present when type is "ask").
    pub ask: Option<ClineAsk>,
    /// The say type (present when type is "say").
    pub say: Option<ClineSay>,
    /// The text content of the message.
    pub text: Option<String>,
    /// Base64-encoded images.
    pub images: Option<Vec<String>>,
    /// Whether this is a partial (streaming) message.
    pub partial: Option<bool>,
    /// Reasoning text (for thinking/reasoning models).
    pub reasoning: Option<String>,
    /// Index into the API conversation history.
    pub conversation_history_index: Option<usize>,
    /// Checkpoint data.
    pub checkpoint: Option<serde_json::Value>,
    /// Progress status for tool operations.
    pub progress_status: Option<ToolProgressStatus>,
    /// Data for successful context condensation.
    pub context_condense: Option<ContextCondense>,
    /// Data for sliding window truncation.
    pub context_truncation: Option<ContextTruncation>,
    /// Whether this message is protected from deletion.
    pub is_protected: Option<bool>,
    /// API protocol used.
    pub api_protocol: Option<ApiProtocol>,
    /// Whether the ask has been answered.
    pub is_answered: Option<bool>,
}

/// Message type: ask or say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Ask,
    Say,
}

/// API protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiProtocol {
    Openai,
    Anthropic,
}

// ---------------------------------------------------------------------------
// TokenUsage
// ---------------------------------------------------------------------------

/// Token usage statistics for a task.
///
/// Source: `packages/types/src/message.ts` — `tokenUsageSchema`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub total_cache_writes: Option<u64>,
    pub total_cache_reads: Option<u64>,
    pub total_cost: f64,
    pub context_tokens: u64,
}

// ---------------------------------------------------------------------------
// QueuedMessage
// ---------------------------------------------------------------------------

/// A message waiting in the queue to be processed.
///
/// Source: `packages/types/src/message.ts` — `queuedMessageSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    pub timestamp: f64,
    pub id: String,
    pub text: String,
    pub images: Option<Vec<String>>,
}
