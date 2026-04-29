//! Task engine type definitions.
//!
//! Defines TaskState, TaskConfig, TaskResult, TaskEvent, TaskError,
//! StreamingState, AssistantMessageContent, ToolUse, McpToolUse,
//! StackItem, AttemptResult, and related types for the task engine.
//!
//! Source: `src/core/task/Task.ts` — Task class properties and types
//! Source: `src/core/assistant-message/types.ts` — AssistantMessageContent
//! Source: `src/shared/tools.ts` — ToolUse, McpToolUse

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use roo_types::message::TokenUsage;
// Note: ToolName from roo_types::tool is used by downstream consumers

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default checkpoint timeout in seconds.
///
/// Source: `packages/types/src/provider-settings.ts` — `DEFAULT_CHECKPOINT_TIMEOUT_SECONDS`
pub const DEFAULT_CHECKPOINT_TIMEOUT_SECONDS: usize = 30;

/// Maximum checkpoint timeout in seconds.
pub const MAX_CHECKPOINT_TIMEOUT_SECONDS: usize = 600;

/// Minimum checkpoint timeout in seconds.
pub const MIN_CHECKPOINT_TIMEOUT_SECONDS: usize = 1;

/// Maximum exponential backoff in seconds for API retries.
///
/// Source: `src/core/task/Task.ts` — `MAX_EXPONENTIAL_BACKOFF_SECONDS`
pub const MAX_EXPONENTIAL_BACKOFF_SECONDS: u64 = 600;

/// Forced context reduction percent (keep 75%, remove 25%).
///
/// Source: `src/core/task/Task.ts` — `FORCED_CONTEXT_REDUCTION_PERCENT`
pub const FORCED_CONTEXT_REDUCTION_PERCENT: u64 = 75;

/// Maximum retries for context window errors.
///
/// Source: `src/core/task/Task.ts` — `MAX_CONTEXT_WINDOW_RETRIES`
pub const MAX_CONTEXT_WINDOW_RETRIES: usize = 3;

/// MCP tool name prefix.
///
/// Source: `src/utils/mcp-name.ts` — `MCP_TOOL_PREFIX`
pub const MCP_TOOL_PREFIX: &str = "mcp";

/// MCP tool name separator.
///
/// Source: `src/utils/mcp-name.ts` — `MCP_TOOL_SEPARATOR`
pub const MCP_TOOL_SEPARATOR: &str = "--";

// ---------------------------------------------------------------------------
// TaskState
// ---------------------------------------------------------------------------

/// The state of a task in its lifecycle.
///
/// State transitions follow a strict state machine:
/// - `Idle` → `Running`
/// - `Running` → `Paused` | `Completed` | `Aborted` | `Delegated`
/// - `Paused` → `Running` | `Aborted`
/// - `Completed` | `Aborted` | `Delegated` → (terminal, no transitions)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Idle,
    Running,
    Paused,
    Completed,
    Aborted,
    Delegated,
}

impl TaskState {
    /// Check whether a transition from `self` to `target` is valid.
    pub fn can_transition_to(&self, target: &TaskState) -> bool {
        match self {
            TaskState::Idle => matches!(target, TaskState::Running),
            TaskState::Running => matches!(
                target,
                TaskState::Paused | TaskState::Completed | TaskState::Aborted | TaskState::Delegated
            ),
            TaskState::Paused => matches!(target, TaskState::Running | TaskState::Aborted),
            TaskState::Delegated => matches!(target, TaskState::Running),
            TaskState::Completed | TaskState::Aborted => false,
        }
    }

    /// Returns `true` if this state is terminal (no further transitions).
    ///
    /// Note: `Delegated` is NOT terminal because it can transition back to
    /// `Running` via `resumeAfterDelegation()`. This matches the TS behavior.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Aborted
        )
    }

    /// Returns `true` if the task is currently active.
    pub fn is_active(&self) -> bool {
        matches!(self, TaskState::Running | TaskState::Paused)
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskState::Idle => write!(f, "idle"),
            TaskState::Running => write!(f, "running"),
            TaskState::Paused => write!(f, "paused"),
            TaskState::Completed => write!(f, "completed"),
            TaskState::Aborted => write!(f, "aborted"),
            TaskState::Delegated => write!(f, "delegated"),
        }
    }
}

impl Default for TaskState {
    fn default() -> Self {
        Self::Idle
    }
}

// ---------------------------------------------------------------------------
// TaskConfig
// ---------------------------------------------------------------------------

/// Configuration for a task.
///
/// Source: `src/core/task/Task.ts` — `TaskOptions` and constructor parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    /// Unique task identifier.
    pub task_id: String,
    /// Root task ID (for subtasks).
    pub root_task_id: Option<String>,
    /// Parent task ID (for subtasks).
    pub parent_task_id: Option<String>,
    /// Task number (for subtask ordering).
    pub task_number: usize,
    /// Current working directory.
    pub cwd: String,
    /// Task mode (e.g., "code", "architect", "ask").
    pub mode: String,
    /// API configuration name (provider profile).
    pub api_config_name: Option<String>,
    /// Workspace path.
    pub workspace: String,
    /// Maximum iterations (None = unlimited).
    pub max_iterations: Option<usize>,
    /// Whether auto-approval is enabled.
    pub auto_approval: bool,
    /// Whether checkpoints are enabled.
    ///
    /// Source: `src/core/task/Task.ts` — `enableCheckpoints`
    pub enable_checkpoints: bool,
    /// Checkpoint timeout in seconds.
    ///
    /// Source: `src/core/task/Task.ts` — `checkpointTimeout`
    pub checkpoint_timeout: usize,
    /// Consecutive mistake limit (overrides global default).
    ///
    /// Source: `src/core/task/Task.ts` — `consecutiveMistakeLimit`
    pub consecutive_mistake_limit: usize,
    /// Initial task text (user message).
    ///
    /// Source: `src/core/task/Task.ts` — `task` option
    pub task_text: Option<String>,
    /// Initial images.
    ///
    /// Source: `src/core/task/Task.ts` — `images` option
    pub images: Vec<String>,
    /// History item ID to resume from.
    ///
    /// Source: `src/core/task/Task.ts` — `historyItem` option
    pub history_item_id: Option<String>,
    /// Base storage path for task persistence.
    ///
    /// When set, the engine will persist messages and metadata to disk.
    /// When `None`, persistence methods are no-ops.
    pub storage_path: Option<String>,
    /// Custom condensing prompt for context compression.
    ///
    /// Source: `src/core/task/Task.ts` — `customSupportPrompts?.CONDENSE`
    /// When set, this prompt is used instead of the default for LLM-based
    /// context condensation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_condensing_prompt: Option<String>,
    /// Instance unique identifier.
    ///
    /// Source: `src/core/task/Task.ts` line 169 — `readonly instanceId: string`
    /// Generated as `crypto.randomUUID().slice(0, 8)` in TS.
    /// In Rust, we use the first 8 chars of a UUIDv4.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instance_id: String,
    /// Whether to automatically start the task upon creation.
    ///
    /// Source: `src/core/task/Task.ts` — `startTask = true` constructor option
    /// When true, the task is started immediately after construction.
    /// If true but no task/images/historyItem is provided, validation will fail.
    pub start_task: bool,
}

