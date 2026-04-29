//! Task event system.
//!
//! Provides [`TaskEvent`] enum and [`TaskEventEmitter`] for emitting
//! task lifecycle events to registered listeners.
//!
//! Source: `packages/types/src/events.ts` — `RooCodeEventName` enum and
//! `src/core/task/Task.ts` — all `emit()` calls.

use std::sync::{Arc, Mutex};

use roo_types::message::{ClineMessage, TokenUsage};
use roo_types::tool::ToolUsage;

use crate::types::TaskState;

// ---------------------------------------------------------------------------
// TaskEvent
// ---------------------------------------------------------------------------

/// Events emitted during task execution.
///
/// Source: `packages/types/src/events.ts` — `RooCodeEventName` enum
/// and `src/core/task/Task.ts` — all `emit()` calls
#[derive(Debug, Clone)]
pub enum TaskEvent {
    // --- Task Provider Lifecycle ---
    /// Task was created.
    /// Source: TS `ClineProvider.ts` — `emit(RooCodeEventName.TaskCreated, task)`
    TaskCreated { task_id: String },

    // --- Task Lifecycle ---
    /// Task started.
    /// Source: TS `initiateTaskLoop()` → `emit(RooCodeEventName.TaskStarted)`
    TaskStarted { task_id: String },
    /// Task completed (attempt_completion or no more tools).
    /// Source: TS `AttemptCompletionTool.ts` line 205 —
    /// `task.emit(RooCodeEventName.TaskCompleted, task.taskId, task.getTokenUsage(), task.toolUsage)`
    TaskCompleted {
        task_id: String,
        token_usage: TokenUsage,
        tool_usage: ToolUsage,
        is_subtask: bool,
    },
    /// Task aborted by user or error.
    /// Source: TS `abortTask()` flow
    TaskAborted { task_id: String, reason: Option<String> },
    /// Task focused (brought to foreground).
    /// Source: TS `ClineProvider.ts` line 274 — `emit(RooCodeEventName.TaskFocused)`
    TaskFocused { task_id: String },
    /// Task unfocused (moved to background).
    /// Source: TS `ClineProvider.ts` line 275 — `emit(RooCodeEventName.TaskUnfocused)`
    TaskUnfocused { task_id: String },
    /// Task transitioned to active state.
    /// Source: TS `ask()` line 1494 — `emit(RooCodeEventName.TaskActive, this.taskId)`
    TaskActive { task_id: String },
    /// Task is waiting for user interaction.
    /// Source: TS `ask()` → `emit(RooCodeEventName.TaskInteractive)`
    TaskInteractive { task_id: String },
    /// Task is resumable (can be resumed from history).
    /// Source: TS `ask()` → `emit(RooCodeEventName.TaskResumable)`
    TaskResumable { task_id: String },
    /// Task is idle and waiting for user input.
    /// Source: TS `ask()` → `emit(RooCodeEventName.TaskIdle)`
    TaskIdle { task_id: String },

    // --- Subtask Lifecycle ---
    /// Task paused.
    /// Source: TS `pause()` flow
    TaskPaused { task_id: String },
    /// Task resumed from pause (unpaused).
    /// Source: TS `ClineProvider.ts` line 281 — `emit(RooCodeEventName.TaskUnpaused)`
    TaskUnpaused { task_id: String },
    /// Task spawned a subtask.
    /// Source: TS `ClineProvider.ts` line 282 — `emit(RooCodeEventName.TaskSpawned)`
    TaskSpawned {
        parent_task_id: String,
        child_task_id: String,
    },
    /// Task delegated to subtask.
    /// Source: TS `ClineProvider.ts` line 3355 — `emit(RooCodeEventName.TaskDelegated, parentTaskId, child.taskId)`
    TaskDelegated {
        parent_task_id: String,
        child_task_id: String,
    },
    /// Subtask delegation completed.
    /// Source: TS `ClineProvider.ts` line 3528 —
    /// `emit(RooCodeEventName.TaskDelegationCompleted, parentTaskId, childTaskId, summary)`
    TaskDelegationCompleted {
        parent_task_id: String,
        child_task_id: String,
        summary: String,
    },
    /// Subtask delegation resumed.
    /// Source: TS `ClineProvider.ts` line 3556 —
    /// `emit(RooCodeEventName.TaskDelegationResumed, parentTaskId, childTaskId)`
    TaskDelegationResumed {
        parent_task_id: String,
        child_task_id: String,
    },

