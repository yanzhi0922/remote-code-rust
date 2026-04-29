//! Task engine core logic.
//!
//! Provides [`TaskEngine`] which orchestrates the task lifecycle including
//! state management, loop control, event emission, streaming state, and
//! result computation.
//!
//! Source: `src/core/task/Task.ts` — Task class

use std::collections::HashMap;

use crate::config::validate_config;
use roo_task_persistence::TaskFileSystem;
use crate::events::TaskEventEmitter;
use crate::loop_control::LoopControl;
use crate::state::StateMachine;
use crate::types::{
    AssistantMessageContent, DiffStrategy, StreamingState, TaskConfig, TaskError, TaskResult,
    TaskState, MAX_EXPONENTIAL_BACKOFF_SECONDS,
};
use roo_types::experiment::Experiments;
use roo_types::todo::TodoItem;

// ---------------------------------------------------------------------------
// CachedStreamingModel
// ---------------------------------------------------------------------------

/// Cached model info for the current streaming session.
///
/// Source: `src/core/task/Task.ts` line 398
///   `cachedStreamingModel?: { id: string; info: ModelInfo }`
#[derive(Debug, Clone)]
pub struct CachedStreamingModel {
    /// The model ID.
    pub id: String,
    /// The model information (simplified — in TS this is a full ModelInfo object).
    pub max_tokens: Option<u64>,
    pub context_window: Option<u64>,
    pub supports_images: bool,
    pub supports_computer_use: bool,
}

// ---------------------------------------------------------------------------
// TokenUsageSnapshot
// ---------------------------------------------------------------------------

/// Token usage snapshot for throttling.
///
/// Source: `src/core/task/Task.ts` lines 401-402
///   `private tokenUsageSnapshot?: TokenUsage`
///   `private tokenUsageSnapshotAt?: number`
#[derive(Debug, Clone)]
pub struct TokenUsageSnapshot {
    /// The token usage at the time of the snapshot.
    pub usage: roo_types::message::TokenUsage,
    /// The timestamp (ms since epoch) when the snapshot was taken.
    pub timestamp_ms: u64,
}

// ---------------------------------------------------------------------------
// TaskEngine
// ---------------------------------------------------------------------------

/// The core task engine.
///
/// Manages the task lifecycle: state transitions, loop control, event emission,
/// streaming state, and result aggregation.
///
/// Unlike the TypeScript `Task` class (a ~4700-line god object), this Rust
/// implementation decomposes responsibilities into separate components:
/// - [`StateMachine`] for state transitions
/// - [`LoopControl`] for iteration and mistake limits
/// - [`TaskEventEmitter`] for event-driven communication
/// - [`StreamingState`] for API streaming state
/// - [`TaskResult`] for result aggregation
///
/// The actual agent loop (API call → parse response → tool execution → loop)
/// is coordinated at the application layer using these components.
///
/// ## Task.ts Property Mapping
///
/// | TS Property | Rust Field | Notes |
/// |---|---|---|
/// | `taskId` | `config.task_id` | Immutable |
/// | `rootTaskId` | `config.root_task_id` | Immutable |
/// | `parentTaskId` | `config.parent_task_id` | Immutable |
/// | `taskNumber` | `config.task_number` | Immutable |
/// | `workspacePath` | `config.workspace` | Immutable |
/// | `consecutiveMistakeCount` | `loop_control.consecutive_mistake_count` | Line 320 |
/// | `consecutiveMistakeLimit` | `config.consecutive_mistake_limit` | Line 321 |
/// | `consecutiveMistakeCountForApplyDiff` | `loop_control.consecutive_mistake_count_for_apply_diff` | Line 322 |
/// | `consecutiveMistakeCountForEditFile` | `loop_control.consecutive_mistake_count_for_edit_file` | Line 323 |
/// | `consecutiveNoToolUseCount` | `loop_control.consecutive_no_tool_use_count` | Line 324 |
/// | `consecutiveNoAssistantMessagesCount` | `loop_control.consecutive_no_assistant_messages_count` | Line 325 |
/// | `toolUsage` | `tool_usage` | Line 326 |
/// | `didEditFile` | `did_edit_file` | Line 306 |
/// | `diffStrategy` | `diff_strategy` | Line 305 |
/// | `cachedStreamingModel` | `cached_streaming_model` | Line 398 |
/// | `tokenUsageSnapshot` | `token_usage_snapshot` | Line 401 |
/// | `assistantMessageSavedToHistory` | `streaming.assistant_message_saved_to_history` | Line 361 |
/// | `skipPrevResponseIdOnce` | `skip_prev_response_id_once` | Line 270 |
/// | `assistantMessageContent` | `assistant_message_content` | Line 343 |
/// | `userMessageContent` | `user_message_content` | Line 346 |
/// | `userMessageContentReady` | `user_message_content_ready` | Line 347 |
/// | `streamingToolCallIndices` | `streaming_tool_call_indices` | Line 394 |
/// | `abort` | `abort` | Line 268 |
/// | `didFinishAbortingStream` | `streaming.did_finish_aborting_stream` | Line 277 |
/// | `abandoned` | `abandoned` | Line 278 |
/// | `abortReason` | `abort_reason` | Line 279 |
/// | `isInitialized` | `is_initialized` | Line 280 |
/// | `isPaused` | `is_paused` | Line 281 |
pub struct TaskEngine {
    // --- Core configuration and state ---
    config: TaskConfig,
    state_machine: StateMachine,
    loop_control: LoopControl,
    streaming: StreamingState,
    result: TaskResult,

    // --- API conversation history ---
    /// API conversation history (messages sent to/from the API).
    ///
    /// Source: `src/core/task/Task.ts` line 309 — `apiConversationHistory`
    api_conversation_history: Vec<roo_types::api::ApiMessage>,

    /// UI-facing conversation messages (ClineMessages).
    ///
    /// Source: `src/core/task/Task.ts` line 310 — `clineMessages`
    cline_messages: Vec<roo_types::message::ClineMessage>,

    // --- Task lifecycle flags ---
    /// Whether the task has been initialized.
    ///
    /// Source: `src/core/task/Task.ts` line 280 — `isInitialized`
    is_initialized: bool,

    /// Whether the task has been abandoned (for delegation).
    ///
    /// Source: `src/core/task/Task.ts` line 278 — `abandoned`
    abandoned: bool,

    /// Whether the task is paused.
    ///
    /// Source: `src/core/task/Task.ts` line 281 — `isPaused`
    is_paused: bool,

    /// Abort reason, if any.
    ///
    /// Source: `src/core/task/Task.ts` line 279 — `abortReason`
    abort_reason: Option<String>,

    /// Whether the task has been aborted.
    ///
    /// Source: `src/core/task/Task.ts` line 268 — `abort`
    abort: bool,

    /// Whether to skip the previous response ID once.
    ///
    /// Source: `src/core/task/Task.ts` line 270 — `skipPrevResponseIdOnce`
    skip_prev_response_id_once: bool,

    // --- Tool usage tracking ---
    /// Tool usage tracking (attempts and failures per tool).
    ///
    /// Source: `src/core/task/Task.ts` line 326 — `toolUsage: ToolUsage = {}`
    tool_usage: roo_types::tool::ToolUsage,

    // --- Editing state ---
    /// Whether a file was edited in the current session.
    ///
    /// Source: `src/core/task/Task.ts` line 306 — `didEditFile: boolean = false`
    did_edit_file: bool,

    /// The diff strategy to use for file editing.
    ///
    /// Source: `src/core/task/Task.ts` line 305 — `diffStrategy?: DiffStrategy`
    diff_strategy: Option<DiffStrategy>,

    // --- Streaming model cache ---
    /// Cached model info for the current streaming session.
    /// Set at the start of each API request to prevent excessive getModel() calls.
    ///
    /// Source: `src/core/task/Task.ts` line 398 — `cachedStreamingModel`
    cached_streaming_model: Option<CachedStreamingModel>,

    // --- Token usage snapshot / throttling ---
    /// Token usage snapshot for throttling.
    ///
    /// Source: `src/core/task/Task.ts` line 401 — `tokenUsageSnapshot`
    token_usage_snapshot: Option<TokenUsageSnapshot>,