impl TaskConfig {
    /// Create a new task config with the given task ID and working directory.
    pub fn new(task_id: impl Into<String>, cwd: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            root_task_id: None,
            parent_task_id: None,
            task_number: 0,
            cwd: cwd.into(),
            mode: "code".to_string(),
            api_config_name: None,
            workspace: String::new(),
            max_iterations: None,
            auto_approval: false,
            enable_checkpoints: true,
            checkpoint_timeout: DEFAULT_CHECKPOINT_TIMEOUT_SECONDS,
            consecutive_mistake_limit: crate::config::DEFAULT_MAX_MISTAKES,
            task_text: None,
            images: Vec::new(),
            history_item_id: None,
            storage_path: None,
            custom_condensing_prompt: None,
            // TS L169: instanceId = crypto.randomUUID().slice(0, 8)
            instance_id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            // TS constructor: startTask = true (default)
            start_task: true,
        }
    }

    /// Create a new task config with auto-generated task ID using uuidv7.
    ///
    /// Source: `src/core/task/Task.ts` line 460 — `this.taskId = taskId ?? uuidv7()`
    pub fn new_auto_id(cwd: impl Into<String>) -> Self {
        let task_id = uuid::Uuid::now_v7().to_string();
        Self::new(task_id, cwd)
    }

    /// Set the mode for this task.
    pub fn with_mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = mode.into();
        self
    }

    /// Set the API config name.
    pub fn with_api_config(mut self, name: impl Into<String>) -> Self {
        self.api_config_name = Some(name.into());
        self
    }

    /// Set the workspace.
    pub fn with_workspace(mut self, workspace: impl Into<String>) -> Self {
        self.workspace = workspace.into();
        self
    }

    /// Set max iterations.
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Set auto-approval.
    pub fn with_auto_approval(mut self, auto: bool) -> Self {
        self.auto_approval = auto;
        self
    }

    /// Set the root task ID.
    pub fn with_root_task_id(mut self, id: impl Into<String>) -> Self {
        self.root_task_id = Some(id.into());
        self
    }

    /// Set the parent task ID.
    pub fn with_parent_task_id(mut self, id: impl Into<String>) -> Self {
        self.parent_task_id = Some(id.into());
        self
    }

    /// Set the task number.
    pub fn with_task_number(mut self, num: usize) -> Self {
        self.task_number = num;
        self
    }

    /// Set whether checkpoints are enabled.
    pub fn with_checkpoints(mut self, enabled: bool) -> Self {
        self.enable_checkpoints = enabled;
        self
    }

    /// Set the checkpoint timeout in seconds.
    pub fn with_checkpoint_timeout(mut self, timeout: usize) -> Self {
        self.checkpoint_timeout = timeout;
        self
    }

    /// Set the consecutive mistake limit.
    pub fn with_consecutive_mistake_limit(mut self, limit: usize) -> Self {
        self.consecutive_mistake_limit = limit;
        self
    }

    /// Set the initial task text.
    pub fn with_task_text(mut self, text: impl Into<String>) -> Self {
        self.task_text = Some(text.into());
        self
    }

    /// Set the initial images.
    pub fn with_images(mut self, images: Vec<String>) -> Self {
        self.images = images;
        self
    }

    /// Set the history item ID to resume from.
    pub fn with_history_item_id(mut self, id: impl Into<String>) -> Self {
        self.history_item_id = Some(id.into());
        self
    }

    /// Set the base storage path for task persistence.
    pub fn with_storage_path(mut self, path: impl Into<String>) -> Self {
        self.storage_path = Some(path.into());
        self
    }

    /// Set whether to automatically start the task.
    ///
    /// Source: `src/core/task/Task.ts` — `startTask` constructor option
    pub fn with_start_task(mut self, start: bool) -> Self {
        self.start_task = start;
        self
    }

    /// Set the instance ID.
    ///
    /// Source: `src/core/task/Task.ts` line 169 — `instanceId`
    pub fn with_instance_id(mut self, id: impl Into<String>) -> Self {
        self.instance_id = id.into();
        self
    }
}

// ---------------------------------------------------------------------------
// TaskResult
// ---------------------------------------------------------------------------

/// The result of a completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskState,
    pub final_message: Option<String>,
    pub token_usage: TokenUsage,
    pub iterations: usize,
    pub tool_usage: HashMap<String, usize>,
}

impl TaskResult {
    /// Create a new task result.
    pub fn new(task_id: impl Into<String>, status: TaskState) -> Self {
        Self {
            task_id: task_id.into(),
            status,
            final_message: None,
            token_usage: TokenUsage::default(),
            iterations: 0,
            tool_usage: HashMap::new(),
        }
    }

    /// Record a tool usage.
    pub fn record_tool_usage(&mut self, tool_name: &str) {
        *self.tool_usage.entry(tool_name.to_string()).or_insert(0) += 1;
    }
}

// ---------------------------------------------------------------------------
// StreamingState
// ---------------------------------------------------------------------------