    // --- Task Execution ---
    /// A message was created or updated.
    /// Source: TS `Task.ts` line 1162 — `emit(RooCodeEventName.Message, { action: "created", message })`
    /// and line 1195 — `emit(RooCodeEventName.Message, { action: "updated", message })`
    Message {
        task_id: String,
        action: String, // "created" or "updated"
        message: ClineMessage,
    },
    /// A new message was created (legacy, kept for backward compat).
    /// Source: TS `addToClineMessages()` → emit
    MessageCreated { message: ClineMessage },
    /// An existing message was updated (legacy, kept for backward compat).
    /// Source: TS `updateClineMessage()` → emit
    MessageUpdated { message: ClineMessage },
    /// Mode switched.
    /// Source: TS `switch_mode` tool → `emit(RooCodeEventName.TaskModeSwitched, taskId, mode)`
    TaskModeSwitched { task_id: String, mode: String },
    /// Task ask was responded to by the user.
    /// Source: TS `ask()` line 1497 — `emit(RooCodeEventName.TaskAskResponded)`
    TaskAskResponded { task_id: String },
    /// User submitted a message.
    /// Source: TS `submitUserMessage()` line 1617 — `emit(RooCodeEventName.TaskUserMessage, this.taskId)`
    UserMessage { task_id: String },
    /// User interaction is required (sent to webview).
    /// Source: TS `ask()` line 1402 — `provider?.postMessageToWebview({ type: "interactionRequired" })`
    InteractionRequired { task_id: String },
    /// Queued messages updated.
    /// Source: TS `Task.ts` line 525 — `emit(RooCodeEventName.QueuedMessagesUpdated, taskId, messages)`
    QueuedMessagesUpdated {
        task_id: String,
        messages: Vec<roo_types::message::QueuedMessage>,
    },

    // --- Task Analytics ---
    /// Token usage was updated.
    /// Source: TS `Task.ts` line 555 —
    /// `emit(RooCodeEventName.TaskTokenUsageUpdated, this.taskId, tokenUsage, toolUsage)`
    TaskTokenUsageUpdated {
        task_id: String,
        token_usage: TokenUsage,
        tool_usage: ToolUsage,
    },
    /// Token usage was updated (legacy, simpler version).
    TokenUsageUpdated { usage: TokenUsage },
    /// A tool execution failed.
    /// Source: TS `Task.ts` line 4633 — `emit(RooCodeEventName.TaskToolFailed, this.taskId, toolName, error)`
    TaskToolFailed {
        task_id: String,
        tool_name: String,
        error: String,
    },

    // --- Configuration Changes ---
    /// Global mode changed.
    /// Source: TS `ClineProvider.ts` line 1427 — `emit(RooCodeEventName.ModeChanged, newMode)`
    ModeChanged { mode: String },
    /// Provider profile changed.
    /// Source: TS `ClineProvider.ts` line 1673 —
    /// `emit(RooCodeEventName.ProviderProfileChanged, { name, provider })`
    ProviderProfileChanged {
        name: String,
        provider: Option<String>,
    },

    // --- State change ---
    /// The task state changed.
    /// Source: TS state transitions → emit
    StateChanged { from: TaskState, to: TaskState },
    /// A tool was executed.
    /// Source: TS `executeTool()` → emit
    ToolExecuted { tool_name: String, success: bool },

    // --- API events ---
    /// API request started.
    /// Source: TS `recursivelyMakeClineRequests()` → `say("api_req_started")`
    ApiRequestStarted { task_id: String },
    /// API request finished.
    /// Source: TS `recursivelyMakeClineRequests()` → update api_req_started
    ApiRequestFinished {
        task_id: String,
        cost: Option<f64>,
        tokens_in: Option<u64>,
        tokens_out: Option<u64>,
    },

    // --- Context events ---
    /// Context condensation requested.
    /// Source: TS `condenseContext()` → emit
    ContextCondensationRequested { task_id: String },
    /// Context condensation completed.
    /// Source: TS `manageContext()` → after condense
    ContextCondensationCompleted {
        task_id: String,
        messages_removed: usize,
    },
    /// Context truncation performed.
    /// Source: TS `maybeTruncateMessages()` → emit
    ContextTruncationPerformed {
        task_id: String,
        messages_removed: usize,
    },

    // --- Error events ---
    /// An error occurred during task execution.
    /// Source: TS `say("error", ...)` — e.g., "MODEL_NO_TOOLS_USED", "MODEL_NO_ASSISTANT_MESSAGES"
    Error {
        task_id: String,
        error: String,
    },
    /// A tool execution error occurred.
    /// Source: TS `recordToolError()` lines 4625-4635
    ToolError {
        task_id: String,
        tool_name: String,
        error: String,
    },

    // --- Checkpoint events ---
    /// Checkpoint saved.
    /// Source: TS `checkpointSave()` → emit
    CheckpointSaved {
        task_id: String,
        commit: Option<String>,
    },
    /// Checkpoint restored.
    /// Source: TS `checkpointRestore()` → emit
    CheckpointRestored { task_id: String },