    // --- Assistant message content ---
    /// The accumulated assistant message content blocks for the current streaming session.
    ///
    /// Source: `src/core/task/Task.ts` line 343 — `assistantMessageContent: AssistantMessageContent[] = []`
    assistant_message_content: Vec<AssistantMessageContent>,

    // --- User message content ---
    /// User message content blocks being assembled for the next API request.
    ///
    /// Source: `src/core/task/Task.ts` line 346 — `userMessageContent`
    user_message_content: Vec<serde_json::Value>,

    /// Whether the user message content is ready to be sent.
    ///
    /// Source: `src/core/task/Task.ts` line 347 — `userMessageContentReady`
    user_message_content_ready: bool,

    // --- Streaming tool call indices ---
    /// Map of tool call IDs to their streaming index.
    ///
    /// Source: `src/core/task/Task.ts` line 394 — `streamingToolCallIndices`
    streaming_tool_call_indices: HashMap<String, usize>,

    // =================================================================
    // New fields — matching TS Task class properties (R22-B1 additions)
    // =================================================================

    /// API configuration (provider settings).
    ///
    /// Source: `src/core/task/Task.ts` line 284 — `apiConfiguration: ProviderSettings`
    api_configuration: Option<roo_types::provider_settings::ProviderSettings>,

    /// API handler instance (provider for making LLM calls).
    ///
    /// Source: `src/core/task/Task.ts` line 285 — `api: ApiHandler`
    /// Note: In the Rust decomposition, the primary provider lives in AgentLoop.
    /// This field allows the engine to optionally hold its own handler reference.
    api_handler: Option<Box<dyn roo_provider::handler::Provider>>,

    /// Todo list for the task.
    ///
    /// Source: `src/core/task/Task.ts` line 172 — `todoList?: TodoItem[]`
    todo_list: Option<Vec<TodoItem>>,

    /// Provider reference (weak reference to the ClineProvider).
    ///
    /// Source: `src/core/task/Task.ts` line 266 — `providerRef: WeakRef<ClineProvider>`
    /// TODO: Replace with actual provider weak reference when ClineProvider is ported.
    /// Using a placeholder `()` type for now.
    provider_ref: Option<()>,

    /// Global storage path for task persistence.
    ///
    /// Source: `src/core/task/Task.ts` line 267 — `globalStoragePath: string`
    global_storage_path: String,

    /// Auto-approval handler for tool execution.
    ///
    /// Source: `src/core/task/Task.ts` line 287 — `autoApprovalHandler: AutoApprovalHandler`
    auto_approval_handler: Option<roo_auto_approval::approval::AutoApprovalHandler>,

    /// Experiment flags.
    ///
    /// Source: `src/core/task/Task.ts` — `experiments` constructor option
    experiments: Experiments,

    /// Initial todo items provided at task creation.
    ///
    /// Source: `src/core/task/Task.ts` — `initialTodos` constructor option
    initial_todos: Vec<TodoItem>,

    /// Initial status for the task's history item.
    ///
    /// Source: `src/core/task/Task.ts` line 159 — `initialStatus?: "active" | "delegated" | "completed"`
    initial_status: Option<String>,

    /// Callback invoked when the task is created.
    ///
    /// Source: `src/core/task/Task.ts` line 155 — `onCreated?: (task: Task) => void`
    /// Called at the end of the constructor: `onCreated?.(this)`
    on_created_callback: Option<Box<dyn FnOnce(&TaskEngine) + Send + Sync>>,
}

impl TaskEngine {
    /// Create a new task engine with the given configuration.
    pub fn new(config: TaskConfig) -> Result<Self, TaskError> {
        validate_config(&config)?;

        let max_iterations = config.max_iterations;
        let task_id = config.task_id.clone();
        let consecutive_mistake_limit = config.consecutive_mistake_limit;
        let global_storage_path = config.storage_path.clone().unwrap_or_default();

        Ok(Self {
            config,
            state_machine: StateMachine::new(),
            loop_control: LoopControl::with_max_iterations(
                consecutive_mistake_limit,
                max_iterations.unwrap_or(usize::MAX),
            ),
            streaming: StreamingState::new(),
            result: TaskResult::new(task_id, TaskState::Idle),
            api_conversation_history: Vec::new(),
            cline_messages: Vec::new(),
            is_initialized: false,
            abandoned: false,
            is_paused: false,
            abort_reason: None,
            abort: false,
            skip_prev_response_id_once: false,
            tool_usage: HashMap::new(),
            did_edit_file: false,
            diff_strategy: None,
            cached_streaming_model: None,
            token_usage_snapshot: None,
            assistant_message_content: Vec::new(),
            user_message_content: Vec::new(),
            user_message_content_ready: false,
            streaming_tool_call_indices: HashMap::new(),

            // --- New fields (R22-B1) ---
            // TS L284: apiConfiguration
            api_configuration: None,
            // TS L285: api (ApiHandler)
            api_handler: None,
            // TS L172: todoList
            todo_list: None,
            // TS L266: providerRef — placeholder
            provider_ref: None,
            // TS L267: globalStoragePath
            global_storage_path,
            // TS L287: autoApprovalHandler
            auto_approval_handler: None,
            // TS constructor: experiments
            experiments: Experiments::default(),
            // TS constructor: initialTodos
            initial_todos: Vec::new(),
            // TS L159: initialStatus
            initial_status: None,
            // TS L155: onCreated callback
            on_created_callback: None,
        })
    }

    // -----------------------------------------------------------------------
    // Core accessors
    // -----------------------------------------------------------------------

    /// Get the current task state.
    pub fn state(&self) -> TaskState {
        self.state_machine.current()
    }

    /// Get a reference to the task configuration.
    pub fn config(&self) -> &TaskConfig {
        &self.config
    }

    /// Get a reference to the task result.
    pub fn result(&self) -> &TaskResult {
        &self.result
    }

    /// Get a reference to the event emitter.
    pub fn emitter(&self) -> &TaskEventEmitter {
        self.state_machine.emitter()
    }

    /// Get a reference to the loop control.
    pub fn loop_control(&self) -> &LoopControl {
        &self.loop_control
    }

    /// Get a mutable reference to the loop control.
    pub fn loop_control_mut(&mut self) -> &mut LoopControl {
        &mut self.loop_control
    }

    /// Get a reference to the streaming state.
    ///
    /// Source: `src/core/task/Task.ts` — streaming-related properties
    pub fn streaming(&self) -> &StreamingState {
        &self.streaming
    }

    /// Get a mutable reference to the streaming state.
    pub fn streaming_mut(&mut self) -> &mut StreamingState {
        &mut self.streaming
    }

    // -----------------------------------------------------------------------
    // API conversation history
    // -----------------------------------------------------------------------

    /// Get a reference to the API conversation history.
    ///
    /// Source: `src/core/task/Task.ts` line 309 — `apiConversationHistory`
    pub fn api_conversation_history(&self) -> &[roo_types::api::ApiMessage] {
        &self.api_conversation_history
    }

    /// Get a mutable reference to the API conversation history.
    pub fn api_conversation_history_mut(&mut self) -> &mut Vec<roo_types::api::ApiMessage> {
        &mut self.api_conversation_history
    }

    /// Add a message to the API conversation history.
    ///
    /// Source: `src/core/task/Task.ts` — `addToApiConversationHistory`
    pub fn add_api_message(&mut self, message: roo_types::api::ApiMessage) {
        self.api_conversation_history.push(message);
    }

    /// Set the API conversation history (e.g., when loading from persistence).
    pub fn set_api_conversation_history(&mut self, history: Vec<roo_types::api::ApiMessage>) {
        self.api_conversation_history = history;
    }

    /// Clear the API conversation history.
    pub fn clear_api_conversation_history(&mut self) {
        self.api_conversation_history.clear();
    }

    // -----------------------------------------------------------------------
    // Cline messages (UI-facing)
    // -----------------------------------------------------------------------

    /// Get a reference to the cline messages (UI-facing messages).
    ///
    /// Source: `src/core/task/Task.ts` line 310 — `clineMessages`
    pub fn cline_messages(&self) -> &[roo_types::message::ClineMessage] {
        &self.cline_messages
    }

    /// Get a mutable reference to the cline messages.
    pub fn cline_messages_mut(&mut self) -> &mut Vec<roo_types::message::ClineMessage> {
        &mut self.cline_messages
    }