/// Streaming state for the current API request.
///
/// Tracks the state of the current streaming response from the API,
/// including content parsing, tool call processing, and checkpoint status.
///
/// Source: `src/core/task/Task.ts` — streaming-related properties:
/// `isStreaming`, `isWaitingForFirstChunk`, `currentStreamingContentIndex`,
/// `didCompleteReadingStream`, `assistantMessageSavedToHistory`, etc.
#[derive(Debug, Clone)]
pub struct StreamingState {
    /// Whether we are currently streaming an API response.
    pub is_streaming: bool,
    /// Whether we are waiting for the first chunk of the response.
    pub is_waiting_for_first_chunk: bool,
    /// Current content index within the streaming response.
    pub current_streaming_content_index: usize,
    /// Whether a checkpoint was done for the current streaming session.
    pub current_streaming_did_checkpoint: bool,
    /// Whether the stream has been fully read.
    pub did_complete_reading_stream: bool,
    /// Whether the assistant message has been saved to API conversation history.
    ///
    /// This is critical for parallel tool calling: tools should NOT execute until
    /// the assistant message is saved.
    pub assistant_message_saved_to_history: bool,
    /// Whether a tool was rejected in the current streaming session.
    pub did_reject_tool: bool,
    /// Whether a tool was already used in the current streaming session.
    pub did_already_use_tool: bool,
    /// Whether a tool failed in the current turn.
    pub did_tool_fail_in_current_turn: bool,
    /// Whether the present-assistant-message handler is locked.
    pub present_assistant_message_locked: bool,
    /// Whether there are pending updates for the assistant message handler.
    pub present_assistant_message_has_pending_updates: bool,
    /// Whether the stream finished aborting.
    ///
    /// Source: `src/core/task/Task.ts` — `didFinishAbortingStream`
    pub did_finish_aborting_stream: bool,
}

impl StreamingState {
    /// Create a new default streaming state.
    pub fn new() -> Self {
        Self {
            is_streaming: false,
            is_waiting_for_first_chunk: false,
            current_streaming_content_index: 0,
            current_streaming_did_checkpoint: false,
            did_complete_reading_stream: false,
            assistant_message_saved_to_history: false,
            did_reject_tool: false,
            did_already_use_tool: false,
            did_tool_fail_in_current_turn: false,
            present_assistant_message_locked: false,
            present_assistant_message_has_pending_updates: false,
            did_finish_aborting_stream: false,
        }
    }

    /// Reset all streaming state for a new API request.
    ///
    /// Source: `src/core/task/Task.ts` — streaming state reset block in
    /// `recursivelyMakeClineRequests`
    pub fn reset_for_new_request(&mut self) {
        self.current_streaming_content_index = 0;
        self.current_streaming_did_checkpoint = false;
        self.did_complete_reading_stream = false;
        self.assistant_message_saved_to_history = false;
        self.did_reject_tool = false;
        self.did_already_use_tool = false;
        self.did_tool_fail_in_current_turn = false;
        self.present_assistant_message_locked = false;
        self.present_assistant_message_has_pending_updates = false;
        self.did_finish_aborting_stream = false;
    }
}

impl Default for StreamingState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TaskError
// ---------------------------------------------------------------------------

/// Errors that can occur during task execution.
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    /// Invalid state transition.
    #[error("invalid state transition from {from} to {to}")]
    InvalidTransition { from: TaskState, to: TaskState },

    /// Task has been cancelled.
    #[error("task cancelled")]
    Cancelled,

    /// Maximum iterations exceeded.
    #[error("maximum iterations exceeded: {0}")]
    MaxIterationsExceeded(usize),

    /// Maximum consecutive mistakes exceeded.
    #[error("maximum consecutive mistakes exceeded: {0}")]
    MaxMistakesExceeded(usize),

    /// A general task error.
    #[error("task error: {0}")]
    General(String),

    /// An I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A serialization error.
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// A persistence error.
    #[error("persistence error: {0}")]
    Persistence(#[from] roo_task_persistence::TaskPersistenceError),
}

// ---------------------------------------------------------------------------
// AssistantMessageContent — text / tool_use / mcp_tool_use
// ---------------------------------------------------------------------------
// Source: `src/core/assistant-message/types.ts`
//   export type AssistantMessageContent = TextContent | ToolUse | McpToolUse

/// Text content block in an assistant message.
///
/// Source: `src/shared/tools.ts` — `TextContent`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String, // always "text"
    /// The text content.
    pub content: String,
    /// Whether this is a partial (streaming) block.
    #[serde(default)]
    pub partial: bool,
}

/// A tool use block in an assistant message.
///
/// Source: `src/shared/tools.ts` — `ToolUse<TName>`
/// Represents a parsed tool call from the API with typed native arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUse {
    #[serde(skip)]
    pub content_type: String, // always "tool_use"
    /// The tool name.
    pub name: String,
    /// Stringified params for display/logging (legacy format).
    ///
    /// Source: `src/shared/tools.ts` — `params: Partial<Record<ToolParamName, string>>`
    pub params: HashMap<String, String>,
    /// Whether this is a partial (streaming) block.
    #[serde(default)]
    pub partial: bool,
    /// The tool call ID assigned by the API.
    #[serde(default)]
    pub id: String,
    /// Typed native arguments for tool execution.
    ///
    /// Source: `src/shared/tools.ts` — `nativeArgs`
    /// In the TS version, this is a typed union per tool. In Rust, we store
    /// the parsed JSON value and let each tool handler extract what it needs.
    #[serde(default)]
    pub native_args: Option<serde_json::Value>,
    /// Original tool name if an alias was resolved.
    #[serde(default)]
    pub original_name: Option<String>,
    /// Whether the legacy format was used (for telemetry).
    #[serde(default)]
    pub used_legacy_format: bool,
}

/// An MCP tool use block in an assistant message.
///
/// Source: `src/shared/tools.ts` — `McpToolUse`
/// Represents a dynamic MCP tool call (mcp--serverName--toolName format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolUse {
    #[serde(skip)]
    pub content_type: String, // always "mcp_tool_use"
    /// The full MCP tool name (e.g., "mcp--serverName--toolName").
    pub name: String,
    /// The tool call ID assigned by the API.
    pub id: String,
    /// The MCP server name (sanitized).
    pub server_name: String,
    /// The MCP tool name on the server.
    pub tool_name: String,
    /// The parsed arguments for the MCP tool.
    pub arguments: serde_json::Value,
    /// Whether this is a partial (streaming) block.
    #[serde(default)]
    pub partial: bool,
}