    // --- Streaming events (real-time) ---
    /// A text delta was received from the streaming API response.
    /// Source: TS `presentAssistantMessage()` — text streaming state machine
    StreamingTextDelta { task_id: String, text: String },
    /// A reasoning/thinking delta was received from the streaming API response.
    /// Source: TS `presentAssistantMessage()` — reasoning streaming
    StreamingReasoningDelta { task_id: String, text: String },
    /// A tool use started (tool call header received from stream).
    /// Source: TS `presentAssistantMessage()` — tool_use detection
    StreamingToolUseStarted {
        task_id: String,
        tool_name: String,
        tool_id: String,
    },
    /// A tool use received an argument delta during streaming.
    /// Source: TS `presentAssistantMessage()` — tool_use streaming
    StreamingToolUseDelta {
        task_id: String,
        tool_id: String,
        delta: String,
    },
    /// A tool use completed (tool result available).
    /// Source: TS `presentAssistantMessage()` — after tool execution
    StreamingToolUseCompleted {
        task_id: String,
        tool_name: String,
        tool_id: String,
        success: bool,
    },
    /// Streaming response completed.
    /// Source: TS `recursivelyMakeClineRequests()` — after stream ends
    StreamingCompleted { task_id: String },

    // --- Tool approval events ---
    /// A tool requires user approval before execution.
    ///
    /// Source: TS `presentAssistantMessage` → `askFollowupQuestion` → user approval flow.
    /// In the TS implementation, when a tool needs approval, the UI presents a dialog
    /// and the user can approve or deny. The Rust implementation uses a oneshot channel
    /// pattern: the agent loop creates a channel, emits this event, and awaits the
    /// receiver. External code calls `AgentLoop::set_approval_response()` to send
    /// the user's decision through the channel.
    ToolApprovalRequired {
        task_id: String,
        tool_name: String,
        tool_id: String,
        reason: String,
    },

    // --- User interaction events ---
    /// API request failed and user is asked whether to retry.
    ///
    /// Source: TS `attemptApiRequest()` → `ask("api_req_failed")` — when API call
    /// fails, the user can choose to retry or cancel. The Rust implementation uses
    /// a oneshot channel pattern similar to tool approval.
    ApiRequestFailed {
        task_id: String,
        error: String,
    },

    /// Consecutive mistake limit reached, asking user for guidance.
    ///
    /// Source: TS `recursivelyMakeClineRequests()` →
    /// `ask("mistake_limit_reached")` — when the model makes too many
    /// consecutive mistakes, the user is asked for feedback. The Rust
    /// implementation uses a oneshot channel pattern similar to tool approval.
    MistakeLimitReached {
        task_id: String,
        count: usize,
        limit: usize,
    },

    // --- Rate limit events ---
    /// Rate limit countdown tick.
    /// Source: TS `maybeWaitForProviderRateLimit()` → `say("api_req_rate_limit_wait")`
    ApiRateLimitWait {
        task_id: String,
        seconds: u64,
    },
}

// ---------------------------------------------------------------------------
// EventListener
// ---------------------------------------------------------------------------

/// A function that handles task events.
pub type EventListenerFn = dyn Fn(&TaskEvent) + Send + Sync;

// ---------------------------------------------------------------------------
// TaskEventEmitter
// ---------------------------------------------------------------------------

/// An event emitter for task lifecycle events.
///
/// Listeners are stored in a thread-safe manner and called in order of registration.
pub struct TaskEventEmitter {
    listeners: Arc<Mutex<Vec<Arc<EventListenerFn>>>>,
}

impl TaskEventEmitter {
    /// Create a new event emitter with no listeners.
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a new event listener.
    pub fn on(&self, listener: impl Fn(&TaskEvent) + Send + Sync + 'static) {
        self.listeners.lock().unwrap().push(Arc::new(listener));
    }

    // --- Task Provider Lifecycle emit methods ---