    /// Add a cline message.
    pub fn add_cline_message(&mut self, message: roo_types::message::ClineMessage) {
        self.cline_messages.push(message);
    }

    // -----------------------------------------------------------------------
    // Task lifecycle flags
    // -----------------------------------------------------------------------

    /// Check whether the task is initialized.
    ///
    /// Source: `src/core/task/Task.ts` line 280 — `isInitialized`
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Mark the task as initialized.
    pub fn set_initialized(&mut self, initialized: bool) {
        self.is_initialized = initialized;
    }

    /// Check whether the task has been abandoned.
    ///
    /// Source: `src/core/task/Task.ts` line 278 — `abandoned`
    pub fn is_abandoned(&self) -> bool {
        self.abandoned
    }

    /// Mark the task as abandoned (for delegation).
    pub fn set_abandoned(&mut self, abandoned: bool) {
        self.abandoned = abandoned;
    }

    /// Get the abort reason.
    ///
    /// Source: `src/core/task/Task.ts` line 279 — `abortReason`
    pub fn abort_reason(&self) -> Option<&str> {
        self.abort_reason.as_deref()
    }

    /// Check whether the task has been aborted.
    ///
    /// Source: `src/core/task/Task.ts` line 268 — `abort`
    pub fn is_aborted(&self) -> bool {
        self.abort
    }

    /// Check whether the task is paused.
    ///
    /// Source: `src/core/task/Task.ts` line 281 — `isPaused`
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    /// Get/set the `skipPrevResponseIdOnce` flag.
    ///
    /// Source: `src/core/task/Task.ts` line 270 — `skipPrevResponseIdOnce`
    pub fn skip_prev_response_id_once(&self) -> bool {
        self.skip_prev_response_id_once
    }

    /// Set the `skipPrevResponseIdOnce` flag.
    pub fn set_skip_prev_response_id_once(&mut self, value: bool) {
        self.skip_prev_response_id_once = value;
    }

    // -----------------------------------------------------------------------
    // Tool usage tracking
    // -----------------------------------------------------------------------

    /// Get a reference to the tool usage map.
    ///
    /// Source: `src/core/task/Task.ts` line 326 — `toolUsage: ToolUsage = {}`
    pub fn tool_usage(&self) -> &roo_types::tool::ToolUsage {
        &self.tool_usage
    }

    /// Record a tool usage (increment attempts count).
    ///
    /// Source: `src/core/task/Task.ts` — `recordToolUsage()`
    pub fn record_tool_usage(&mut self, tool_name: roo_types::tool::ToolName) {
        let entry = self
            .tool_usage
            .entry(tool_name)
            .or_insert(roo_types::tool::ToolUsageEntry {
                attempts: 0,
                failures: 0,
            });
        entry.attempts += 1;
    }

    /// Record a tool failure (increment failures count).
    pub fn record_tool_failure(&mut self, tool_name: roo_types::tool::ToolName) {
        let entry = self
            .tool_usage
            .entry(tool_name)
            .or_insert(roo_types::tool::ToolUsageEntry {
                attempts: 0,
                failures: 0,
            });
        entry.failures += 1;
    }

    // -----------------------------------------------------------------------
    // Editing state
    // -----------------------------------------------------------------------

    /// Check whether a file was edited in the current session.
    ///
    /// Source: `src/core/task/Task.ts` line 306 — `didEditFile`
    pub fn did_edit_file(&self) -> bool {
        self.did_edit_file
    }

    /// Set the `didEditFile` flag.
    pub fn set_did_edit_file(&mut self, value: bool) {
        self.did_edit_file = value;
    }

    /// Get the diff strategy.
    ///
    /// Source: `src/core/task/Task.ts` line 305 — `diffStrategy`
    pub fn diff_strategy(&self) -> Option<DiffStrategy> {
        self.diff_strategy
    }

    /// Set the diff strategy.
    pub fn set_diff_strategy(&mut self, strategy: Option<DiffStrategy>) {
        self.diff_strategy = strategy;
    }

    // -----------------------------------------------------------------------
    // Cached streaming model
    // -----------------------------------------------------------------------

    /// Get the cached streaming model info.
    ///
    /// Source: `src/core/task/Task.ts` line 398 — `cachedStreamingModel`
    pub fn cached_streaming_model(&self) -> Option<&CachedStreamingModel> {
        self.cached_streaming_model.as_ref()
    }

    /// Set the cached streaming model info.
    pub fn set_cached_streaming_model(&mut self, model: Option<CachedStreamingModel>) {
        self.cached_streaming_model = model;
    }

    // -----------------------------------------------------------------------
    // Token usage snapshot / throttling
    // -----------------------------------------------------------------------

    /// Get the token usage snapshot.
    ///
    /// Source: `src/core/task/Task.ts` line 401 — `tokenUsageSnapshot`
    pub fn token_usage_snapshot(&self) -> Option<&TokenUsageSnapshot> {
        self.token_usage_snapshot.as_ref()
    }