/// Assistant message content: either text, a tool use, or an MCP tool use.
///
/// Source: `src/core/assistant-message/types.ts`
///   export type AssistantMessageContent = TextContent | ToolUse | McpToolUse
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AssistantMessageContent {
    /// Text content from the assistant.
    #[serde(rename = "text")]
    Text {
        content: String,
        #[serde(default)]
        partial: bool,
    },
    /// A built-in tool use request.
    #[serde(rename = "tool_use")]
    ToolUse(ToolUse),
    /// An MCP tool use request (dynamic tool from MCP server).
    #[serde(rename = "mcp_tool_use")]
    McpToolUse(McpToolUse),
}

impl AssistantMessageContent {
    /// Returns `true` if this content represents a tool call.
    pub fn is_tool_call(&self) -> bool {
        matches!(self, Self::ToolUse(_) | Self::McpToolUse(_))
    }

    /// Returns `true` if this is a partial (still streaming) block.
    pub fn is_partial(&self) -> bool {
        match self {
            Self::Text { partial, .. } => *partial,
            Self::ToolUse(tu) => tu.partial,
            Self::McpToolUse(mtu) => mtu.partial,
        }
    }

    /// Get the tool call ID, if this is a tool use block.
    pub fn tool_call_id(&self) -> Option<&str> {
        match self {
            Self::ToolUse(tu) => Some(&tu.id),
            Self::McpToolUse(mtu) => Some(&mtu.id),
            Self::Text { .. } => None,
        }
    }

    /// Get the tool name, if this is a tool use block.
    pub fn tool_name(&self) -> Option<&str> {
        match self {
            Self::ToolUse(tu) => Some(&tu.name),
            Self::McpToolUse(mtu) => Some(&mtu.name),
            Self::Text { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ToolCallStreamEvent — events from raw chunk processing
// ---------------------------------------------------------------------------
// Source: `src/core/assistant-message/NativeToolCallParser.ts`
//   export type ToolCallStreamEvent =
//     ApiStreamToolCallStartChunk | ApiStreamToolCallDeltaChunk | ApiStreamToolCallEndChunk

/// Stream events emitted during raw tool call chunk processing.
///
/// Source: `src/core/assistant-message/NativeToolCallParser.ts` — `ToolCallStreamEvent`
#[derive(Debug, Clone)]
pub enum ToolCallStreamEvent {
    /// A new tool call has started.
    Start {
        id: String,
        name: String,
    },
    /// A delta of arguments has been received for an active tool call.
    Delta {
        id: String,
        delta: String,
    },
    /// A tool call has ended (stream finished).
    End {
        id: String,
    },
}

// ---------------------------------------------------------------------------
// StreamEvent — real-time streaming event (replaces batch AttemptResult)
// ---------------------------------------------------------------------------

/// A real-time event from the streaming API response.
///
/// This is the core type for the new streaming architecture. Instead of
/// collecting all chunks into a `ParsedStreamContent` batch, each chunk
/// is converted into one or more `StreamEvent`s and sent via `mpsc::channel`
/// for real-time consumption by `recursively_make_cline_requests()`.
///
/// Source: `src/core/task/Task.ts` — async generator `attemptApiRequest()`
/// where each chunk is yielded individually for `for await` consumption.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Text content delta from the assistant.
    TextDelta { text: String },

    /// Reasoning/thinking content delta.
    ReasoningDelta { text: String },

    /// A tool call has started (streaming start event).
    ToolCallStart { id: String, name: String },

    /// A tool call argument delta (streaming).
    ToolCallDelta { id: String, delta: String },

    /// A tool call has ended (streaming end event).
    ToolCallEnd { id: String },

    /// A complete tool call (non-streaming provider, all-at-once).
    ToolCallComplete { id: String, name: String, arguments: String },

    /// A raw partial tool call chunk (index-based, needs NativeToolCallParser processing).
    ToolCallPartial {
        index: u64,
        id: Option<String>,
        name: Option<String>,
        arguments: Option<String>,
    },

    /// Token usage information.
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cache_write_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
        reasoning_tokens: Option<u64>,
        total_cost: Option<f64>,
    },

    /// Grounding sources (Gemini search-augmented models).
    Grounding { sources: Vec<roo_types::api::GroundingSource> },

    /// Thinking block completed with signature (Anthropic extended thinking).
    ThinkingComplete { signature: String },

    /// The stream has completed successfully.
    StreamCompleted,

    /// An error occurred during streaming.
    Error {
        message: String,
        /// Whether this error occurred on the first chunk (context window exceeded).
        is_first_chunk: bool,
    },
}

// ---------------------------------------------------------------------------
// StreamingToolCallState — state for streaming tool call accumulation
// ---------------------------------------------------------------------------
// Source: `src/core/assistant-message/NativeToolCallParser.ts`
//   private static streamingToolCalls = new Map<string, { id, name, argumentsAccumulator }>()

/// Internal state tracking a streaming tool call's argument accumulation.
///
/// Source: `src/core/assistant-message/NativeToolCallParser.ts` — `streamingToolCalls` map value
#[derive(Debug, Clone)]
pub struct StreamingToolCallState {
    /// Tool call ID.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Accumulated JSON arguments string.
    pub arguments_accumulator: String,
}

// ---------------------------------------------------------------------------
// RawChunkTrackerEntry — state for raw chunk tracking
// ---------------------------------------------------------------------------
// Source: `src/core/assistant-message/NativeToolCallParser.ts`
//   private static rawChunkTracker = new Map<number, { id, name, hasStarted, deltaBuffer }>()

/// Internal state tracking a raw chunk from the API stream.
///
/// Source: `src/core/assistant-message/NativeToolCallParser.ts` — `rawChunkTracker` map value
#[derive(Debug, Clone)]
pub struct RawChunkTrackerEntry {
    /// Tool call ID.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Whether the start event has been emitted.
    pub has_started: bool,
    /// Buffered deltas received before the start event.
    pub delta_buffer: Vec<String>,
}

// ---------------------------------------------------------------------------
// StackItem — recursive loop stack item
// ---------------------------------------------------------------------------
// Source: `src/core/task/Task.ts` — used in `recursivelyMakeClineRequests`
//   The stack tracks the recursion state for the agent loop.

/// A stack item for tracking the recursive agent loop state.
///
/// Source: `src/core/task/Task.ts` — `recursivelyMakeClineRequests` recursion state
/// Used to track the state of each recursive call in the agent loop.
#[derive(Debug, Clone)]
pub enum StackItem {
    /// The agent is waiting for a tool result to be processed.
    ToolResult {
        tool_use_id: String,
        tool_name: String,
    },
    /// The agent is waiting for user input (ask response).
    AskResponse {
        ask_type: String,
    },
    /// The agent is processing a sub-task (new_task delegation).
    SubTask {
        parent_task_id: String,
        child_task_id: String,
    },
}

// ---------------------------------------------------------------------------
// AttemptResult — API request result
// ---------------------------------------------------------------------------
// Source: `src/core/task/Task.ts` — `recursivelyMakeClineRequests` return logic
//   The result of an API attempt determines what happens next in the loop.

/// The result of an API request attempt.
///
/// Source: `src/core/task/Task.ts` — `recursivelyMakeClineRequests` return values
/// Determines what happens next in the agent loop after an API call.
#[derive(Debug, Clone)]
pub enum AttemptResult {
    /// The API request completed successfully and the loop should continue.
    Continue,
    /// The API request completed successfully and the task is done.
    Completed,
    /// The task was aborted (user cancelled, rate limit, etc.).
    Aborted {
        reason: Option<String>,
    },
    /// The task was delegated to a sub-task.
    Delegated {
        child_task_id: String,
    },
    /// The context window was exceeded and needs truncation.
    ContextWindowExceeded {
        retry_count: usize,
    },
    /// A rate limit was hit; back off and retry.
    RateLimited {
        retry_after_ms: Option<u64>,
    },
    /// An API error occurred.
    ApiError {
        error: String,
        retryable: bool,
    },
    /// The user paused the task.
    Paused,
    /// No response from the API (empty response).
    NoResponse,
}

impl AttemptResult {
    /// Returns `true` if the task should continue looping after this result.
    pub fn should_continue(&self) -> bool {
        matches!(self, Self::Continue | Self::ContextWindowExceeded { .. })
    }

    /// Returns `true` if this result represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Aborted { .. } | Self::Delegated { .. } | Self::Paused
        )
    }
}