    /// Emit a task created event.
    ///
    /// Source: TS `ClineProvider.ts` — `emit(RooCodeEventName.TaskCreated, task)`
    pub fn emit_task_created(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskCreated {
            task_id: task_id.to_string(),
        });
    }

    // --- Task Lifecycle emit methods ---

    /// Emit a task started event.
    pub fn emit_task_started(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskStarted {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a task completed event with full payload.
    ///
    /// Source: TS `AttemptCompletionTool.ts` line 205 —
    /// `task.emit(RooCodeEventName.TaskCompleted, task.taskId, task.getTokenUsage(), task.toolUsage)`
    pub fn emit_task_completed(
        &self,
        task_id: &str,
        token_usage: TokenUsage,
        tool_usage: ToolUsage,
        is_subtask: bool,
    ) {
        self.emit(&TaskEvent::TaskCompleted {
            task_id: task_id.to_string(),
            token_usage,
            tool_usage,
            is_subtask,
        });
    }

    /// Emit a task aborted event.
    pub fn emit_task_aborted(&self, task_id: &str, reason: Option<String>) {
        self.emit(&TaskEvent::TaskAborted {
            task_id: task_id.to_string(),
            reason,
        });
    }

    /// Emit a task focused event.
    ///
    /// Source: TS `ClineProvider.ts` line 274 — `emit(RooCodeEventName.TaskFocused)`
    pub fn emit_task_focused(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskFocused {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a task unfocused event.
    ///
    /// Source: TS `ClineProvider.ts` line 275 — `emit(RooCodeEventName.TaskUnfocused)`
    pub fn emit_task_unfocused(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskUnfocused {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a task active event.
    ///
    /// Source: TS `ask()` line 1494 — `emit(RooCodeEventName.TaskActive, this.taskId)`
    pub fn emit_task_active(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskActive {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a task interactive event.
    pub fn emit_task_interactive(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskInteractive {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a task resumable event.
    pub fn emit_task_resumable(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskResumable {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a task idle event.
    pub fn emit_task_idle(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskIdle {
            task_id: task_id.to_string(),
        });
    }

    // --- Subtask Lifecycle emit methods ---

    /// Emit a task paused event.
    pub fn emit_task_paused(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskPaused {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a task unpaused event.
    ///
    /// Source: TS `ClineProvider.ts` line 281 — `emit(RooCodeEventName.TaskUnpaused)`
    pub fn emit_task_unpaused(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskUnpaused {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a task spawned event.
    ///
    /// Source: TS `ClineProvider.ts` line 282 — `emit(RooCodeEventName.TaskSpawned)`
    pub fn emit_task_spawned(&self, parent_task_id: &str, child_task_id: &str) {
        self.emit(&TaskEvent::TaskSpawned {
            parent_task_id: parent_task_id.to_string(),
            child_task_id: child_task_id.to_string(),
        });
    }

    /// Emit a task delegated event.
    pub fn emit_task_delegated(&self, parent_task_id: &str, child_task_id: &str) {
        self.emit(&TaskEvent::TaskDelegated {
            parent_task_id: parent_task_id.to_string(),
            child_task_id: child_task_id.to_string(),
        });
    }

    /// Emit a task delegation completed event.
    ///
    /// Source: TS `ClineProvider.ts` line 3528 —
    /// `emit(RooCodeEventName.TaskDelegationCompleted, parentTaskId, childTaskId, summary)`
    pub fn emit_task_delegation_completed(
        &self,
        parent_task_id: &str,
        child_task_id: &str,
        summary: &str,
    ) {
        self.emit(&TaskEvent::TaskDelegationCompleted {
            parent_task_id: parent_task_id.to_string(),
            child_task_id: child_task_id.to_string(),
            summary: summary.to_string(),
        });
    }

    /// Emit a task delegation resumed event.
    ///
    /// Source: TS `ClineProvider.ts` line 3556 —
    /// `emit(RooCodeEventName.TaskDelegationResumed, parentTaskId, childTaskId)`
    pub fn emit_task_delegation_resumed(&self, parent_task_id: &str, child_task_id: &str) {
        self.emit(&TaskEvent::TaskDelegationResumed {
            parent_task_id: parent_task_id.to_string(),
            child_task_id: child_task_id.to_string(),
        });
    }

    // --- Task Execution emit methods ---

    /// Emit a Message event (created or updated).
    ///
    /// Source: TS `Task.ts` line 1162 — `emit(RooCodeEventName.Message, { action: "created", message })`
    pub fn emit_message(&self, task_id: &str, action: &str, message: ClineMessage) {
        self.emit(&TaskEvent::Message {
            task_id: task_id.to_string(),
            action: action.to_string(),
            message,
        });
    }

    /// Emit a message created event (legacy).
    pub fn emit_message_created(&self, message: ClineMessage) {
        self.emit(&TaskEvent::MessageCreated { message });
    }

    /// Emit a message updated event (legacy).
    pub fn emit_message_updated(&self, message: ClineMessage) {
        self.emit(&TaskEvent::MessageUpdated { message });
    }

    /// Emit a mode switched event.
    pub fn emit_mode_switched(&self, task_id: &str, mode: &str) {
        self.emit(&TaskEvent::TaskModeSwitched {
            task_id: task_id.to_string(),
            mode: mode.to_string(),
        });
    }

    /// Emit a task ask responded event.
    ///
    /// Source: TS `ask()` line 1497 — `emit(RooCodeEventName.TaskAskResponded)`
    pub fn emit_task_ask_responded(&self, task_id: &str) {
        self.emit(&TaskEvent::TaskAskResponded {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a user message event.
    ///
    /// Source: TS `submitUserMessage()` line 1617 — `emit(RooCodeEventName.TaskUserMessage, this.taskId)`
    pub fn emit_user_message(&self, task_id: &str) {
        self.emit(&TaskEvent::UserMessage {
            task_id: task_id.to_string(),
        });
    }

    /// Emit an interaction required event.
    ///
    /// Source: TS `ask()` line 1402 — `provider?.postMessageToWebview({ type: "interactionRequired" })`
    pub fn emit_interaction_required(&self, task_id: &str) {
        self.emit(&TaskEvent::InteractionRequired {
            task_id: task_id.to_string(),
        });
    }

    /// Emit a queued messages updated event.
    ///
    /// Source: TS `Task.ts` line 525 — `emit(RooCodeEventName.QueuedMessagesUpdated, taskId, messages)`
    pub fn emit_queued_messages_updated(
        &self,
        task_id: &str,
        messages: Vec<roo_types::message::QueuedMessage>,
    ) {
        self.emit(&TaskEvent::QueuedMessagesUpdated {
            task_id: task_id.to_string(),
            messages,
        });
    }

    // --- Task Analytics emit methods ---

    /// Emit a task token usage updated event with full payload.
    ///
    /// Source: TS `Task.ts` line 555 —
    /// `emit(RooCodeEventName.TaskTokenUsageUpdated, this.taskId, tokenUsage, toolUsage)`
    pub fn emit_task_token_usage_updated(
        &self,
        task_id: &str,
        token_usage: TokenUsage,
        tool_usage: ToolUsage,
    ) {
        self.emit(&TaskEvent::TaskTokenUsageUpdated {
            task_id: task_id.to_string(),
            token_usage,
            tool_usage,
        });
    }

    /// Emit a token usage updated event (legacy).
    pub fn emit_token_usage_updated(&self, usage: TokenUsage) {
        self.emit(&TaskEvent::TokenUsageUpdated { usage });
    }

    /// Emit a task tool failed event.
    ///
    /// Source: TS `Task.ts` line 4633 — `emit(RooCodeEventName.TaskToolFailed, this.taskId, toolName, error)`
    pub fn emit_task_tool_failed(&self, task_id: &str, tool_name: &str, error: &str) {
        self.emit(&TaskEvent::TaskToolFailed {
            task_id: task_id.to_string(),
            tool_name: tool_name.to_string(),
            error: error.to_string(),
        });
    }

    // --- Configuration emit methods ---

    /// Emit a mode changed event.
    ///
    /// Source: TS `ClineProvider.ts` line 1427 — `emit(RooCodeEventName.ModeChanged, newMode)`
    pub fn emit_mode_changed(&self, mode: &str) {
        self.emit(&TaskEvent::ModeChanged {
            mode: mode.to_string(),
        });
    }

    /// Emit a provider profile changed event.
    ///
    /// Source: TS `ClineProvider.ts` line 1673 —
    /// `emit(RooCodeEventName.ProviderProfileChanged, { name, provider })`
    pub fn emit_provider_profile_changed(&self, name: &str, provider: Option<&str>) {
        self.emit(&TaskEvent::ProviderProfileChanged {
            name: name.to_string(),
            provider: provider.map(String::from),
        });
    }

    // --- State / Tool emit methods ---

    /// Emit a state changed event.
    pub fn emit_state_changed(&self, from: TaskState, to: TaskState) {
        self.emit(&TaskEvent::StateChanged { from, to });
    }

    /// Emit a tool executed event.
    pub fn emit_tool_executed(&self, tool_name: &str, success: bool) {
        self.emit(&TaskEvent::ToolExecuted {
            tool_name: tool_name.to_string(),
            success,
        });
    }

    // --- API emit methods ---

    /// Emit an API request started event.
    pub fn emit_api_request_started(&self, task_id: &str) {
        self.emit(&TaskEvent::ApiRequestStarted {
            task_id: task_id.to_string(),
        });
    }

    /// Emit an API request finished event.
    pub fn emit_api_request_finished(
        &self,
        task_id: &str,
        cost: Option<f64>,
        tokens_in: Option<u64>,
        tokens_out: Option<u64>,
    ) {
        self.emit(&TaskEvent::ApiRequestFinished {
            task_id: task_id.to_string(),
            cost,
            tokens_in,
            tokens_out,
        });
    }

    // --- Context emit methods ---

    /// Emit a context condensation completed event.
    pub fn emit_context_condensation_completed(&self, task_id: &str, messages_removed: usize) {
        self.emit(&TaskEvent::ContextCondensationCompleted {
            task_id: task_id.to_string(),
            messages_removed,
        });
    }

    /// Emit a context truncation performed event.
    pub fn emit_context_truncation_performed(&self, task_id: &str, messages_removed: usize) {
        self.emit(&TaskEvent::ContextTruncationPerformed {
            task_id: task_id.to_string(),
            messages_removed,
        });
    }

    // --- Streaming emit methods ---

    /// Emit a streaming text delta event.
    pub fn emit_streaming_text_delta(&self, task_id: &str, text: &str) {
        self.emit(&TaskEvent::StreamingTextDelta {
            task_id: task_id.to_string(),
            text: text.to_string(),
        });
    }

    /// Emit a streaming tool use started event.
    pub fn emit_streaming_tool_use_started(&self, task_id: &str, tool_name: &str, tool_id: &str) {
        self.emit(&TaskEvent::StreamingToolUseStarted {
            task_id: task_id.to_string(),
            tool_name: tool_name.to_string(),
            tool_id: tool_id.to_string(),
        });
    }

    /// Emit a streaming tool use completed event.
    pub fn emit_streaming_tool_use_completed(
        &self,
        task_id: &str,
        tool_name: &str,
        tool_id: &str,
        success: bool,
    ) {
        self.emit(&TaskEvent::StreamingToolUseCompleted {
            task_id: task_id.to_string(),
            tool_name: tool_name.to_string(),
            tool_id: tool_id.to_string(),
            success,
        });
    }

    /// Emit a tool approval required event.
    ///
    /// Source: TS `presentAssistantMessage` → `askFollowupQuestion` → user approval flow.
    /// Emitted when a tool call requires user approval before execution.
    pub fn emit_tool_approval_required(
        &self,
        task_id: &str,
        tool_name: &str,
        tool_id: &str,
        reason: &str,
    ) {
        self.emit(&TaskEvent::ToolApprovalRequired {
            task_id: task_id.to_string(),
            tool_name: tool_name.to_string(),
            tool_id: tool_id.to_string(),
            reason: reason.to_string(),
        });
    }

    /// Emit an API request failed event.
    ///
    /// Source: TS `attemptApiRequest()` → `ask("api_req_failed")`.
    /// Emitted when an API call fails and user interaction is needed
    /// to decide whether to retry or cancel.
    pub fn emit_api_request_failed(&self, task_id: &str, error: &str) {
        self.emit(&TaskEvent::ApiRequestFailed {
            task_id: task_id.to_string(),
            error: error.to_string(),
        });
    }

    /// Emit a mistake limit reached event.
    ///
    /// Source: TS `recursivelyMakeClineRequests()` →
    /// `ask("mistake_limit_reached")`.
    /// Emitted when the consecutive mistake limit is reached and
    /// user interaction is needed to provide guidance.
    pub fn emit_mistake_limit_reached(&self, task_id: &str, count: usize, limit: usize) {
        self.emit(&TaskEvent::MistakeLimitReached {
            task_id: task_id.to_string(),
            count,
            limit,
        });
    }

    /// Emit a streaming reasoning delta event.
    pub fn emit_streaming_reasoning_delta(&self, task_id: &str, text: &str) {
        self.emit(&TaskEvent::StreamingReasoningDelta {
            task_id: task_id.to_string(),
            text: text.to_string(),
        });
    }

    /// Emit a streaming tool use delta event.
    pub fn emit_streaming_tool_use_delta(&self, task_id: &str, tool_id: &str, delta: &str) {
        self.emit(&TaskEvent::StreamingToolUseDelta {
            task_id: task_id.to_string(),
            tool_id: tool_id.to_string(),
            delta: delta.to_string(),
        });
    }

    /// Emit a streaming completed event.
    pub fn emit_streaming_completed(&self, task_id: &str) {
        self.emit(&TaskEvent::StreamingCompleted {
            task_id: task_id.to_string(),
        });
    }

    // --- Checkpoint emit methods ---

    /// Emit a checkpoint saved event.
    pub fn emit_checkpoint_saved(&self, task_id: &str, commit: Option<String>) {
        self.emit(&TaskEvent::CheckpointSaved {
            task_id: task_id.to_string(),
            commit,
        });
    }

    /// Emit a checkpoint restored event.
    pub fn emit_checkpoint_restored(&self, task_id: &str) {
        self.emit(&TaskEvent::CheckpointRestored {
            task_id: task_id.to_string(),
        });
    }

    /// Emit an event to all registered listeners.
    pub fn emit(&self, event: &TaskEvent) {
        let listeners = self.listeners.lock().unwrap();
        for listener in listeners.iter() {
            listener(event);
        }
    }

    /// Get the number of registered listeners.
    pub fn listener_count(&self) -> usize {
        self.listeners.lock().unwrap().len()
    }
}

impl Default for TaskEventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TaskEventEmitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskEventEmitter")
            .field("listener_count", &self.listener_count())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::message::MessageType;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_test_message(text: &str) -> ClineMessage {
        ClineMessage {
            ts: 1700000000.0,
            r#type: MessageType::Say,
            ask: None,
            say: Some(roo_types::message::ClineSay::Text),
            text: Some(text.to_string()),
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
        }
    }

    fn make_test_token_usage() -> TokenUsage {
        TokenUsage {
            total_tokens_in: 1000,
            total_tokens_out: 500,
            total_cache_writes: Some(100),
            total_cache_reads: Some(50),
            total_cost: 1.5,
            context_tokens: 2000,
        }
    }

    fn make_test_tool_usage() -> ToolUsage {
        let mut usage = ToolUsage::new();
        usage.insert(
            roo_types::tool::ToolName::ReadFile,
            roo_types::tool::ToolUsageEntry {
                attempts: 3,
                failures: 1,
            },
        );
        usage
    }

    #[test]
    fn test_event_emitter_no_listeners() {
        let emitter = TaskEventEmitter::new();
        emitter.emit_state_changed(TaskState::Idle, TaskState::Running);
        // Should not panic
    }

    #[test]
    fn test_event_emitter_single_listener() {
        let count = Arc::new(AtomicUsize::new(0));
        let emitter = TaskEventEmitter::new();

        let count_clone = count.clone();
        emitter.on(move |_event| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        emitter.emit_state_changed(TaskState::Idle, TaskState::Running);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_event_emitter_multiple_listeners() {
        let count = Arc::new(AtomicUsize::new(0));
        let emitter = TaskEventEmitter::new();

        for _ in 0..3 {
            let count_clone = count.clone();
            emitter.on(move |_event| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        emitter.emit_state_changed(TaskState::Idle, TaskState::Running);
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_event_emitter_message_created() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::MessageCreated { message } = event {
                *received_clone.lock().unwrap() = Some(message.text.clone());
            }
        });

        let msg = make_test_message("Hello");
        emitter.emit_message_created(msg);

        let text = received.lock().unwrap().clone();
        assert_eq!(text, Some(Some("Hello".to_string())));
    }

    #[test]
    fn test_event_emitter_tool_executed() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::ToolExecuted { tool_name, success } = event {
                *received_clone.lock().unwrap() = Some((tool_name.clone(), *success));
            }
        });

        emitter.emit_tool_executed("read_file", true);

        let data = received.lock().unwrap().clone();
        assert_eq!(data, Some(("read_file".to_string(), true)));
    }

    #[test]
    fn test_event_emitter_token_usage_updated() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TokenUsageUpdated { usage } = event {
                *received_clone.lock().unwrap() = Some(usage.total_tokens_in);
            }
        });

        let usage = make_test_token_usage();
        emitter.emit_token_usage_updated(usage);

        let tokens = received.lock().unwrap().clone();
        assert_eq!(tokens, Some(1000u64));
    }

    #[test]
    fn test_event_emitter_task_completed_full_payload() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TaskCompleted {
                task_id,
                token_usage,
                tool_usage,
                is_subtask,
            } = event
            {
                *received_clone.lock().unwrap() = Some((
                    task_id.clone(),
                    token_usage.total_tokens_in,
                    tool_usage.len(),
                    *is_subtask,
                ));
            }
        });

        let token_usage = make_test_token_usage();
        let tool_usage = make_test_tool_usage();
        emitter.emit_task_completed("task_1", token_usage, tool_usage, false);

        let data = received.lock().unwrap().clone();
        assert_eq!(
            data,
            Some(("task_1".to_string(), 1000u64, 1, false))
        );
    }

    #[test]
    fn test_event_emitter_task_token_usage_updated() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TaskTokenUsageUpdated {
                task_id,
                token_usage,
                tool_usage,
            } = event
            {
                *received_clone.lock().unwrap() =
                    Some((task_id.clone(), token_usage.total_tokens_in, tool_usage.len()));
            }
        });

        let token_usage = make_test_token_usage();
        let tool_usage = make_test_tool_usage();
        emitter.emit_task_token_usage_updated("task_1", token_usage, tool_usage);

        let data = received.lock().unwrap().clone();
        assert_eq!(data, Some(("task_1".to_string(), 1000u64, 1)));
    }

    #[test]
    fn test_event_emitter_task_tool_failed() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TaskToolFailed {
                task_id,
                tool_name,
                error,
            } = event
            {
                *received_clone.lock().unwrap() =
                    Some((task_id.clone(), tool_name.clone(), error.clone()));
            }
        });

        emitter.emit_task_tool_failed("task_1", "read_file", "File not found");

        let data = received.lock().unwrap().clone();
        assert_eq!(
            data,
            Some((
                "task_1".to_string(),
                "read_file".to_string(),
                "File not found".to_string()
            ))
        );
    }

    #[test]
    fn test_event_emitter_task_delegation_completed() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TaskDelegationCompleted {
                parent_task_id,
                child_task_id,
                summary,
            } = event
            {
                *received_clone.lock().unwrap() = Some((
                    parent_task_id.clone(),
                    child_task_id.clone(),
                    summary.clone(),
                ));
            }
        });

        emitter.emit_task_delegation_completed("parent_1", "child_1", "Task completed successfully");

        let data = received.lock().unwrap().clone();
        assert_eq!(
            data,
            Some((
                "parent_1".to_string(),
                "child_1".to_string(),
                "Task completed successfully".to_string()
            ))
        );
    }

    #[test]
    fn test_event_emitter_task_delegation_resumed() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TaskDelegationResumed {
                parent_task_id,
                child_task_id,
            } = event
            {
                *received_clone.lock().unwrap() =
                    Some((parent_task_id.clone(), child_task_id.clone()));
            }
        });

        emitter.emit_task_delegation_resumed("parent_1", "child_1");

        let data = received.lock().unwrap().clone();
        assert_eq!(data, Some(("parent_1".to_string(), "child_1".to_string())));
    }

    #[test]
    fn test_event_emitter_mode_changed() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::ModeChanged { mode } = event {
                *received_clone.lock().unwrap() = Some(mode.clone());
            }
        });

        emitter.emit_mode_changed("code");

        let data = received.lock().unwrap().clone();
        assert_eq!(data, Some("code".to_string()));
    }

    #[test]
    fn test_event_emitter_provider_profile_changed() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::ProviderProfileChanged { name, provider } = event {
                *received_clone.lock().unwrap() = Some((name.clone(), provider.clone()));
            }
        });

        emitter.emit_provider_profile_changed("my-profile", Some("anthropic"));

        let data = received.lock().unwrap().clone();
        assert_eq!(
            data,
            Some(("my-profile".to_string(), Some("anthropic".to_string())))
        );
    }

    #[test]
    fn test_event_emitter_message_event() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::Message {
                task_id,
                action,
                message,
            } = event
            {
                *received_clone.lock().unwrap() =
                    Some((task_id.clone(), action.clone(), message.text.clone()));
            }
        });

        let msg = make_test_message("Hello");
        emitter.emit_message("task_1", "created", msg);

        let data = received.lock().unwrap().clone();
        assert_eq!(
            data,
            Some((
                "task_1".to_string(),
                "created".to_string(),
                Some("Hello".to_string())
            ))
        );
    }

    #[test]
    fn test_event_emitter_task_unpaused() {
        let received = Arc::new(std::sync::Mutex::new(false));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TaskUnpaused { .. } = event {
                *received_clone.lock().unwrap() = true;
            }
        });

        emitter.emit_task_unpaused("task_1");

        assert!(*received.lock().unwrap());
    }

    #[test]
    fn test_event_emitter_task_spawned() {
        let received = Arc::new(std::sync::Mutex::new(None));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TaskSpawned {
                parent_task_id,
                child_task_id,
            } = event
            {
                *received_clone.lock().unwrap() =
                    Some((parent_task_id.clone(), child_task_id.clone()));
            }
        });

        emitter.emit_task_spawned("parent_1", "child_1");

        let data = received.lock().unwrap().clone();
        assert_eq!(data, Some(("parent_1".to_string(), "child_1".to_string())));
    }

    #[test]
    fn test_event_emitter_task_focused() {
        let received = Arc::new(std::sync::Mutex::new(false));
        let emitter = TaskEventEmitter::new();

        let received_clone = received.clone();
        emitter.on(move |event| {
            if let TaskEvent::TaskFocused { .. } = event {
                *received_clone.lock().unwrap() = true;
            }
        });

        emitter.emit_task_focused("task_1");
        assert!(*received.lock().unwrap());
    }

    #[test]
    fn test_event_emitter_listener_count() {
        let emitter = TaskEventEmitter::new();
        assert_eq!(emitter.listener_count(), 0);

        emitter.on(|_| {});
        assert_eq!(emitter.listener_count(), 1);

        emitter.on(|_| {});
        assert_eq!(emitter.listener_count(), 2);
    }

    #[test]
    fn test_task_event_clone() {
        let event = TaskEvent::StateChanged {
            from: TaskState::Idle,
            to: TaskState::Running,
        };
        let cloned = event.clone();
        if let TaskEvent::StateChanged { from, to } = cloned {
            assert_eq!(from, TaskState::Idle);
            assert_eq!(to, TaskState::Running);
        } else {
            panic!("Expected StateChanged event");
        }
    }

    #[test]
    fn test_task_event_debug() {
        let event = TaskEvent::ToolExecuted {
            tool_name: "read_file".to_string(),
            success: true,
        };
        let debug_str = format!("{event:?}");
        assert!(debug_str.contains("read_file"));
    }
}