    /// Take a token usage snapshot for throttling.
    pub fn take_token_usage_snapshot(&mut self) {
        self.token_usage_snapshot = Some(TokenUsageSnapshot {
            usage: self.result.token_usage.clone(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        });
    }

    /// Clear the token usage snapshot.
    pub fn clear_token_usage_snapshot(&mut self) {
        self.token_usage_snapshot = None;
    }

    // -----------------------------------------------------------------------
    // Assistant message content
    // -----------------------------------------------------------------------

    /// Get a reference to the assistant message content blocks.
    ///
    /// Source: `src/core/task/Task.ts` line 343 — `assistantMessageContent`
    pub fn assistant_message_content(&self) -> &[AssistantMessageContent] {
        &self.assistant_message_content
    }

    /// Get a mutable reference to the assistant message content blocks.
    pub fn assistant_message_content_mut(&mut self) -> &mut Vec<AssistantMessageContent> {
        &mut self.assistant_message_content
    }

    /// Add an assistant message content block.
    pub fn add_assistant_message_content(&mut self, content: AssistantMessageContent) {
        self.assistant_message_content.push(content);
    }

    /// Clear the assistant message content.
    pub fn clear_assistant_message_content(&mut self) {
        self.assistant_message_content.clear();
    }

    /// Replace an assistant message content block at the given index.
    ///
    /// Source: `src/core/task/Task.ts` — streaming tool call finalization
    /// replaces partial tool_use blocks with complete ones at tracked indices.
    pub fn update_assistant_message_content(&mut self, index: usize, content: AssistantMessageContent) {
        if index < self.assistant_message_content.len() {
            self.assistant_message_content[index] = content;
        }
    }

    /// Mark the assistant message content block at the given index as not partial.
    ///
    /// Source: `src/core/task/Task.ts` L2968-2971 — when finalizeStreamingToolCall
    /// returns null (malformed JSON), the existing tool_use is marked non-partial.
    pub fn mark_assistant_content_not_partial(&mut self, index: usize) {
        if index < self.assistant_message_content.len() {
            match &mut self.assistant_message_content[index] {
                AssistantMessageContent::Text { partial, .. } => *partial = false,
                AssistantMessageContent::ToolUse(tu) => tu.partial = false,
                AssistantMessageContent::McpToolUse(mcp) => mcp.partial = false,
            }
        }
    }

    // -----------------------------------------------------------------------
    // User message content
    // -----------------------------------------------------------------------

    /// Get a reference to the user message content blocks.
    ///
    /// Source: `src/core/task/Task.ts` line 346 — `userMessageContent`
    pub fn user_message_content(&self) -> &[serde_json::Value] {
        &self.user_message_content
    }

    /// Get a mutable reference to the user message content blocks.
    pub fn user_message_content_mut(&mut self) -> &mut Vec<serde_json::Value> {
        &mut self.user_message_content
    }

    /// Check whether the user message content is ready.
    ///
    /// Source: `src/core/task/Task.ts` line 347 — `userMessageContentReady`
    pub fn user_message_content_ready(&self) -> bool {
        self.user_message_content_ready
    }

    /// Set whether the user message content is ready.
    pub fn set_user_message_content_ready(&mut self, ready: bool) {
        self.user_message_content_ready = ready;
    }

    /// Push a tool result to user message content, preventing duplicates.
    ///
    /// Source: `src/core/task/Task.ts` lines 370-383 — `pushToolResultToUserContent`
    pub fn push_tool_result_to_user_content(&mut self, tool_use_id: &str, content: &str, is_error: bool) -> bool {
        // Check for duplicate
        let exists = self.user_message_content.iter().any(|block| {
            if let Some(obj) = block.as_object() {
                obj.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                    && obj.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_use_id)
            } else {
                false
            }
        });

        if exists {
            tracing::warn!(
                "Skipping duplicate tool_result for tool_use_id: {}",
                tool_use_id
            );
            return false;
        }

        let result = serde_json::json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        });
        self.user_message_content.push(result);
        true
    }

    // -----------------------------------------------------------------------
    // Streaming tool call indices
    // -----------------------------------------------------------------------

    /// Get a reference to the streaming tool call indices map.
    ///
    /// Source: `src/core/task/Task.ts` line 394 — `streamingToolCallIndices`
    pub fn streaming_tool_call_indices(&self) -> &HashMap<String, usize> {
        &self.streaming_tool_call_indices
    }

    /// Get a mutable reference to the streaming tool call indices map.
    pub fn streaming_tool_call_indices_mut(&mut self) -> &mut HashMap<String, usize> {
        &mut self.streaming_tool_call_indices
    }

    /// Record a streaming tool call index.
    pub fn record_streaming_tool_call_index(&mut self, tool_call_id: String, index: usize) {
        self.streaming_tool_call_indices.insert(tool_call_id, index);
    }

    /// Clear the streaming tool call indices.
    pub fn clear_streaming_tool_call_indices(&mut self) {
        self.streaming_tool_call_indices.clear();
    }

    // -----------------------------------------------------------------------
    // New fields accessors (R22-B1 additions)
    // Source: `src/core/task/Task.ts` — corresponding TS properties
    // -----------------------------------------------------------------------

    /// Get a reference to the API configuration.
    ///
    /// Source: `src/core/task/Task.ts` line 284 — `apiConfiguration`
    pub fn api_configuration(&self) -> Option<&roo_types::provider_settings::ProviderSettings> {
        self.api_configuration.as_ref()
    }

    /// Set the API configuration.
    pub fn set_api_configuration(&mut self, config: roo_types::provider_settings::ProviderSettings) {
        self.api_configuration = Some(config);
    }

    /// Get a reference to the API handler, if any.
    ///
    /// Source: `src/core/task/Task.ts` line 285 — `api: ApiHandler`
    pub fn api_handler(&self) -> Option<&dyn roo_provider::handler::Provider> {
        self.api_handler.as_deref()
    }

    /// Set the API handler.
    pub fn set_api_handler(&mut self, handler: Box<dyn roo_provider::handler::Provider>) {
        self.api_handler = Some(handler);
    }

    /// Get a reference to the todo list.
    ///
    /// Source: `src/core/task/Task.ts` line 172 — `todoList?: TodoItem[]`
    pub fn todo_list(&self) -> Option<&[TodoItem]> {
        self.todo_list.as_deref()
    }

    /// Get a mutable reference to the todo list.
    pub fn todo_list_mut(&mut self) -> &mut Option<Vec<TodoItem>> {
        &mut self.todo_list
    }

    /// Set the todo list.
    pub fn set_todo_list(&mut self, todos: Vec<TodoItem>) {
        self.todo_list = Some(todos);
    }

    /// Get the global storage path.
    ///
    /// Source: `src/core/task/Task.ts` line 267 — `globalStoragePath`
    pub fn global_storage_path(&self) -> &str {
        &self.global_storage_path
    }

    /// Set the global storage path.
    pub fn set_global_storage_path(&mut self, path: String) {
        self.global_storage_path = path;
    }

    /// Get a reference to the auto-approval handler, if any.
    ///
    /// Source: `src/core/task/Task.ts` line 287 — `autoApprovalHandler`
    pub fn auto_approval_handler(&self) -> Option<&roo_auto_approval::approval::AutoApprovalHandler> {
        self.auto_approval_handler.as_ref()
    }

    /// Get a mutable reference to the auto-approval handler, if any.
    pub fn auto_approval_handler_mut(&mut self) -> Option<&mut roo_auto_approval::approval::AutoApprovalHandler> {
        self.auto_approval_handler.as_mut()
    }

    /// Set the auto-approval handler.
    pub fn set_auto_approval_handler(&mut self, handler: roo_auto_approval::approval::AutoApprovalHandler) {
        self.auto_approval_handler = Some(handler);
    }

    /// Get a reference to the experiments configuration.
    ///
    /// Source: `src/core/task/Task.ts` — `experiments` constructor option
    pub fn experiments(&self) -> &Experiments {
        &self.experiments
    }

    /// Set the experiments configuration.
    pub fn set_experiments(&mut self, experiments: Experiments) {
        self.experiments = experiments;
    }

    /// Get a reference to the initial todos.
    ///
    /// Source: `src/core/task/Task.ts` — `initialTodos` constructor option
    pub fn initial_todos(&self) -> &[TodoItem] {
        &self.initial_todos
    }

    /// Set the initial todos.
    pub fn set_initial_todos(&mut self, todos: Vec<TodoItem>) {
        self.initial_todos = todos;
    }

    /// Get the initial status.
    ///
    /// Source: `src/core/task/Task.ts` line 159 — `initialStatus`
    pub fn initial_status(&self) -> Option<&str> {
        self.initial_status.as_deref()
    }

    /// Set the initial status.
    pub fn set_initial_status(&mut self, status: Option<String>) {
        self.initial_status = status;
    }

    /// Set the on_created callback.
    ///
    /// Source: `src/core/task/Task.ts` line 155 — `onCreated?: (task: Task) => void`
    pub fn set_on_created_callback(&mut self, callback: Box<dyn FnOnce(&TaskEngine) + Send + Sync>) {
        self.on_created_callback = Some(callback);
    }

    /// Invoke the on_created callback, if set.
    ///
    /// Source: `src/core/task/Task.ts` line 566 — `onCreated?.(this)`
    pub fn invoke_on_created(mut self) {
        if let Some(callback) = self.on_created_callback.take() {
            callback(&self);
        }
    }

    // -----------------------------------------------------------------------
    // State transitions
    // -----------------------------------------------------------------------

    /// Start the task.
    pub fn start(&mut self) -> Result<TaskState, TaskError> {
        let state = self.state_machine.start()?;
        self.result.status = state;
        Ok(state)
    }

    /// Pause the task.
    pub fn pause(&mut self) -> Result<TaskState, TaskError> {
        let state = self.state_machine.pause()?;
        self.is_paused = true;
        self.result.status = state;
        Ok(state)
    }

    /// Resume the task.
    pub fn resume(&mut self) -> Result<TaskState, TaskError> {
        let state = self.state_machine.resume()?;
        self.is_paused = false;
        self.result.status = state;
        Ok(state)
    }

    /// Complete the task.
    pub fn complete(&mut self) -> Result<TaskState, TaskError> {
        let state = self.state_machine.complete()?;
        self.result.status = state;
        Ok(state)
    }

    /// Abort the task.
    pub fn abort(&mut self) -> Result<TaskState, TaskError> {
        self.loop_control.cancel();
        self.abort = true;
        self.abort_reason = Some("user_cancelled".to_string());
        let state = self.state_machine.abort()?;
        self.result.status = state;
        Ok(state)
    }

    /// Abort the task with a specific reason.
    ///
    /// Source: `src/core/task/Task.ts` — `abortReason` can be various values
    /// like "user_cancelled", "rate_limit_hit", "max_tokens_exceeded", etc.
    pub fn abort_with_reason(&mut self, reason: &str) -> Result<TaskState, TaskError> {
        self.loop_control.cancel();
        self.abort = true;
        self.abort_reason = Some(reason.to_string());
        let state = self.state_machine.abort()?;
        self.result.status = state;
        Ok(state)
    }

    /// Delegate the task.
    pub fn delegate(&mut self) -> Result<TaskState, TaskError> {
        let state = self.state_machine.delegate()?;
        self.result.status = state;
        Ok(state)
    }

    // -----------------------------------------------------------------------
    // Loop control
    // -----------------------------------------------------------------------

    /// Check whether the task loop should continue.
    pub fn should_continue(&self) -> bool {
        self.loop_control.should_continue()
    }

    /// Advance one iteration.
    ///
    /// Returns `true` if the iteration limit was reached.
    pub fn advance_iteration(&mut self) -> bool {
        let reached = self.loop_control.increment_iteration();
        self.result.iterations = self.loop_control.current_iteration;
        reached
    }

    /// Record a tool execution.
    pub fn record_tool_execution(&mut self, tool_name: &str, success: bool) {
        self.result.record_tool_usage(tool_name);
        self.state_machine.emitter().emit_tool_executed(tool_name, success);

        if success {
            self.loop_control.reset_mistake_count();
        } else {
            let exceeded = self.loop_control.record_mistake();
            if exceeded {
                self.state_machine.emitter().emit_state_changed(
                    self.state_machine.current(),
                    TaskState::Aborted,
                );
            }
        }
    }

    /// Record a mistake without tool execution.
    pub fn record_mistake(&mut self) -> bool {
        self.loop_control.record_mistake()
    }

    /// Reset the mistake count.
    pub fn reset_mistakes(&mut self) {
        self.loop_control.reset_mistake_count();
    }

    /// Set the final message.
    pub fn set_final_message(&mut self, message: String) {
        self.result.final_message = Some(message);
    }

    /// Update token usage in the result.
    pub fn update_token_usage(&mut self, usage: roo_types::message::TokenUsage) {
        self.state_machine.emitter().emit_token_usage_updated(usage.clone());
        self.result.token_usage = usage;
    }

    // -----------------------------------------------------------------------
    // Streaming state management
    // -----------------------------------------------------------------------

    /// Prepare for a new API request by resetting streaming state.
    ///
    /// Source: `src/core/task/Task.ts` — streaming state reset block in
    /// `recursivelyMakeClineRequests`
    pub fn prepare_for_new_api_request(&mut self) {
        self.streaming.reset_for_new_request();
        self.assistant_message_content.clear();
        self.user_message_content.clear();
        self.user_message_content_ready = false;
        self.streaming_tool_call_indices.clear();
        self.cached_streaming_model = None;
    }

    /// Calculate exponential backoff delay for retry attempts.
    ///
    /// Returns the delay in milliseconds.
    ///
    /// Source: `src/core/task/Task.ts` — `MAX_EXPONENTIAL_BACKOFF_SECONDS`
    pub fn calculate_backoff_delay(retry_attempt: u32) -> u64 {
        let base_delay = 1000u64; // 1 second base
        let delay = base_delay * 2u64.pow(retry_attempt);
        delay.min(MAX_EXPONENTIAL_BACKOFF_SECONDS * 1000)
    }

    /// Resume the task after delegation.
    ///
    /// Clears ask states, resets abort/streaming flags, and prepares for
    /// the next API call.
    ///
    /// Source: `src/core/task/Task.ts` — `resumeAfterDelegation`
    pub fn resume_after_delegation(&mut self) -> Result<TaskState, TaskError> {
        // Reset abort and streaming state
        self.loop_control.reset_turn();
        self.abandoned = false;
        self.abort = false;
        self.abort_reason = None;
        self.is_paused = false;
        self.streaming.reset_for_new_request();
        self.streaming.did_finish_aborting_stream = false;
        self.streaming.is_streaming = false;
        self.streaming.is_waiting_for_first_chunk = false;

        // Transition back to running
        let state = self.state_machine.resume()?;
        self.result.status = state;
        self.is_initialized = true;
        Ok(state)
    }

    // -----------------------------------------------------------------------
    // Context management
    // -----------------------------------------------------------------------

    /// Truncate conversation history by removing messages and inserting a marker.
    ///
    /// Removes `remove_count` messages starting from index 1 (preserving the
    /// first/system message), then inserts the truncation marker at index 1.
    ///
    /// This ensures the conversation history stays within the model's context
    /// window while preserving the most recent messages.
    pub fn truncate_history(&mut self, remove_count: usize, marker: roo_types::api::ApiMessage) {
        if remove_count > 0 && remove_count < self.api_conversation_history.len() {
            // Remove messages from index 1 to 1+remove_count (keep first message)
            self.api_conversation_history.drain(1..1 + remove_count);
            // Insert truncation marker at index 1
            self.api_conversation_history.insert(1, marker);
        }
    }

    /// Force truncate context to keep only a ratio of messages.
    ///
    /// Used when the API returns a `context_length_exceeded` error. Removes
    /// messages from the middle of the history, keeping the first message
    /// (system) and the most recent messages.
    ///
    /// Source: `src/core/task/Task.ts` — `FORCED_CONTEXT_REDUCTION_PERCENT`
    pub fn force_truncate_context(&mut self, keep_ratio: f64) {
        let total = self.api_conversation_history.len();
        if total <= 2 {
            // Not enough messages to truncate
            return;
        }
        let keep = ((total as f64) * keep_ratio) as usize;
        let remove = total.saturating_sub(keep);
        if remove > 1 {
            // Remove from index 1, keeping first (system) and last messages
            self.api_conversation_history.drain(1..1 + remove);
            // Insert a truncation notice
            let marker = roo_types::api::ApiMessage {
                role: roo_types::api::MessageRole::User,
                content: vec![roo_types::api::ContentBlock::Text {
                    text: "[CONTEXT WINDOW EXCEEDED — Earlier conversation history has been removed to fit within the context window.]".to_string(),
                }],
                reasoning: None,
                ts: Some(std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as f64),
                truncation_parent: None,
                is_truncation_marker: Some(true),
                truncation_id: Some(uuid::Uuid::now_v7().to_string()),
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            };
            self.api_conversation_history.insert(1, marker);
        }
    }

    /// Append environment context to the last user message or create a new one.
    ///
    /// Source: `src/core/task/Task.ts` — `getEnvironmentDetails()` injection
    pub fn add_environment_context(&mut self, details: &str) {
        if let Some(last_msg) = self.api_conversation_history.last_mut() {
            if last_msg.role == roo_types::api::MessageRole::User {
                last_msg.content.push(roo_types::api::ContentBlock::Text {
                    text: format!("\n\n<environment_details>\n{}\n</environment_details>", details),
                });
                return;
            }
        }
        // No last user message — create a new one
        self.api_conversation_history.push(roo_types::api::ApiMessage {
            role: roo_types::api::MessageRole::User,
            content: vec![roo_types::api::ContentBlock::Text {
                text: format!("<environment_details>\n{}\n</environment_details>", details),
            }],
            reasoning: None,
            ts: Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as f64),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        });
    }

    // -----------------------------------------------------------------------
    // Finalization
    // -----------------------------------------------------------------------

    /// Finalize the task and return the result.
    pub fn finalize(mut self) -> TaskResult {
        if self.state_machine.current() == TaskState::Running {
            // Force to completed if still running
            let _ = self.state_machine.complete();
            self.result.status = TaskState::Completed;
        }
        self.result
    }

    /// Build a result snapshot without consuming the engine.
    ///
    /// This is useful for the agent loop which needs to return a result
    /// while keeping the engine alive during execution.
    pub fn build_result_snapshot(&self) -> TaskResult {
        self.result.clone()
    }

    /// Complete the task if still running and return a result snapshot.
    ///
    /// Combines `complete()` (if needed) and `build_result_snapshot()`.
    pub fn finalize_in_place(&mut self) -> TaskResult {
        if self.state_machine.current() == TaskState::Running {
            let _ = self.state_machine.complete();
            self.result.status = TaskState::Completed;
        }
        self.result.clone()
    }

    // -----------------------------------------------------------------------
    // Persistence
    // -----------------------------------------------------------------------
    // Source: TS `Task.ts` — `saveClineMessages()`, `saveApiConversationHistory()`,
    // `getSavedApiConversationHistory()`, etc.

    /// Save cline messages (UI-facing messages) to persistent storage.
    ///
    /// Source: TS `Task.ts` — `saveClineMessages()`
    pub async fn save_cline_messages(&self) -> Result<(), TaskError> {
        let storage_path = match &self.config.storage_path {
            Some(p) => p,
            None => {
                tracing::debug!("save_cline_messages: no storage_path configured, skipping");
                return Ok(());
            }
        };

        let base = std::path::Path::new(storage_path);
        let path = roo_task_persistence::messages_path(base, &self.config.task_id);
        let fs = roo_task_persistence::OsFileSystem;

        tracing::debug!(
            count = self.cline_messages.len(),
            path = %path.display(),
            "save_cline_messages"
        );

        roo_task_persistence::save_task_messages(&fs, &path, &self.cline_messages)
            .map_err(|e| TaskError::General(format!("Failed to save cline messages: {}", e)))?;

        Ok(())
    }

    /// Save API conversation history to persistent storage.
    ///
    /// Source: TS `Task.ts` — `saveApiConversationHistory()`
    pub async fn save_api_conversation_history(&self) -> Result<(), TaskError> {
        let storage_path = match &self.config.storage_path {
            Some(p) => p,
            None => {
                tracing::debug!("save_api_conversation_history: no storage_path configured, skipping");
                return Ok(());
            }
        };

        let base = std::path::Path::new(storage_path);
        let path = roo_task_persistence::api_messages_path(base, &self.config.task_id);
        let fs = roo_task_persistence::OsFileSystem;

        tracing::debug!(
            count = self.api_conversation_history.len(),
            path = %path.display(),
            "save_api_conversation_history"
        );

        roo_task_persistence::save_api_messages(&fs, &path, &self.api_conversation_history)
            .map_err(|e| TaskError::General(format!("Failed to save API conversation history: {}", e)))?;

        Ok(())
    }

    /// Load API conversation history from persistent storage.
    ///
    /// Source: TS `Task.ts` — `getSavedApiConversationHistory()`
    pub async fn load_api_conversation_history(&mut self) -> Result<(), TaskError> {
        let storage_path = match &self.config.storage_path {
            Some(p) => p,
            None => {
                tracing::debug!("load_api_conversation_history: no storage_path configured, skipping");
                return Ok(());
            }
        };

        let base = std::path::Path::new(storage_path);
        let path = roo_task_persistence::api_messages_path(base, &self.config.task_id);
        let fs = roo_task_persistence::OsFileSystem;

        tracing::debug!(path = %path.display(), "load_api_conversation_history");

        let messages = roo_task_persistence::read_api_messages(&fs, &path)
            .map_err(|e| TaskError::General(format!("Failed to load API conversation history: {}", e)))?;

        tracing::debug!(count = messages.len(), "load_api_conversation_history: loaded");
        self.api_conversation_history = messages;

        Ok(())
    }

    /// Save task metadata (state, result, config) to persistent storage.
    ///
    /// Source: TS `Task.ts` — `saveTask()`
    pub async fn save_task(&self) -> Result<(), TaskError> {
        let storage_path = match &self.config.storage_path {
            Some(p) => p,
            None => {
                tracing::debug!("save_task: no storage_path configured, skipping");
                return Ok(());
            }
        };

        let base = std::path::Path::new(storage_path);
        let fs = roo_task_persistence::OsFileSystem;

        // Ensure the task directory exists
        roo_task_persistence::ensure_task_dir(&fs, base, &self.config.task_id)
            .map_err(|e| TaskError::General(format!("Failed to create task directory: {}", e)))?;

        // Compute and save metadata
        let opts = roo_task_persistence::TaskMetadataOptions {
            task_id: self.config.task_id.clone(),
            root_task_id: self.config.root_task_id.clone(),
            parent_task_id: self.config.parent_task_id.clone(),
            task_number: self.config.task_number,
            global_storage_path: base.to_path_buf(),
            messages: self.cline_messages.clone(),
            workspace: if self.config.workspace.is_empty() {
                self.config.cwd.clone()
            } else {
                self.config.workspace.clone()
            },
            mode: Some(self.config.mode.clone()),
            api_config_name: self.config.api_config_name.clone(),
            initial_status: task_state_to_persistence_status(self.state_machine.current()),
        };

        let metadata = roo_task_persistence::compute_task_metadata(&fs, &opts)
            .map_err(|e| TaskError::General(format!("Failed to compute task metadata: {}", e)))?;

        let meta_path = roo_task_persistence::metadata_path(base, &self.config.task_id);
        let content = serde_json::to_string_pretty(&metadata)
            .map_err(|e| TaskError::General(format!("Failed to serialize task metadata: {}", e)))?;

        fs.write_file(&meta_path, &content)
            .map_err(|e| TaskError::General(format!("Failed to write task metadata: {}", e)))?;

        tracing::debug!(task_id = %self.config.task_id, "save_task: metadata saved");

        Ok(())
    }
}