// ---------------------------------------------------------------------------
// DiffStrategy
// ---------------------------------------------------------------------------
// Source: `src/shared/tools.ts` — `DiffStrategy`

/// Diff strategy for applying file edits.
///
/// Source: `src/shared/tools.ts` — `DiffStrategy`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStrategy {
    /// Use the multi-search-replace diff strategy.
    MultiSearchReplace,
    /// Use the standard unified diff strategy.
    Unified,
}

// ---------------------------------------------------------------------------
// ToolParamName — valid tool parameter names
// ---------------------------------------------------------------------------
// Source: `src/shared/tools.ts` — `toolParamNames`

/// All valid tool parameter names.
///
/// Source: `src/shared/tools.ts` — `toolParamNames`
pub const TOOL_PARAM_NAMES: &[&str] = &[
    "path",
    "content",
    "diff",
    "command",
    "cwd",
    "timeout",
    "mode",
    "mode_slug",
    "reason",
    "message",
    "todos",
    "question",
    "follow_up",
    "result",
    "regex",
    "file_pattern",
    "recursive",
    "query",
    "skill",
    "args",
    "server_name",
    "tool_name",
    "arguments",
    "uri",
    "artifact_id",
    "search",
    "offset",
    "limit",
    "line_ranges",
    "file_path",
    "old_string",
    "new_string",
    "replace_all",
    "expected_replacements",
    "images",
    "files",
    "anchor_line",
    "max_levels",
    "max_lines",
    "include_siblings",
    "include_header",
    "indentation",
    "patch",
    "prompt",
    "image",
    "directory_prefix",
];

/// Check if a parameter name is a recognized tool parameter.
pub fn is_valid_tool_param(name: &str) -> bool {
    TOOL_PARAM_NAMES.contains(&name)
}

// ---------------------------------------------------------------------------
// MCP tool name utilities
// ---------------------------------------------------------------------------

/// Check if a tool name is a dynamic MCP tool (mcp--serverName--toolName format).
///
/// Source: `src/utils/mcp-name.ts` — MCP tool name detection
pub fn is_mcp_tool_name(name: &str) -> bool {
    let prefix = format!("{}{}", MCP_TOOL_PREFIX, MCP_TOOL_SEPARATOR);
    name.starts_with(&prefix)
}

/// Parse an MCP tool name into (server_name, tool_name).
///
/// Source: `src/utils/mcp-name.ts` — `parseMcpToolName`
/// Format: `mcp--serverName--toolName`
pub fn parse_mcp_tool_name(name: &str) -> Option<(String, String)> {
    let prefix = format!("{}{}", MCP_TOOL_PREFIX, MCP_TOOL_SEPARATOR);
    if !name.starts_with(&prefix) {
        return None;
    }
    let rest = &name[prefix.len()..];
    let separator = MCP_TOOL_SEPARATOR;
    if let Some(sep_pos) = rest.find(separator) {
        let server_name = &rest[..sep_pos];
        let tool_name = &rest[sep_pos + separator.len()..];
        if !server_name.is_empty() && !tool_name.is_empty() {
            return Some((server_name.to_string(), tool_name.to_string()));
        }
    }
    None
}