/// Convert a [`TaskState`] (engine-level) to a [`PersistenceTaskStatus`].
///
/// This bridges the gap between the task engine's state enum and the
/// persistence layer's status enum.
fn task_state_to_persistence_status(state: TaskState) -> roo_task_persistence::PersistenceTaskStatus {
    match state {
        TaskState::Idle | TaskState::Running | TaskState::Paused => {
            roo_task_persistence::PersistenceTaskStatus::Active
        }
        TaskState::Completed => roo_task_persistence::PersistenceTaskStatus::Completed,
        TaskState::Aborted => roo_task_persistence::PersistenceTaskStatus::Aborted,
        TaskState::Delegated => roo_task_persistence::PersistenceTaskStatus::Delegated,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> TaskEngine {
        TaskEngine::new(
            TaskConfig::new("test-task", "/tmp/work")
                .with_mode("code")
                .with_max_iterations(100)
                .with_start_task(false),
        )
        .unwrap()
    }

    #[test]
    fn test_engine_new() {
        let engine = make_engine();
        assert_eq!(engine.state(), TaskState::Idle);
        assert_eq!(engine.config().task_id, "test-task");
        assert!(!engine.is_initialized());
        assert!(!engine.is_abandoned());
        assert!(engine.abort_reason().is_none());
        assert!(engine.api_conversation_history().is_empty());
        assert!(!engine.is_aborted());
        assert!(!engine.did_edit_file());
        assert!(engine.diff_strategy().is_none());
        assert!(engine.cached_streaming_model().is_none());
        assert!(engine.token_usage_snapshot().is_none());
        assert!(engine.assistant_message_content().is_empty());
        assert!(engine.user_message_content().is_empty());
        assert!(!engine.user_message_content_ready());
        assert!(engine.streaming_tool_call_indices().is_empty());
        assert!(!engine.skip_prev_response_id_once());
    }

    #[test]
    fn test_engine_start() {
        let mut engine = make_engine();
        let state = engine.start().unwrap();
        assert_eq!(state, TaskState::Running);
    }

    #[test]
    fn test_engine_full_lifecycle() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.complete().unwrap();
        assert_eq!(engine.state(), TaskState::Completed);
    }

    #[test]
    fn test_engine_pause_resume() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.pause().unwrap();
        assert_eq!(engine.state(), TaskState::Paused);
        engine.resume().unwrap();
        assert_eq!(engine.state(), TaskState::Running);
    }

    #[test]
    fn test_engine_abort() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.abort().unwrap();
        assert_eq!(engine.state(), TaskState::Aborted);
        assert!(!engine.should_continue());
        assert!(engine.is_aborted());
        assert_eq!(engine.abort_reason(), Some("user_cancelled"));
    }

    #[test]
    fn test_engine_delegate() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.delegate().unwrap();
        assert_eq!(engine.state(), TaskState::Delegated);
    }

    #[test]
    fn test_engine_advance_iteration() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.advance_iteration();
        engine.advance_iteration();
        assert_eq!(engine.result().iterations, 2);
    }

    #[test]
    fn test_engine_record_tool_execution_success() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.record_tool_execution("read_file", true);
        assert_eq!(engine.result().tool_usage["read_file"], 1);
    }

    #[test]
    fn test_engine_record_tool_execution_failure() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.record_tool_execution("write_to_file", false);
        assert_eq!(engine.result().tool_usage["write_to_file"], 1);
        assert_eq!(engine.loop_control().consecutive_mistake_count, 1);
    }

    #[test]
    fn test_engine_record_multiple_tool_usages() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.record_tool_execution("read_file", true);
        engine.record_tool_execution("read_file", true);
        engine.record_tool_execution("write_to_file", true);
        assert_eq!(engine.result().tool_usage["read_file"], 2);
        assert_eq!(engine.result().tool_usage["write_to_file"], 1);
    }

    #[test]
    fn test_engine_set_final_message() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.set_final_message("Task completed successfully".to_string());
        assert_eq!(
            engine.result().final_message,
            Some("Task completed successfully".to_string())
        );
    }

    #[test]
    fn test_engine_finalize() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.advance_iteration();
        engine.set_final_message("Done".to_string());
        let result = engine.finalize();
        assert_eq!(result.status, TaskState::Completed);
        assert_eq!(result.final_message, Some("Done".to_string()));
        assert_eq!(result.iterations, 1);
    }

    #[test]
    fn test_engine_finalize_already_completed() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.complete().unwrap();
        let result = engine.finalize();
        assert_eq!(result.status, TaskState::Completed);
    }

    #[test]
    fn test_engine_invalid_config() {
        let config = TaskConfig::new("", "/tmp/work").with_mode("code");
        let result = TaskEngine::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_engine_iteration_limit() {
        let mut engine = TaskEngine::new(
            TaskConfig::new("limited-task", "/tmp/work")
                .with_mode("code")
                .with_max_iterations(2)
                .with_start_task(false),
        )
        .unwrap();
        engine.start().unwrap();
        engine.advance_iteration(); // 1
        assert!(engine.should_continue());
        engine.advance_iteration(); // 2 = limit
        assert!(!engine.should_continue());
    }

    #[test]
    fn test_engine_update_token_usage() {
        let mut engine = make_engine();
        engine.start().unwrap();
        let usage = roo_types::message::TokenUsage {
            total_tokens_in: 1000,
            total_tokens_out: 500,
            total_cache_writes: Some(100),
            total_cache_reads: Some(50),
            total_cost: 1.5,
            context_tokens: 2000,
        };
        engine.update_token_usage(usage);
        assert_eq!(engine.result().token_usage.total_tokens_in, 1000);
        assert_eq!(engine.result().token_usage.total_cost, 1.5);
    }

    #[test]
    fn test_engine_event_emission() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut engine = make_engine();
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        engine.emitter().on(move |event| {
            if let crate::events::TaskEvent::StateChanged { from, to } = event {
                if *from == TaskState::Idle && *to == TaskState::Running {
                    count_clone.fetch_add(1, Ordering::SeqCst);
                }
            }
        });

        engine.start().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // --- Streaming state tests ---

    #[test]
    fn test_engine_streaming_state() {
        let mut engine = make_engine();
        assert!(!engine.streaming().is_streaming);

        engine.streaming_mut().is_streaming = true;
        assert!(engine.streaming().is_streaming);
    }

    #[test]
    fn test_engine_prepare_for_new_api_request() {
        let mut engine = make_engine();
        engine.streaming_mut().assistant_message_saved_to_history = true;
        engine.streaming_mut().current_streaming_content_index = 5;
        engine.streaming_mut().did_tool_fail_in_current_turn = true;
        engine.add_assistant_message_content(AssistantMessageContent::Text {
            content: "test".to_string(),
            partial: true,
        });
        engine.set_user_message_content_ready(true);

        engine.prepare_for_new_api_request();

        assert!(!engine.streaming().assistant_message_saved_to_history);
        assert_eq!(engine.streaming().current_streaming_content_index, 0);
        assert!(!engine.streaming().did_tool_fail_in_current_turn);
        assert!(engine.assistant_message_content().is_empty());
        assert!(engine.user_message_content().is_empty());
        assert!(!engine.user_message_content_ready());
    }

    // --- API conversation history tests ---

    #[test]
    fn test_engine_api_conversation_history() {
        let mut engine = make_engine();
        assert!(engine.api_conversation_history().is_empty());

        let msg = roo_types::api::ApiMessage {
            role: roo_types::api::MessageRole::User,
            content: vec![roo_types::api::ContentBlock::Text {
                text: "Hello".to_string(),
            }],
            reasoning: None,
            ts: Some(1000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        };

        engine.add_api_message(msg.clone());
        assert_eq!(engine.api_conversation_history().len(), 1);

        engine.clear_api_conversation_history();
        assert!(engine.api_conversation_history().is_empty());
    }

    // --- Tool usage tracking tests ---

    #[test]
    fn test_engine_tool_usage_tracking() {
        let mut engine = make_engine();
        engine.start().unwrap();

        engine.record_tool_usage(roo_types::tool::ToolName::ReadFile);
        engine.record_tool_usage(roo_types::tool::ToolName::ReadFile);
        engine.record_tool_failure(roo_types::tool::ToolName::ReadFile);

        let usage = engine.tool_usage();
        let read_entry = usage.get(&roo_types::tool::ToolName::ReadFile).unwrap();
        assert_eq!(read_entry.attempts, 2);
        assert_eq!(read_entry.failures, 1);
    }

    // --- Editing state tests ---

    #[test]
    fn test_engine_editing_state() {
        let mut engine = make_engine();
        assert!(!engine.did_edit_file());
        engine.set_did_edit_file(true);
        assert!(engine.did_edit_file());

        assert!(engine.diff_strategy().is_none());
        engine.set_diff_strategy(Some(DiffStrategy::MultiSearchReplace));
        assert_eq!(engine.diff_strategy(), Some(DiffStrategy::MultiSearchReplace));
    }

    // --- Cached streaming model tests ---

    #[test]
    fn test_engine_cached_streaming_model() {
        let mut engine = make_engine();
        assert!(engine.cached_streaming_model().is_none());

        engine.set_cached_streaming_model(Some(CachedStreamingModel {
            id: "gpt-4".to_string(),
            max_tokens: Some(8192),
            context_window: Some(128000),
            supports_images: true,
            supports_computer_use: false,
        }));

        let model = engine.cached_streaming_model().unwrap();
        assert_eq!(model.id, "gpt-4");
        assert_eq!(model.max_tokens, Some(8192));
    }

    // --- Token usage snapshot tests ---

    #[test]
    fn test_engine_token_usage_snapshot() {
        let mut engine = make_engine();
        assert!(engine.token_usage_snapshot().is_none());

        engine.take_token_usage_snapshot();
        assert!(engine.token_usage_snapshot().is_some());

        engine.clear_token_usage_snapshot();
        assert!(engine.token_usage_snapshot().is_none());
    }

    // --- Assistant message content tests ---

    #[test]
    fn test_engine_assistant_message_content() {
        let mut engine = make_engine();
        assert!(engine.assistant_message_content().is_empty());

        engine.add_assistant_message_content(AssistantMessageContent::Text {
            content: "Hello".to_string(),
            partial: false,
        });
        assert_eq!(engine.assistant_message_content().len(), 1);

        engine.clear_assistant_message_content();
        assert!(engine.assistant_message_content().is_empty());
    }

    // --- User message content tests ---

    #[test]
    fn test_engine_user_message_content() {
        let mut engine = make_engine();
        assert!(engine.user_message_content().is_empty());
        assert!(!engine.user_message_content_ready());

        engine.user_message_content_mut().push(serde_json::json!({"type": "text", "text": "hello"}));
        assert_eq!(engine.user_message_content().len(), 1);

        engine.set_user_message_content_ready(true);
        assert!(engine.user_message_content_ready());
    }

    #[test]
    fn test_engine_push_tool_result_no_duplicates() {
        let mut engine = make_engine();
        assert!(engine.push_tool_result_to_user_content("call_1", "result", false));
        assert_eq!(engine.user_message_content().len(), 1);

        // Duplicate should be rejected
        assert!(!engine.push_tool_result_to_user_content("call_1", "result2", false));
        assert_eq!(engine.user_message_content().len(), 1);

        // Different ID should be accepted
        assert!(engine.push_tool_result_to_user_content("call_2", "result3", true));
        assert_eq!(engine.user_message_content().len(), 2);
    }

    // --- Streaming tool call indices tests ---

    #[test]
    fn test_engine_streaming_tool_call_indices() {
        let mut engine = make_engine();
        assert!(engine.streaming_tool_call_indices().is_empty());

        engine.record_streaming_tool_call_index("call_1".to_string(), 0);
        engine.record_streaming_tool_call_index("call_2".to_string(), 1);
        assert_eq!(engine.streaming_tool_call_indices().len(), 2);
        assert_eq!(engine.streaming_tool_call_indices()["call_1"], 0);

        engine.clear_streaming_tool_call_indices();
        assert!(engine.streaming_tool_call_indices().is_empty());
    }

    // --- Skip prev response ID tests ---

    #[test]
    fn test_engine_skip_prev_response_id_once() {
        let mut engine = make_engine();
        assert!(!engine.skip_prev_response_id_once());

        engine.set_skip_prev_response_id_once(true);
        assert!(engine.skip_prev_response_id_once());
    }

    // --- Initialization and abandonment tests ---

    #[test]
    fn test_engine_initialization() {
        let mut engine = make_engine();
        assert!(!engine.is_initialized());
        engine.set_initialized(true);
        assert!(engine.is_initialized());
    }

    #[test]
    fn test_engine_abandonment() {
        let mut engine = make_engine();
        assert!(!engine.is_abandoned());
        engine.set_abandoned(true);
        assert!(engine.is_abandoned());
    }

    // --- Backoff calculation tests ---

    #[test]
    fn test_calculate_backoff_delay() {
        assert_eq!(TaskEngine::calculate_backoff_delay(0), 1000); // 1s
        assert_eq!(TaskEngine::calculate_backoff_delay(1), 2000); // 2s
        assert_eq!(TaskEngine::calculate_backoff_delay(2), 4000); // 4s
        assert_eq!(TaskEngine::calculate_backoff_delay(3), 8000); // 8s
        assert_eq!(TaskEngine::calculate_backoff_delay(10), MAX_EXPONENTIAL_BACKOFF_SECONDS * 1000); // capped
    }

    // --- Resume after delegation tests ---

    #[test]
    fn test_engine_resume_after_delegation() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.pause().unwrap();

        let state = engine.resume_after_delegation().unwrap();
        assert_eq!(state, TaskState::Running);
        assert!(engine.is_initialized());
        assert!(!engine.is_abandoned());
        assert!(!engine.is_aborted());
    }

    // --- Mistake limit tests ---

    #[test]
    fn test_engine_mistake_limit_with_new_config() {
        let mut engine = TaskEngine::new(
            TaskConfig::new("test", "/tmp")
                .with_mode("code")
                .with_consecutive_mistake_limit(2)
                .with_start_task(false),
        )
        .unwrap();

        engine.start().unwrap();

        // First mistake
        engine.record_tool_execution("tool1", false);
        assert!(engine.should_continue()); // 1 < 2

        // Second mistake hits the limit (>= comparison)
        engine.record_tool_execution("tool1", false);
        assert!(!engine.should_continue()); // 2 >= 2 → STOP
    }

    // --- Cline messages tests ---

    #[test]
    fn test_engine_cline_messages() {
        let mut engine = make_engine();
        assert!(engine.cline_messages().is_empty());

        let msg = roo_types::message::ClineMessage {
            ts: 1700000000.0,
            r#type: roo_types::message::MessageType::Say,
            ask: None,
            say: Some(roo_types::message::ClineSay::Text),
            text: Some("Hello".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        };

        engine.add_cline_message(msg);
        assert_eq!(engine.cline_messages().len(), 1);
    }

    // --- is_paused tests ---

    #[test]
    fn test_engine_is_paused() {
        let mut engine = make_engine();
        assert!(!engine.is_paused());

        engine.start().unwrap();
        assert!(!engine.is_paused());

        engine.pause().unwrap();
        assert!(engine.is_paused());

        engine.resume().unwrap();
        assert!(!engine.is_paused());
    }

    // --- abort_with_reason tests ---

    #[test]
    fn test_engine_abort_with_reason() {
        let mut engine = make_engine();
        engine.start().unwrap();

        engine.abort_with_reason("rate_limit_hit").unwrap();
        assert_eq!(engine.state(), TaskState::Aborted);
        assert_eq!(engine.abort_reason(), Some("rate_limit_hit"));
        assert!(!engine.should_continue());
        assert!(engine.is_aborted());
    }

    // --- resume_after_delegation resets is_paused ---

    #[test]
    fn test_engine_resume_after_delegation_resets_paused() {
        let mut engine = make_engine();
        engine.start().unwrap();
        engine.pause().unwrap();
        assert!(engine.is_paused());

        engine.resume_after_delegation().unwrap();
        assert!(!engine.is_paused());
        assert_eq!(engine.state(), TaskState::Running);
    }

    // --- Persistence tests ---

    #[tokio::test]
    async fn test_persistence_noop_without_storage_path() {
        let mut engine = make_engine();
        // No storage_path configured — all persistence methods should be no-ops
        assert!(engine.save_cline_messages().await.is_ok());
        assert!(engine.save_api_conversation_history().await.is_ok());
        assert!(engine.load_api_conversation_history().await.is_ok());
        assert!(engine.save_task().await.is_ok());
    }

    #[tokio::test]
    async fn test_save_and_load_api_conversation_history() {
        let dir = tempfile::tempdir().unwrap();
        let storage_path = dir.path().to_string_lossy().to_string();

        let mut engine = TaskEngine::new(
            TaskConfig::new("persist-test", "/tmp/work")
                .with_mode("code")
                .with_max_iterations(100)
                .with_storage_path(&storage_path)
                .with_start_task(false),
        )
        .unwrap();

        // Add some API messages
        engine.add_api_message(roo_types::api::ApiMessage {
            role: roo_types::api::MessageRole::User,
            content: vec![roo_types::api::ContentBlock::Text {
                text: "Hello".to_string(),
            }],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        });

        // Save
        engine.save_api_conversation_history().await.unwrap();

        // Create a new engine and load
        let mut engine2 = TaskEngine::new(
            TaskConfig::new("persist-test", "/tmp/work")
                .with_mode("code")
                .with_max_iterations(100)
                .with_storage_path(&storage_path)
                .with_start_task(false),
        )
        .unwrap();

        engine2.load_api_conversation_history().await.unwrap();
        assert_eq!(engine2.api_conversation_history().len(), 1);
    }

    #[tokio::test]
    async fn test_save_cline_messages() {
        let dir = tempfile::tempdir().unwrap();
        let storage_path = dir.path().to_string_lossy().to_string();

        let mut engine = TaskEngine::new(
            TaskConfig::new("cline-test", "/tmp/work")
                .with_mode("code")
                .with_max_iterations(100)
                .with_storage_path(&storage_path)
                .with_start_task(false),
        )
        .unwrap();

        engine.add_cline_message(roo_types::message::ClineMessage {
            ts: 1000.0,
            r#type: roo_types::message::MessageType::Say,
            ask: None,
            say: None,
            text: Some("test message".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        });

        engine.save_cline_messages().await.unwrap();

        // Verify file was created
        let base = std::path::Path::new(&storage_path);
        let path = roo_task_persistence::messages_path(base, "cline-test");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_save_task_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let storage_path = dir.path().to_string_lossy().to_string();

        let mut engine = TaskEngine::new(
            TaskConfig::new("meta-test", "/tmp/work")
                .with_mode("code")
                .with_max_iterations(100)
                .with_storage_path(&storage_path)
                .with_start_task(false),
        )
        .unwrap();

        engine.start().unwrap();
        engine.save_task().await.unwrap();

        // Verify metadata file was created
        let base = std::path::Path::new(&storage_path);
        let path = roo_task_persistence::metadata_path(base, "meta-test");
        assert!(path.exists());

        // Verify it's valid JSON
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["taskId"], "meta-test");
    }

    #[test]
    fn test_task_state_to_persistence_status() {
        assert_eq!(
            task_state_to_persistence_status(TaskState::Idle),
            roo_task_persistence::PersistenceTaskStatus::Active
        );
        assert_eq!(
            task_state_to_persistence_status(TaskState::Running),
            roo_task_persistence::PersistenceTaskStatus::Active
        );
        assert_eq!(
            task_state_to_persistence_status(TaskState::Paused),
            roo_task_persistence::PersistenceTaskStatus::Active
        );
        assert_eq!(
            task_state_to_persistence_status(TaskState::Completed),
            roo_task_persistence::PersistenceTaskStatus::Completed
        );
        assert_eq!(
            task_state_to_persistence_status(TaskState::Aborted),
            roo_task_persistence::PersistenceTaskStatus::Aborted
        );
        assert_eq!(
            task_state_to_persistence_status(TaskState::Delegated),
            roo_task_persistence::PersistenceTaskStatus::Delegated
        );
    }
}