/// Normalize MCP tool name: convert underscores to hyphens.
///
/// Source: `src/utils/mcp-name.ts` — `normalizeMcpToolName`
/// Some models output `mcp__serverName__toolName` instead of `mcp--serverName--toolName`.
pub fn normalize_mcp_tool_name(name: &str) -> String {
    // Only normalize the prefix and separator parts
    if name.starts_with("mcp__") {
        name.replacen("mcp__", &format!("mcp{}", MCP_TOOL_SEPARATOR), 1)
            .replace("__", MCP_TOOL_SEPARATOR)
    } else {
        name.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- TaskState tests ---

    #[test]
    fn test_task_state_default() {
        assert_eq!(TaskState::default(), TaskState::Idle);
    }

    #[test]
    fn test_task_state_display() {
        assert_eq!(format!("{}", TaskState::Idle), "idle");
        assert_eq!(format!("{}", TaskState::Running), "running");
        assert_eq!(format!("{}", TaskState::Paused), "paused");
        assert_eq!(format!("{}", TaskState::Completed), "completed");
        assert_eq!(format!("{}", TaskState::Aborted), "aborted");
        assert_eq!(format!("{}", TaskState::Delegated), "delegated");
    }

    #[test]
    fn test_task_state_is_terminal() {
        assert!(!TaskState::Idle.is_terminal());
        assert!(!TaskState::Running.is_terminal());
        assert!(!TaskState::Paused.is_terminal());
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Aborted.is_terminal());
        // Delegated is NOT terminal — can resume after delegation
        assert!(!TaskState::Delegated.is_terminal());
    }

    #[test]
    fn test_task_state_is_active() {
        assert!(!TaskState::Idle.is_active());
        assert!(TaskState::Running.is_active());
        assert!(TaskState::Paused.is_active());
        assert!(!TaskState::Completed.is_active());
        assert!(!TaskState::Aborted.is_active());
        assert!(!TaskState::Delegated.is_active());
    }

    #[test]
    fn test_valid_transitions_from_idle() {
        assert!(TaskState::Idle.can_transition_to(&TaskState::Running));
        assert!(!TaskState::Idle.can_transition_to(&TaskState::Paused));
        assert!(!TaskState::Idle.can_transition_to(&TaskState::Completed));
        assert!(!TaskState::Idle.can_transition_to(&TaskState::Aborted));
        assert!(!TaskState::Idle.can_transition_to(&TaskState::Idle));
    }

    #[test]
    fn test_valid_transitions_from_running() {
        assert!(TaskState::Running.can_transition_to(&TaskState::Paused));
        assert!(TaskState::Running.can_transition_to(&TaskState::Completed));
        assert!(TaskState::Running.can_transition_to(&TaskState::Aborted));
        assert!(TaskState::Running.can_transition_to(&TaskState::Delegated));
        assert!(!TaskState::Running.can_transition_to(&TaskState::Idle));
        assert!(!TaskState::Running.can_transition_to(&TaskState::Running));
    }

    #[test]
    fn test_valid_transitions_from_paused() {
        assert!(TaskState::Paused.can_transition_to(&TaskState::Running));
        assert!(TaskState::Paused.can_transition_to(&TaskState::Aborted));
        assert!(!TaskState::Paused.can_transition_to(&TaskState::Idle));
        assert!(!TaskState::Paused.can_transition_to(&TaskState::Paused));
        assert!(!TaskState::Paused.can_transition_to(&TaskState::Completed));
    }

    #[test]
    fn test_no_transitions_from_terminal_states() {
        for terminal in [TaskState::Completed, TaskState::Aborted] {
            for target in [
                TaskState::Idle,
                TaskState::Running,
                TaskState::Paused,
                TaskState::Completed,
                TaskState::Aborted,
                TaskState::Delegated,
            ] {
                assert!(!terminal.can_transition_to(&target), "{terminal:?} should not transition to {target:?}");
            }
        }
        assert!(TaskState::Delegated.can_transition_to(&TaskState::Running));
        assert!(!TaskState::Delegated.can_transition_to(&TaskState::Idle));
        assert!(!TaskState::Delegated.can_transition_to(&TaskState::Paused));
        assert!(!TaskState::Delegated.can_transition_to(&TaskState::Completed));
        assert!(!TaskState::Delegated.can_transition_to(&TaskState::Aborted));
    }

    #[test]
    fn test_task_state_serde_roundtrip() {
        for state in [
            TaskState::Idle,
            TaskState::Running,
            TaskState::Paused,
            TaskState::Completed,
            TaskState::Aborted,
            TaskState::Delegated,
        ] {
            let json = serde_json::to_string(&state).unwrap();
            let back: TaskState = serde_json::from_str(&json).unwrap();
            assert_eq!(back, state);
        }
    }

    // --- TaskConfig tests ---

    #[test]
    fn test_task_config_new() {
        let config = TaskConfig::new("task-1", "/tmp/work");
        assert_eq!(config.task_id, "task-1");
        assert_eq!(config.cwd, "/tmp/work");
        assert_eq!(config.mode, "code");
        assert!(config.api_config_name.is_none());
        assert!(!config.auto_approval);
        assert!(config.max_iterations.is_none());
        assert!(config.enable_checkpoints);
        assert_eq!(config.checkpoint_timeout, DEFAULT_CHECKPOINT_TIMEOUT_SECONDS);
        assert_eq!(config.consecutive_mistake_limit, crate::config::DEFAULT_MAX_MISTAKES);
        assert!(config.task_text.is_none());
        assert!(config.images.is_empty());
        assert!(config.history_item_id.is_none());
    }

    #[test]
    fn test_task_config_builder() {
        let config = TaskConfig::new("task-2", "/tmp/work")
            .with_mode("architect")
            .with_api_config("gpt4")
            .with_workspace("/tmp/ws")
            .with_max_iterations(100)
            .with_auto_approval(true)
            .with_root_task_id("root-1")
            .with_parent_task_id("parent-1")
            .with_task_number(3)
            .with_checkpoints(false)
            .with_checkpoint_timeout(60)
            .with_consecutive_mistake_limit(5)
            .with_task_text("Fix the bug")
            .with_images(vec!["img1.png".to_string()])
            .with_history_item_id("hist-1");

        assert_eq!(config.task_id, "task-2");
        assert_eq!(config.mode, "architect");
        assert_eq!(config.api_config_name, Some("gpt4".to_string()));
        assert_eq!(config.workspace, "/tmp/ws");
        assert_eq!(config.max_iterations, Some(100));
        assert!(config.auto_approval);
        assert_eq!(config.root_task_id, Some("root-1".to_string()));
        assert_eq!(config.parent_task_id, Some("parent-1".to_string()));
        assert_eq!(config.task_number, 3);
        assert!(!config.enable_checkpoints);
        assert_eq!(config.checkpoint_timeout, 60);
        assert_eq!(config.consecutive_mistake_limit, 5);
        assert_eq!(config.task_text, Some("Fix the bug".to_string()));
        assert_eq!(config.images, vec!["img1.png".to_string()]);
        assert_eq!(config.history_item_id, Some("hist-1".to_string()));
    }

    #[test]
    fn test_task_config_serialization() {
        let config = TaskConfig::new("task-3", "/tmp/work")
            .with_mode("code")
            .with_max_iterations(50);

        let json = serde_json::to_string(&config).unwrap();
        let back: TaskConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.task_id, "task-3");
        assert_eq!(back.max_iterations, Some(50));
    }

    // --- TaskResult tests ---

    #[test]
    fn test_task_result_new() {
        let result = TaskResult::new("task-1", TaskState::Completed);
        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.status, TaskState::Completed);
        assert!(result.final_message.is_none());
        assert_eq!(result.iterations, 0);
        assert!(result.tool_usage.is_empty());
    }

    #[test]
    fn test_task_result_record_tool_usage() {
        let mut result = TaskResult::new("task-1", TaskState::Completed);
        result.record_tool_usage("read_file");
        result.record_tool_usage("read_file");
        result.record_tool_usage("write_to_file");

        assert_eq!(result.tool_usage["read_file"], 2);
        assert_eq!(result.tool_usage["write_to_file"], 1);
    }

    #[test]
    fn test_task_result_serialization() {
        let mut result = TaskResult::new("task-1", TaskState::Completed);
        result.final_message = Some("Done!".to_string());
        result.iterations = 10;
        result.record_tool_usage("read_file");

        let json = serde_json::to_string(&result).unwrap();
        let back: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.task_id, "task-1");
        assert_eq!(back.final_message, Some("Done!".to_string()));
        assert_eq!(back.iterations, 10);
        assert_eq!(back.tool_usage["read_file"], 1);
    }

    // --- StreamingState tests ---

    #[test]
    fn test_streaming_state_new() {
        let state = StreamingState::new();
        assert!(!state.is_streaming);
        assert!(!state.is_waiting_for_first_chunk);
        assert_eq!(state.current_streaming_content_index, 0);
        assert!(!state.current_streaming_did_checkpoint);
        assert!(!state.did_complete_reading_stream);
        assert!(!state.assistant_message_saved_to_history);
        assert!(!state.did_reject_tool);
        assert!(!state.did_already_use_tool);
        assert!(!state.did_tool_fail_in_current_turn);
        assert!(!state.present_assistant_message_locked);
        assert!(!state.present_assistant_message_has_pending_updates);
        assert!(!state.did_finish_aborting_stream);
    }

    #[test]
    fn test_streaming_state_default() {
        assert_eq!(StreamingState::default().is_streaming, false);
    }

    #[test]
    fn test_streaming_state_reset_for_new_request() {
        let mut state = StreamingState::new();
        state.is_streaming = true;
        state.current_streaming_content_index = 5;
        state.assistant_message_saved_to_history = true;
        state.did_tool_fail_in_current_turn = true;
        state.did_finish_aborting_stream = true;

        state.reset_for_new_request();

        assert_eq!(state.current_streaming_content_index, 0);
        assert!(!state.current_streaming_did_checkpoint);
        assert!(!state.did_complete_reading_stream);
        assert!(!state.assistant_message_saved_to_history);
        assert!(!state.did_reject_tool);
        assert!(!state.did_already_use_tool);
        assert!(!state.did_tool_fail_in_current_turn);
        assert!(!state.present_assistant_message_locked);
        assert!(!state.present_assistant_message_has_pending_updates);
        assert!(!state.did_finish_aborting_stream);

        // is_streaming should NOT be reset by reset_for_new_request
        assert!(state.is_streaming);
    }

    // --- TaskError tests ---

    #[test]
    fn test_task_error_invalid_transition() {
        let err = TaskError::InvalidTransition {
            from: TaskState::Idle,
            to: TaskState::Completed,
        };
        assert!(err.to_string().contains("invalid state transition"));
    }

    #[test]
    fn test_task_error_cancelled() {
        let err = TaskError::Cancelled;
        assert_eq!(err.to_string(), "task cancelled");
    }

    #[test]
    fn test_task_error_max_iterations() {
        let err = TaskError::MaxIterationsExceeded(100);
        assert!(err.to_string().contains("100"));
    }

    // --- Constants tests ---

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_CHECKPOINT_TIMEOUT_SECONDS, 30);
        assert_eq!(MAX_CHECKPOINT_TIMEOUT_SECONDS, 600);
        assert_eq!(MIN_CHECKPOINT_TIMEOUT_SECONDS, 1);
        assert_eq!(MAX_EXPONENTIAL_BACKOFF_SECONDS, 600);
        assert_eq!(FORCED_CONTEXT_REDUCTION_PERCENT, 75);
        assert_eq!(MAX_CONTEXT_WINDOW_RETRIES, 3);
    }

    // --- AssistantMessageContent tests ---

    #[test]
    fn test_assistant_message_content_text() {
        let content = AssistantMessageContent::Text {
            content: "Hello".to_string(),
            partial: false,
        };
        assert!(!content.is_tool_call());
        assert!(!content.is_partial());
        assert!(content.tool_call_id().is_none());
        assert!(content.tool_name().is_none());
    }

    #[test]
    fn test_assistant_message_content_tool_use() {
        let content = AssistantMessageContent::ToolUse(ToolUse {
            content_type: "tool_use".to_string(),
            name: "read_file".to_string(),
            params: HashMap::new(),
            partial: false,
            id: "call_123".to_string(),
            native_args: None,
            original_name: None,
            used_legacy_format: false,
        });
        assert!(content.is_tool_call());
        assert!(!content.is_partial());
        assert_eq!(content.tool_call_id(), Some("call_123"));
        assert_eq!(content.tool_name(), Some("read_file"));
    }

    #[test]
    fn test_assistant_message_content_mcp_tool_use() {
        let content = AssistantMessageContent::McpToolUse(McpToolUse {
            content_type: "mcp_tool_use".to_string(),
            name: "mcp--server--tool".to_string(),
            id: "call_456".to_string(),
            server_name: "server".to_string(),
            tool_name: "tool".to_string(),
            arguments: serde_json::json!({}),
            partial: true,
        });
        assert!(content.is_tool_call());
        assert!(content.is_partial());
        assert_eq!(content.tool_call_id(), Some("call_456"));
        assert_eq!(content.tool_name(), Some("mcp--server--tool"));
    }

    #[test]
    fn test_assistant_message_content_serde_roundtrip() {
        // Text variant
        let text = AssistantMessageContent::Text {
            content: "Hello".to_string(),
            partial: false,
        };
        let json = serde_json::to_string(&text).unwrap();
        let back: AssistantMessageContent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AssistantMessageContent::Text { .. }));

        // ToolUse variant
        let tool_use = AssistantMessageContent::ToolUse(ToolUse {
            content_type: "tool_use".to_string(),
            name: "read_file".to_string(),
            params: HashMap::new(),
            partial: false,
            id: "call_1".to_string(),
            native_args: None,
            original_name: None,
            used_legacy_format: false,
        });
        let json = serde_json::to_string(&tool_use).unwrap();
        let back: AssistantMessageContent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AssistantMessageContent::ToolUse(_)));

        // McpToolUse variant
        let mcp = AssistantMessageContent::McpToolUse(McpToolUse {
            content_type: "mcp_tool_use".to_string(),
            name: "mcp--server--tool".to_string(),
            id: "call_2".to_string(),
            server_name: "server".to_string(),
            tool_name: "tool".to_string(),
            arguments: serde_json::json!({"key": "value"}),
            partial: false,
        });
        let json = serde_json::to_string(&mcp).unwrap();
        let back: AssistantMessageContent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AssistantMessageContent::McpToolUse(_)));
    }

    // --- AttemptResult tests ---

    #[test]
    fn test_attempt_result_should_continue() {
        assert!(AttemptResult::Continue.should_continue());
        assert!(AttemptResult::ContextWindowExceeded { retry_count: 0 }.should_continue());
        assert!(!AttemptResult::Completed.should_continue());
        assert!(!AttemptResult::Aborted { reason: None }.should_continue());
        assert!(!AttemptResult::RateLimited { retry_after_ms: None }.should_continue());
    }

    #[test]
    fn test_attempt_result_is_terminal() {
        assert!(AttemptResult::Completed.is_terminal());
        assert!(AttemptResult::Aborted { reason: None }.is_terminal());
        assert!(AttemptResult::Delegated { child_task_id: "c1".into() }.is_terminal());
        assert!(AttemptResult::Paused.is_terminal());
        assert!(!AttemptResult::Continue.is_terminal());
        assert!(!AttemptResult::ApiError { error: "err".into(), retryable: true }.is_terminal());
    }

    // --- MCP tool name utilities tests ---

    #[test]
    fn test_is_mcp_tool_name() {
        assert!(is_mcp_tool_name("mcp--serverName--toolName"));
        assert!(is_mcp_tool_name("mcp--my_server--my_tool"));
        assert!(!is_mcp_tool_name("read_file"));
        assert!(!is_mcp_tool_name("use_mcp_tool"));
        assert!(!is_mcp_tool_name("mcp_tool"));
    }

    #[test]
    fn test_parse_mcp_tool_name() {
        let (server, tool) = parse_mcp_tool_name("mcp--serverName--toolName").unwrap();
        assert_eq!(server, "serverName");
        assert_eq!(tool, "toolName");

        let (server, tool) = parse_mcp_tool_name("mcp--my_server--my_tool").unwrap();
        assert_eq!(server, "my_server");
        assert_eq!(tool, "my_tool");

        assert!(parse_mcp_tool_name("read_file").is_none());
        assert!(parse_mcp_tool_name("mcp--").is_none());
        assert!(parse_mcp_tool_name("mcp--only").is_none());
    }

    #[test]
    fn test_normalize_mcp_tool_name() {
        // Underscores in prefix should be converted to hyphens
        assert_eq!(
            normalize_mcp_tool_name("mcp__serverName__toolName"),
            "mcp--serverName--toolName"
        );
        // Already normalized names should pass through
        assert_eq!(
            normalize_mcp_tool_name("mcp--serverName--toolName"),
            "mcp--serverName--toolName"
        );
        // Non-MCP names should pass through
        assert_eq!(normalize_mcp_tool_name("read_file"), "read_file");
    }

    // --- ToolCallStreamEvent tests ---

    #[test]
    fn test_tool_call_stream_event_start() {
        let event = ToolCallStreamEvent::Start {
            id: "call_1".into(),
            name: "read_file".into(),
        };
        match &event {
            ToolCallStreamEvent::Start { id, name } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
            }
            _ => panic!("Expected Start variant"),
        }
    }

    #[test]
    fn test_tool_call_stream_event_delta() {
        let event = ToolCallStreamEvent::Delta {
            id: "call_1".into(),
            delta: r#"{"path":"x.rs"}"#.into(),
        };
        match &event {
            ToolCallStreamEvent::Delta { id, delta } => {
                assert_eq!(id, "call_1");
                assert_eq!(delta, r#"{"path":"x.rs"}"#);
            }
            _ => panic!("Expected Delta variant"),
        }
    }

    #[test]
    fn test_tool_call_stream_event_end() {
        let event = ToolCallStreamEvent::End { id: "call_1".into() };
        match &event {
            ToolCallStreamEvent::End { id } => {
                assert_eq!(id, "call_1");
            }
            _ => panic!("Expected End variant"),
        }
    }

    // --- DiffStrategy tests ---

    #[test]
    fn test_diff_strategy_serde() {
        let strategy = DiffStrategy::MultiSearchReplace;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"multi_search_replace\"");
        let back: DiffStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, DiffStrategy::MultiSearchReplace);
    }

    // --- Tool param validation tests ---

    #[test]
    fn test_is_valid_tool_param() {
        assert!(is_valid_tool_param("path"));
        assert!(is_valid_tool_param("content"));
        assert!(is_valid_tool_param("command"));
        assert!(is_valid_tool_param("server_name"));
        assert!(!is_valid_tool_param("unknown_param"));
        assert!(!is_valid_tool_param(""));
    }

    // --- StackItem tests ---

    #[test]
    fn test_stack_item_tool_result() {
        let item = StackItem::ToolResult {
            tool_use_id: "call_1".into(),
            tool_name: "read_file".into(),
        };
        match &item {
            StackItem::ToolResult { tool_use_id, tool_name } => {
                assert_eq!(tool_use_id, "call_1");
                assert_eq!(tool_name, "read_file");
            }
            _ => panic!("Expected ToolResult variant"),
        }
    }

    #[test]
    fn test_stack_item_ask_response() {
        let item = StackItem::AskResponse {
            ask_type: "followup".into(),
        };
        match &item {
            StackItem::AskResponse { ask_type } => {
                assert_eq!(ask_type, "followup");
            }
            _ => panic!("Expected AskResponse variant"),
        }
    }

    #[test]
    fn test_stack_item_sub_task() {
        let item = StackItem::SubTask {
            parent_task_id: "p1".into(),
            child_task_id: "c1".into(),
        };
        match &item {
            StackItem::SubTask { parent_task_id, child_task_id } => {
                assert_eq!(parent_task_id, "p1");
                assert_eq!(child_task_id, "c1");
            }
            _ => panic!("Expected SubTask variant"),
        }
    }
}
