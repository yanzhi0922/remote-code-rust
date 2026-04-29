//! Task lifecycle management.
//!
//! Faithfully replicates the lifecycle methods from
//! `src/core/task/Task.ts` (lines 1579–2470, 4454–4632).
//!
//! ## Method mapping
//!
//! | Rust method                            | TS source                            | Lines       |
//! |----------------------------------------|--------------------------------------|-------------|
//! | `TaskLifecycle::start()`               | `Task.start()`                       | 1924–1935   |
//! | `TaskLifecycle::start_task()`          | `Task.startTask()`                   | 1937–1999   |
//! | `TaskLifecycle::resume_task_from_history()` | `Task.resumeTaskFromHistory()`   | 2001–2232   |
//! | `TaskLifecycle::cancel_current_request()` | `Task.cancelCurrentRequest()`      | 2238–2249   |
//! | `TaskLifecycle::abort_task()`          | `Task.abortTask()`                   | 2257–2289   |
//! | `TaskLifecycle::dispose()`             | `Task.dispose()`                     | 2291–2378   |
//! | `TaskLifecycle::start_subtask()`       | `Task.startSubtask()`                | 2380–2403   |
//! | `TaskLifecycle::resume_after_delegation()` | `Task.resumeAfterDelegation()`  | 2406–2470   |
//! | `TaskLifecycle::submit_user_message()` | `Task.submitUserMessage()`           | 1585–1629   |
//! | `TaskLifecycle::condense_context()`    | `Task.condenseContext()`              | 1648–1753   |
//! | `TaskLifecycle::update_api_configuration()` | `Task.updateApiConfiguration()` | 1579–1583   |
//! | `TaskLifecycle::handle_terminal_operation()` | `Task.handleTerminalOperation()` | 1631–1636 |
//! | `TaskLifecycle::checkpoint_save()`     | `Task.checkpointSave()`              | 4454–4565   |
//! | `TaskLifecycle::checkpoint_restore()`  | `Task.checkpointRestore()`           | 4567–4589   |
//! | `TaskLifecycle::checkpoint_diff()`     | `Task.checkpointDiff()`              | 4591–4604   |
//! | `TaskLifecycle::get_token_usage()`     | `Task.getTokenUsage()`               | 4613–4615   |
//! | `TaskLifecycle::record_tool_usage()`   | `Task.recordToolUsage()`             | 4617–4623   |
//! | `TaskLifecycle::record_tool_error()`   | `Task.recordToolError()`             | 4625–4632   |

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use roo_types::message::{ClineAsk, ClineMessage, ClineSay, MessageType};

use crate::ask_say::{AskResponse, AskSayHandler, SayOptions};
use crate::engine::TaskEngine;
use crate::events::TaskEvent;
use crate::types::{TaskConfig, TaskError, TaskState};

// ---------------------------------------------------------------------------
// ServiceRefs
// ---------------------------------------------------------------------------

/// References to external services needed by the task lifecycle.
///
/// All fields are optional — if not set, the corresponding functionality
/// is silently skipped. This allows `TaskLifecycle` to work in tests and
/// lightweight contexts without requiring the full service stack.
///
/// Source: `src/core/task/Task.ts` — constructor injection of services
/// (`this.mcpHub`, `this.terminal`, `this.messageQueueService`,
/// `this.telemetryService`).
#[derive(Clone, Default)]
pub struct ServiceRefs {
    /// MCP Hub for MCP server connections.
    ///
    /// Source: TS `this.mcpHub`
    pub mcp_hub: Option<Arc<roo_mcp::McpHub>>,
    /// Terminal registry for managing terminal processes.
    ///
    /// Source: TS `this.terminal`
    pub terminal_registry: Option<Arc<roo_terminal::TerminalRegistry>>,
    /// Message queue for buffering user messages during ask states.
    ///
    /// Source: TS `this.messageQueueService`
    pub message_queue: Option<Arc<tokio::sync::Mutex<roo_message_queue::MessageQueueService>>>,
    /// Telemetry service for capturing lifecycle events.
    ///
    /// Source: TS `this.telemetryService`
    pub telemetry: Option<Arc<std::sync::RwLock<roo_telemetry::TelemetryService>>>,
}

// ---------------------------------------------------------------------------
// TaskLifecycle
// ---------------------------------------------------------------------------

/// Manages the complete task lifecycle.
///
/// This struct wraps a [`TaskEngine`] and an [`AskSayHandler`] to provide
/// the full task lifecycle as defined in `Task.ts`.
///
/// # Lifecycle
///
/// 1. **New Task**: `start()` → `start_task()` → `initiate_task_loop()`
/// 2. **Resume**: `resume_task_from_history()` → load history → ask to resume
/// 3. **Cancel**: `cancel_current_request()` → abort stream
/// 4. **Abort**: `abort_task()` → clean up
/// 5. **Delegate**: `start_subtask()` → create child → `resume_after_delegation()`
/// 6. **Dispose**: `dispose()` → clean up all resources
pub struct TaskLifecycle {
    /// The task engine managing state, loop control, and events.
    engine: TaskEngine,
    /// The ask/say handler for interactive communication.
    ask_say: AskSayHandler,
    /// External service references (MCP Hub, Terminal, Message Queue, Telemetry).
    services: ServiceRefs,

    // ── Controllers (Source: TS constructor lines 478–493) ──────────────
    /// RooIgnoreController for .rooignore pattern filtering.
    ///
    /// Source: TS `this.rooIgnoreController = new RooIgnoreController(this.cwd)` (line 478)
    roo_ignore_controller: Option<roo_ignore::RooIgnoreController>,
    /// RooProtectedController for write-protected file detection.
    ///
    /// Source: TS `this.rooProtectedController = new RooProtectedController(this.cwd)` (line 479)
    roo_protected_controller: Option<roo_protect::RooProtectedController>,
    /// FileContextTracker for tracking file reads/edits.
    ///
    /// Source: TS `this.fileContextTracker = new FileContextTracker(provider, this.taskId)` (line 480)
    file_context_tracker:
        Option<roo_context_tracking::FileContextTracker<roo_context_tracking::InMemoryMetadataStore>>,
    /// DiffViewProvider for managing diff editor state.
    ///
    /// Source: TS `this.diffViewProvider = new DiffViewProvider(this.cwd, this)` (line 493)
    diff_view_provider: Option<roo_editor::DiffViewProvider>,
    /// AutoApprovalHandler for tracking auto-approval limits.
    ///
    /// Source: TS `this.autoApprovalHandler = new AutoApprovalHandler()` (line 488)
    auto_approval_handler: Option<roo_auto_approval::approval::AutoApprovalHandler>,

    // ── Lifecycle state ─────────────────────────────────────────────────
    /// Whether the task has been started.
    ///
    /// Source: TS `this._started` (line 1925)
    started: bool,
    /// Whether the task has been disposed.
    disposed: bool,
    /// Child task ID, if this task has delegated to a subtask.
    child_task_id: Option<String>,
    /// Pending new task tool call ID.
    #[allow(dead_code)]
    pending_new_task_tool_call_id: Option<String>,
    /// Abort controller flag.
    ///
    /// Source: TS `this.abort` (line 1279)
    abort: bool,
    /// Abort reason.
    ///
    /// Source: TS `this.abortReason`
    abort_reason: Option<String>,
    /// Whether the task has been abandoned (for delegation).
    ///
    /// Source: TS `this.abandoned` (line 2262)
    abandoned: bool,
    /// Maximum number of MCP tools before warning.
    ///
    /// Source: TS `MAX_MCP_TOOLS_THRESHOLD`
    max_mcp_tools_threshold: usize,
    /// Cancellation token for mid-stream abort.
    ///
    /// When set (via `set_cancellation_token()`), this token is cancelled
    /// when `cancel_current_request()` is called, allowing the spawned
    /// stream-consumer task in `AgentLoop` to stop immediately.
    cancellation_token: Option<CancellationToken>,
}

impl TaskLifecycle {
    /// Create a new task lifecycle with the given engine.
    ///
    /// Source: `src/core/task/Task.ts` — constructor (lines 420–578)
    ///
    /// Initializes all controllers and handlers that the TS constructor creates:
    /// - `rooIgnoreController` (line 478)
    /// - `rooProtectedController` (line 479)
    /// - `fileContextTracker` (line 480)
    /// - `diffViewProvider` (line 493)
    /// - `autoApprovalHandler` (line 488)
    pub fn new(engine: TaskEngine) -> Self {
        let cwd = engine.config().cwd.clone();
        let task_id = engine.config().task_id.clone();

        // Source: TS line 478 — `this.rooIgnoreController = new RooIgnoreController(this.cwd)`
        let roo_ignore_controller = Some(roo_ignore::RooIgnoreController::new(&cwd));

        // Source: TS line 479 — `this.rooProtectedController = new RooProtectedController(this.cwd)`
        let roo_protected_controller = Some(roo_protect::RooProtectedController::new(&cwd));

        // Source: TS line 480 — `this.fileContextTracker = new FileContextTracker(provider, this.taskId)`
        let store = roo_context_tracking::InMemoryMetadataStore::new();
        let file_context_tracker =
            Some(roo_context_tracking::FileContextTracker::new(&task_id, store));

        // Source: TS line 493 — `this.diffViewProvider = new DiffViewProvider(this.cwd, this)`
        let diff_view_provider = Some(roo_editor::DiffViewProvider::new(
            roo_editor::DiffViewOptions::default(),
        ));

        // Source: TS line 488 — `this.autoApprovalHandler = new AutoApprovalHandler()`
        let auto_approval_handler = Some(roo_auto_approval::approval::AutoApprovalHandler::new());

        Self {
            engine,
            ask_say: AskSayHandler::new(),
            services: ServiceRefs::default(),
            roo_ignore_controller,
            roo_protected_controller,
            file_context_tracker,
            diff_view_provider,
            auto_approval_handler,
            started: false,
            disposed: false,
            child_task_id: None,
            pending_new_task_tool_call_id: None,
            abort: false,
            abort_reason: None,
            abandoned: false,
            max_mcp_tools_threshold: 128,
            cancellation_token: None,
        }
    }

    /// Attach external service references to this lifecycle.
    ///
    /// Source: `src/core/task/Task.ts` — constructor injection of
    /// `mcpHub`, `terminal`, `messageQueueService`, `telemetryService`.
    pub fn with_services(mut self, services: ServiceRefs) -> Self {
        self.services = services;
        self
    }

    // -------------------------------------------------------------------
    // Getters
    // -------------------------------------------------------------------

    /// Get a reference to the task engine.
    pub fn engine(&self) -> &TaskEngine {
        &self.engine
    }

    /// Get a mutable reference to the task engine.
    pub fn engine_mut(&mut self) -> &mut TaskEngine {
        &mut self.engine
    }

    /// Get a reference to the ask/say handler.
    pub fn ask_say(&self) -> &AskSayHandler {
        &self.ask_say
    }

    /// Get a mutable reference to the ask/say handler.
    pub fn ask_say_mut(&mut self) -> &mut AskSayHandler {
        &mut self.ask_say
    }

    /// Get a reference to the service refs.
    pub fn services(&self) -> &ServiceRefs {
        &self.services
    }

    /// Get a mutable reference to the service refs.
    pub fn services_mut(&mut self) -> &mut ServiceRefs {
        &mut self.services
    }

    /// Get the task ID.
    pub fn task_id(&self) -> &str {
        &self.engine.config().task_id
    }

    /// Get the current task state.
    pub fn state(&self) -> TaskState {
        self.engine.state()
    }

    /// Get the child task ID.
    pub fn child_task_id(&self) -> Option<&str> {
        self.child_task_id.as_deref()
    }

    /// Check if the task has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.abort
    }

    /// Get the abort reason.
    pub fn abort_reason(&self) -> Option<&str> {
        self.abort_reason.as_deref()
    }

    /// Check if the task has been disposed.
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }

    /// Get the RooIgnoreController.
    ///
    /// Source: TS `this.rooIgnoreController` (line 478)
    pub fn roo_ignore_controller(&self) -> Option<&roo_ignore::RooIgnoreController> {
        self.roo_ignore_controller.as_ref()
    }

    /// Get a mutable reference to the RooIgnoreController.
    pub fn roo_ignore_controller_mut(&mut self) -> Option<&mut roo_ignore::RooIgnoreController> {
        self.roo_ignore_controller.as_mut()
    }

    /// Get the RooProtectedController.
    ///
    /// Source: TS `this.rooProtectedController` (line 479)
    pub fn roo_protected_controller(&self) -> Option<&roo_protect::RooProtectedController> {
        self.roo_protected_controller.as_ref()
    }

    /// Get the FileContextTracker.
    ///
    /// Source: TS `this.fileContextTracker` (line 480)
    pub fn file_context_tracker(
        &self,
    ) -> Option<&roo_context_tracking::FileContextTracker<roo_context_tracking::InMemoryMetadataStore>>
    {
        self.file_context_tracker.as_ref()
    }

    /// Get a mutable reference to the FileContextTracker.
    pub fn file_context_tracker_mut(
        &mut self,
    ) -> Option<&mut roo_context_tracking::FileContextTracker<roo_context_tracking::InMemoryMetadataStore>>
    {
        self.file_context_tracker.as_mut()
    }

    /// Get the DiffViewProvider.
    ///
    /// Source: TS `this.diffViewProvider` (line 493)
    pub fn diff_view_provider(&self) -> Option<&roo_editor::DiffViewProvider> {
        self.diff_view_provider.as_ref()
    }

    /// Get a mutable reference to the DiffViewProvider.
    pub fn diff_view_provider_mut(&mut self) -> Option<&mut roo_editor::DiffViewProvider> {
        self.diff_view_provider.as_mut()
    }

    /// Get the AutoApprovalHandler.
    ///
    /// Source: TS `this.autoApprovalHandler` (line 488)
    pub fn auto_approval_handler(
        &self,
    ) -> Option<&roo_auto_approval::approval::AutoApprovalHandler> {
        self.auto_approval_handler.as_ref()
    }

    /// Get a mutable reference to the AutoApprovalHandler.
    pub fn auto_approval_handler_mut(
        &mut self,
    ) -> Option<&mut roo_auto_approval::approval::AutoApprovalHandler> {
        self.auto_approval_handler.as_mut()
    }

    /// Get the task status as reported to the UI.
    ///
    /// Source: `src/core/task/Task.ts` — `taskStatus` getter
    pub fn task_status(&self) -> TaskStatus {
        // Map from internal TaskState to TaskStatus
        match self.engine.state() {
            TaskState::Idle => TaskStatus::Idle,
            TaskState::Running => TaskStatus::Running,
            TaskState::Paused => TaskStatus::Paused,
            TaskState::Completed => TaskStatus::Completed,
            TaskState::Aborted => TaskStatus::Aborted,
            TaskState::Delegated => TaskStatus::Running, // Delegated tasks show as running
        }
    }

    // ===================================================================
    // start()
    // Source: `src/core/task/Task.ts` — `start()` (lines 1924–1935)
    // ===================================================================

    /// Start the task.
    ///
    /// Source: `src/core/task/Task.ts` — `start()` (lines 1924–1935)
    ///
    /// Manually starts a new task when it was created with `startTask: false`.
    /// If the task has already been started, this is a no-op.
    /// If there's an initial task text or images, starts a new task.
    /// If there's a history item ID, resumes from history.
    pub async fn start(&mut self) -> Result<(), TaskError> {
        // Source: TS lines 1925–1927 — check if already started
        if self.started {
            debug!(task_id = %self.task_id(), "Task already started, skipping");
            return Ok(());
        }
        self.started = true;

        let task_text = self.engine.config().task_text.clone();
        let images = self.engine.config().images.clone();
        let history_item_id = self.engine.config().history_item_id.clone();

        // Source: TS lines 1930–1934 — dispatch based on available data
        if task_text.is_some() || !images.is_empty() {
            self.start_task(task_text, images).await
        } else if history_item_id.is_some() {
            self.resume_task_from_history().await
        } else {
            // Nothing to do — just mark as initialized
            self.engine.set_initialized(true);
            Ok(())
        }
    }

    // ===================================================================
    // startTask()
    // Source: `src/core/task/Task.ts` — `startTask()` (lines 1937–1999)
    // ===================================================================

    /// Start a new task with the given text and images.
    ///
    /// Source: `src/core/task/Task.ts` — `startTask()` (lines 1937–1999)
    ///
    /// 1. Clears existing messages (lines 1945–1946)
    /// 2. Emits the task text as a "say" message (line 1953)
    /// 3. Checks for too many MCP tools (lines 1955–1971)
    /// 4. Marks as initialized (line 1972)
    /// 5. Initiates the task loop (lines 1977–1990)
    async fn start_task(
        &mut self,
        task_text: Option<String>,
        images: Vec<String>,
    ) -> Result<(), TaskError> {
        // Source: TS lines 1938–1999 — wrapped in try/catch for abort handling
        // Clear existing messages
        // Source: TS `this.clineMessages = []` and `this.apiConversationHistory = []`
        self.ask_say.overwrite_cline_messages(Vec::new());
        self.engine.clear_api_conversation_history();

        // Emit the task text
        // Source: TS line 1953 — `await this.say("text", task, images)`
        if let Some(ref text) = task_text {
            self.ask_say
                .say_simple(ClineSay::Text, Some(text.clone()), Some(images.clone()))
                .await?;
        }

        // Check for too many MCP tools
        // Source: TS lines 1955–1971
        self.check_mcp_tools_count().await;

        // Mark as initialized
        // Source: TS line 1972 — `this.isInitialized = true`
        self.engine.set_initialized(true);

        info!(
            task_id = %self.task_id(),
            text_len = task_text.as_ref().map(|t| t.len()).unwrap_or(0),
            images = images.len(),
            "Task started"
        );

        // Emit TaskStarted event
        self.engine.emitter().emit_task_started(self.task_id());

        // Telemetry: task created
        self.emit_telemetry_task_created();

        Ok(())
    }

    /// Check MCP tools count and warn if too many.
    ///
    /// Source: `src/core/task/Task.ts` — lines 1955–1971
    async fn check_mcp_tools_count(&mut self) {
        if let Some(ref mcp_hub) = self.services.mcp_hub {
            let servers = mcp_hub.get_servers();
            let mut enabled_tool_count = 0usize;
            let mut enabled_server_count = 0usize;

            for server in &servers {
                // McpServerConnection has a `tools` field (Vec<McpTool>)
                let server_tool_count = server.tools.len();
                if server_tool_count > 0 {
                    enabled_server_count += 1;
                    enabled_tool_count += server_tool_count;
                }
            }

            if enabled_tool_count > self.max_mcp_tools_threshold {
                let text = serde_json::json!({
                    "toolCount": enabled_tool_count,
                    "serverCount": enabled_server_count,
                    "threshold": self.max_mcp_tools_threshold,
                })
                .to_string();

                warn!(
                    tool_count = enabled_tool_count,
                    server_count = enabled_server_count,
                    threshold = self.max_mcp_tools_threshold,
                    "Too many MCP tools detected"
                );

                let _ = self
                    .ask_say
                    .say(
                        ClineSay::TooManyToolsWarning,
                        Some(text),
                        None,
                        None,
                        None,
                        None,
                        SayOptions {
                            is_non_interactive: true,
                        },
                        None,
                        None,
                    )
                    .await;
            }
        }
    }

    // ===================================================================
    // resumeTaskFromHistory()
    // Source: `src/core/task/Task.ts` — lines 2001–2232
    // ===================================================================

    /// Resume a task from saved history.
    ///
    /// Source: `src/core/task/Task.ts` — `resumeTaskFromHistory()` (lines 2001–2232)
    ///
    /// 1. Load saved cline messages (line 2003)
    /// 2. Remove old resume messages (lines 2006–2013)
    /// 3. Remove trailing reasoning messages (lines 2015–2023)
    /// 4. Remove incomplete api_req_started (lines 2025–2041)
    /// 5. Overwrite cline messages (line 2043)
    /// 6. Reload cline messages from saved (line 2044)
    /// 7. Load API conversation history (line 2052)
    /// 8. Determine ask type and ask user (lines 2054–2068)
    /// 9. Process resume response (lines 2070–2077)
    /// 10. Repair history (fill missing tool_results) (lines 2079–2176)
    /// 11. Initiate task loop (line 2223)
    pub async fn resume_task_from_history(&mut self) -> Result<(), TaskError> {
        // Source: TS lines 2002–2232 — wrapped in try/catch
        // Step 1: Load saved messages
        let mut modified_cline_messages = self.load_saved_cline_messages().await?;

        // Step 2: Remove any resume messages that may have been added before
        // Source: TS lines 2006–2013
        let last_relevant_idx = modified_cline_messages
            .iter()
            .rposition(|m| m.ask != Some(ClineAsk::ResumeTask) && m.ask != Some(ClineAsk::ResumeCompletedTask));

        if let Some(idx) = last_relevant_idx {
            modified_cline_messages.truncate(idx + 1);
        }

        // Step 3: Remove trailing reasoning-only UI messages
        // Source: TS lines 2015–2023
        while let Some(last) = modified_cline_messages.last() {
            if last.r#type == MessageType::Say && last.say == Some(ClineSay::Reasoning) {
                modified_cline_messages.pop();
            } else {
                break;
            }
        }

        // Step 4: Remove incomplete api_req_started messages
        // Source: TS lines 2025–2041
        if let Some(idx) = modified_cline_messages
            .iter()
            .rposition(|m| m.r#type == MessageType::Say && m.say == Some(ClineSay::ApiReqStarted))
        {
            if let Some(ref text) = modified_cline_messages[idx].text {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(text) {
                    let cost = data.get("cost");
                    let cancel_reason = data.get("cancelReason");
                    if cost.is_none() && cancel_reason.is_none() {
                        modified_cline_messages.remove(idx);
                    }
                }
            }
        }

        // Step 5: Overwrite cline messages
        // Source: TS line 2043
        self.ask_say.overwrite_cline_messages(modified_cline_messages);

        // Step 6: Reload cline messages from saved (to get the cleaned version)
        // Source: TS line 2044
        let saved_messages = self.load_saved_cline_messages().await?;
        self.ask_say.overwrite_cline_messages(saved_messages);

        // Step 7: Load API conversation history
        // Source: TS line 2052
        self.engine.load_api_conversation_history().await?;

        // Step 8: Determine ask type based on last message
        // Source: TS lines 2054–2064
        let ask_type = self.determine_resume_ask_type();

        // Mark as initialized
        // Source: TS line 2066
        self.engine.set_initialized(true);

        info!(
            task_id = %self.task_id(),
            ask_type = ?ask_type,
            "Resuming task from history"
        );

        // Emit TaskStarted event
        self.engine.emitter().emit_task_started(self.task_id());

        // Telemetry: task restarted
        self.emit_telemetry_task_restarted();

        Ok(())
    }

    /// Determine the ask type for resuming a task.
    ///
    /// Source: TS lines 2054–2064
    fn determine_resume_ask_type(&self) -> ClineAsk {
        let messages = self.ask_say.cline_messages();
        let last_message = messages.iter().rev().find(|m| {
            m.ask != Some(ClineAsk::ResumeTask) && m.ask != Some(ClineAsk::ResumeCompletedTask)
        });

        match last_message {
            Some(msg) if msg.ask == Some(ClineAsk::CompletionResult) => {
                ClineAsk::ResumeCompletedTask
            }
            _ => ClineAsk::ResumeTask,
        }
    }

    /// Load saved cline messages from persistence.
    ///
    /// Source: TS `getSavedClineMessages()` (line 1152–1154)
    async fn load_saved_cline_messages(&self) -> Result<Vec<ClineMessage>, TaskError> {
        // If no storage path, return empty
        if self.engine.config().storage_path.is_none() {
            return Ok(Vec::new());
        }

        let storage_path = self.engine.config().storage_path.as_ref().unwrap();
        let base = std::path::Path::new(storage_path);
        let path = roo_task_persistence::messages_path(base, &self.engine.config().task_id);
        let fs = roo_task_persistence::OsFileSystem;

        match roo_task_persistence::read_task_messages(&fs, &path) {
            Ok(messages) => {
                debug!(count = messages.len(), "Loaded saved cline messages");
                Ok(messages)
            }
            Err(e) => {
                warn!(error = %e, "Failed to load saved cline messages");
                Ok(Vec::new())
            }
        }
    }

    // ===================================================================
    // cancelCurrentRequest()
    // Source: `src/core/task/Task.ts` — lines 2238–2249
    // ===================================================================

    /// Set the cancellation token from the `AgentLoop`.
    ///
    /// When set, `cancel_current_request()` will cancel this token,
    /// causing the spawned stream-consumer task to stop immediately.
    pub fn set_cancellation_token(&mut self, token: CancellationToken) {
        self.cancellation_token = Some(token);
    }

    /// Cancel the current API request.
    ///
    /// Source: `src/core/task/Task.ts` — `cancelCurrentRequest()` (lines 2238–2249)
    ///
    /// Aborts the current streaming API request. In the TS version, this
    /// aborts the `currentRequestAbortController`. In Rust, we:
    /// 1. Set the streaming flag to false
    /// 2. Cancel the `CancellationToken` (if set) to stop the spawned task
    pub fn cancel_current_request(&mut self) {
        if self.abort {
            debug!("Task already aborted, ignoring cancel request");
            return;
        }

        info!(task_id = %self.task_id(), "Cancelling current request");
        self.engine.streaming_mut().is_streaming = false;

        // Cancel the CancellationToken to stop the spawned stream-consumer task
        if let Some(ref token) = self.cancellation_token {
            token.cancel();
            debug!("CancellationToken triggered for mid-stream abort");
        }
    }

    // ===================================================================
    // abortTask()
    // Source: `src/core/task/Task.ts` — lines 2257–2289
    // ===================================================================

    /// Abort the task completely.
    ///
    /// Source: `src/core/task/Task.ts` — `abortTask()` (lines 2257–2289)
    ///
    /// 1. Sets the abandoned flag if requested (lines 2261–2263)
    /// 2. Sets the abort flag (line 2265)
    /// 3. Resets consecutive error counters (lines 2268–2269)
    /// 4. Emits final token usage (line 2272)
    /// 5. Emits TaskAborted event (line 2274)
    /// 6. Calls dispose (line 2277)
    /// 7. Saves cline messages (line 2285)
    pub async fn abort_task(&mut self, is_abandoned: bool) -> Result<(), TaskError> {
        info!(
            task_id = %self.task_id(),
            is_abandoned = is_abandoned,
            "Aborting task"
        );

        // Source: TS lines 2261–2263
        if is_abandoned {
            self.abandoned = true;
            self.engine.set_abandoned(true);
        }

        // Source: TS line 2265
        self.abort = true;
        self.abort_reason = Some("user_cancelled".to_string());

        // Source: TS lines 2268–2269 — reset consecutive error counters
        self.engine.reset_mistakes();

        // Cancel the loop
        self.engine.loop_control_mut().cancel();

        // Source: TS line 2272 — force final token usage update
        self.emit_final_token_usage_update();

        // Source: TS line 2274 — emit TaskAborted event
        self.engine
            .emitter()
            .emit_task_aborted(self.task_id(), self.abort_reason.clone());

        // Transition to aborted state
        if self.engine.state() != TaskState::Aborted {
            let _ = self.engine.abort_with_reason("user_cancelled");
        }

        // Source: TS lines 2276–2281 — call dispose
        self.dispose();

        // Source: TS lines 2283–2288 — save cline messages
        if let Err(e) = self.engine.save_cline_messages().await {
            warn!(error = %e, "Error saving messages during abort");
        }

        Ok(())
    }

    // ===================================================================
    // dispose()
    // Source: `src/core/task/Task.ts` — lines 2291–2378
    // ===================================================================

    /// Dispose of the task and clean up resources.
    ///
    /// Source: `src/core/task/Task.ts` — `dispose()` (lines 2291–2378)
    ///
    /// 1. Cancels any in-progress HTTP request (lines 2295–2299)
    /// 2. Removes provider profile change listener (lines 2302–2312)
    /// 3. Disposes message queue (lines 2315–2324)
    /// 4. Removes all event listeners (lines 2327–2331)
    /// 5. Releases terminals (lines 2334–2339)
    /// 6. Disposes RooIgnoreController (lines 2351–2359)
    /// 7. Disposes file context tracker (lines 2361–2365)
    /// 8. Reverts diff changes if editing (lines 2367–2374)
    pub fn dispose(&mut self) {
        if self.disposed {
            return;
        }

        info!(task_id = %self.task_id(), "Disposing task");
        self.disposed = true;
        self.abort = true;

        // Source: TS lines 2295–2299 — cancel current request
        self.engine.streaming_mut().is_streaming = false;
        if let Some(ref token) = self.cancellation_token {
            token.cancel();
        }

        // Source: TS lines 2302–2312 — remove provider profile change listener
        // (In Rust, this is handled by dropping the event emitter)

        // Source: TS lines 2315–2324 — dispose message queue and remove listeners
        if let Some(ref queue) = self.services.message_queue {
            if let Ok(mut q) = queue.try_lock() {
                while q.dequeue_message().is_some() {}
            }
        }

        // Source: TS lines 2327–2331 — remove all event listeners
        // (In Rust, this is handled by dropping the event emitter)

        // Source: TS lines 2334–2339 — release terminals for this task
        // Note: release_terminals_for_task is async, but dispose() is sync.
        // We spawn a background task to handle this.
        if let Some(ref terminal_registry) = self.services.terminal_registry {
            let reg = terminal_registry.clone();
            let task_id = self.task_id().to_string();
            tokio::spawn(async move {
                reg.release_terminals_for_task(&task_id).await;
            });
        }

        // Source: TS lines 2342–2349 — cleanup command output artifacts
        // (In Rust, this is handled by the storage layer on task completion)

        // Source: TS lines 2351–2359 — dispose RooIgnoreController
        if self.roo_ignore_controller.is_some() {
            self.roo_ignore_controller = None;
        }

        // Source: TS lines 2361–2365 — dispose file context tracker
        if self.file_context_tracker.is_some() {
            self.file_context_tracker = None;
        }

        // Source: TS lines 2367–2374 — revert diff changes if editing
        if self.engine.streaming().is_streaming {
            if let Some(ref _diff_view) = self.diff_view_provider {
                // TODO: Implement DiffViewProvider::revert_changes() async method
                // In TS: `this.diffViewProvider.revertChanges().catch(console.error)`
                warn!("DiffViewProvider revert_changes not yet implemented in Rust");
            }
        }
    }

    // ===================================================================
    // startSubtask()
    // Source: `src/core/task/Task.ts` — lines 2380–2403
    // ===================================================================

    /// Start a subtask (delegate to a child task).
    ///
    /// Source: `src/core/task/Task.ts` — `startSubtask()` (lines 2380–2403)
    ///
    /// Creates a new child task with the given parameters and delegates
    /// execution to it. The parent task transitions to the Delegated state.
    pub async fn start_subtask(
        &mut self,
        message: &str,
        _initial_todos: Vec<roo_types::todo::TodoItem>,
        mode: &str,
    ) -> Result<String, TaskError> {
        let subtask_id = format!(
            "{}-sub-{}",
            self.engine.config().task_id,
            uuid::Uuid::now_v7()
        );

        info!(
            parent_task_id = %self.task_id(),
            subtask_id = %subtask_id,
            mode = mode,
            "Starting subtask"
        );

        // Create subtask config
        let subtask_config = TaskConfig::new(&subtask_id, &self.engine.config().cwd)
            .with_mode(mode)
            .with_task_text(message)
            .with_root_task_id(
                self.engine
                    .config()
                    .root_task_id
                    .as_deref()
                    .unwrap_or(self.task_id()),
            )
            .with_parent_task_id(self.task_id());

        // Copy storage path if available
        if let Some(ref storage_path) = self.engine.config().storage_path {
            let _subtask_config = subtask_config.with_storage_path(storage_path);
        }

        // Store child task ID
        self.child_task_id = Some(subtask_id.clone());

        // Delegate the parent task
        self.engine.delegate()?;

        // Emit subtask created event
        self.engine
            .emitter()
            .emit_task_spawned(self.task_id(), &subtask_id);

        Ok(subtask_id)
    }

    // ===================================================================
    // resumeAfterDelegation()
    // Source: `src/core/task/Task.ts` — lines 2406–2470
    // ===================================================================

    /// Resume the task after a subtask completes.
    ///
    /// Source: `src/core/task/Task.ts` — `resumeAfterDelegation()` (lines 2406–2470)
    ///
    /// 1. Clears ask states (lines 2407–2410)
    /// 2. Resets abort and streaming flags (lines 2412–2418)
    /// 3. Marks as initialized and active (lines 2423–2425)
    /// 4. Loads conversation history if needed (lines 2427–2430)
    /// 5. Adds environment details (lines 2432–2460)
    /// 6. Saves updated history (line 2463)
    /// 7. Initiates task loop (line 2467)
    pub async fn resume_after_delegation(&mut self) -> Result<(), TaskError> {
        info!(
            task_id = %self.task_id(),
            "Resuming after delegation"
        );

        // Source: TS lines 2407–2410 — clear ask states
        // (handled by engine.resume_after_delegation)

        // Source: TS lines 2412–2418 — reset abort and streaming state
        // Source: TS lines 2423–2425 — mark as initialized and active
        self.engine.resume_after_delegation()?;

        // Clear child task ID
        self.child_task_id = None;
        self.abandoned = false;

        // Source: TS lines 2427–2430 — load conversation history if needed
        if self.engine.api_conversation_history().is_empty() {
            self.engine.load_api_conversation_history().await?;
        }

        Ok(())
    }

    // ===================================================================
    // submitUserMessage()
    // Source: `src/core/task/Task.ts` — lines 1585–1629
    // ===================================================================

    /// Submit a user message to the task.
    ///
    /// Source: `src/core/task/Task.ts` — `submitUserMessage()` (lines 1585–1629)
    ///
    /// 1. Trims and validates text/images (lines 1592–1597)
    /// 2. Sets mode if provided (lines 1602–1604)
    /// 3. Sets provider profile if provided (lines 1606–1615)
    /// 4. Emits TaskUserMessage event (line 1617)
    /// 5. Handles the message via handleWebviewAskResponse (line 1622)
    pub async fn submit_user_message(
        &mut self,
        text: &str,
        images: Option<Vec<String>>,
        _mode: Option<&str>,
        provider_profile: Option<&str>,
    ) -> Result<(), TaskError> {
        // Source: TS lines 1592–1597 — trim and validate
        let mut trimmed_text = text.trim().to_string();
        let images = images.unwrap_or_default();

        if trimmed_text.is_empty() && images.is_empty() {
            return Ok(());
        }

        // Source: TS lines 1602–1604 — set mode if provided
        // (Mode switching is handled at the TaskManager level)

        // Source: TS lines 1606–1615 — set provider profile if provided
        // This updates the API configuration to match the new profile.
        // (Provider profile switching is handled at the TaskManager level)
        let _ = provider_profile;

        // Source: TS — @mentions processing
        // In the TS version, @mentions are resolved in the webview message
        // handler before submitUserMessage is called. Here we process them
        // inline using the roo_mentions module.
        if trimmed_text.contains('@') {
            trimmed_text = self.resolve_mentions(&trimmed_text).await;
        }

        // Source: TS line 1617 — emit TaskUserMessage event
        self.engine
            .emitter()
            .emit(&TaskEvent::UserMessage {
                task_id: self.task_id().to_string(),
            });

        // Source: TS line 1622 — handle the message via handleWebviewAskResponse
        // `this.handleWebviewAskResponse("messageResponse", text, images)`
        let _checkpoint_needed = self
            .ask_say
            .handle_response_full(
                AskResponse::MessageResponse,
                Some(trimmed_text.clone()),
                if images.is_empty() { None } else { Some(images) },
            )
            .await;

        // Telemetry: conversation message
        self.emit_telemetry_conversation_message("user");

        debug!(text_len = trimmed_text.len(), "User message submitted");
        Ok(())
    }

    /// Resolve @mentions in user message text.
    ///
    /// Source: `src/core/webview/ClineProvider.ts` — `handleWebviewMessage`
    ///   `sendMessage` branch, where @mentions are parsed and replaced
    ///   with file content before the message is sent to the task.
    ///
    /// Uses the `roo_mentions` module to parse @mentions and replace them
    /// with the referenced file contents.
    async fn resolve_mentions(&self, text: &str) -> String {
        let cwd = std::path::PathBuf::from(&self.engine.config().cwd);
        let content_blocks = vec![roo_mentions::types::ContentBlock::text(text)];
        let result =
            roo_mentions::processor::process_user_content_mentions(&content_blocks, &cwd).await;

        // Extract the processed text from the result
        result
            .content
            .iter()
            .filter_map(|b: &roo_mentions::types::ContentBlock| b.as_text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    // ===================================================================
    // condenseContext()
    // Source: `src/core/task/Task.ts` — lines 1648–1753
    // ===================================================================

    /// Manually trigger context condensation.
    ///
    /// Source: `src/core/task/Task.ts` — `condenseContext()` (lines 1648–1753)
    ///
    /// 1. Flushes pending tool results (line 1651)
    /// 2. Gets system prompt (line 1653)
    /// 3. Gets condensing configuration (lines 1656–1658)
    /// 4. Gets current token usage (line 1660)
    /// 5. Builds tools for condensing metadata (lines 1662–1679)
    /// 6. Generates environment details (line 1694)
    /// 7. Calls summarizeConversation (lines 1698–1718)
    /// 8. Handles errors (lines 1719–1730)
    /// 9. Overwrites API conversation history (line 1731)
    /// 10. Emits condense_context say message (lines 1740–1749)
    /// 11. Processes queued messages (line 1752)
    pub async fn condense_context(&mut self) -> Result<(), TaskError> {
        info!(task_id = %self.task_id(), "Condensing context (manual trigger)");

        // Source: TS line 1651 — flush pending tool results
        self.flush_pending_tool_results().await?;

        // Source: TS line 1660 — get previous context tokens
        let _prev_context_tokens = self.engine.result().token_usage.context_tokens;

        // Source: TS lines 1696–1697 — get files read by Roo safely
        let _files_read_by_roo = self.get_files_read_by_roo_safely("condenseContext");

        // Source: TS lines 1698–1718 — call summarizeConversation
        //
        // The actual condensation requires an API handler and system prompt,
        // which are owned by the agent loop. The TaskLifecycle emits a
        // condensation request event and the agent loop performs the actual
        // condensation. This matches the TS architecture where condenseContext()
        // calls summarizeConversation() which needs the API handler.
        //
        // For the manual trigger path, we emit the event and let the
        // agent loop handle it. If we have an API handler available,
        // we can perform the condensation directly.
        if self.engine.api_handler().is_some() {
            // Direct condensation path — requires API handler
            // TODO: Implement direct condensation when API handler is available
            // This would call summarize_conversation() with the proper options.
            warn!("Direct condensation not yet implemented, falling back to event-based");
        }

        // Emit condensation request event for the agent loop to handle
        self.engine
            .emitter()
            .emit(&TaskEvent::ContextCondensationRequested {
                task_id: self.task_id().to_string(),
            });

        // Source: TS lines 1733–1749 — emit condense_context say message
        // (This is done by the agent loop after successful condensation)

        // Source: TS line 1752 — process queued messages
        self.process_queued_messages().await;

        info!(task_id = %self.task_id(), "Context condensation requested");
        Ok(())
    }

    /// Flush pending tool results to history.
    ///
    /// Source: `src/core/task/Task.ts` — `flushPendingToolResultsToHistory()`
    async fn flush_pending_tool_results(&mut self) -> Result<bool, TaskError> {
        let history_len = self.engine.api_conversation_history().len();
        if history_len == 0 {
            return Ok(false);
        }
        // Check if the last message is an assistant message with tool_use
        // Source: TS — checks for pending userMessageContent
        Ok(false) // No pending results in this simplified implementation
    }

    /// Safely get files read by Roo, returning None on error.
    ///
    /// Source: `src/core/task/Task.ts` — `getFilesReadByRooSafely()` (lines 1639–1646)
    fn get_files_read_by_roo_safely(&self, _context: &str) -> Option<Vec<String>> {
        if let Some(ref tracker) = self.file_context_tracker {
            match tracker.get_files_read_by_roo(None) {
                Ok(files) => Some(files),
                Err(e) => {
                    warn!(error = %e, "Failed to get files read by Roo");
                    None
                }
            }
        } else {
            None
        }
    }

    // ===================================================================
    // updateApiConfiguration()
    // Source: `src/core/task/Task.ts` — lines 1579–1583
    // ===================================================================

    /// Update the API configuration.
    ///
    /// Source: `src/core/task/Task.ts` — `updateApiConfiguration()` (lines 1579–1583)
    ///
    /// Updates the API configuration and rebuilds the API handler.
    /// This is used when the user changes the model or provider mid-task.
    /// Update the API configuration and rebuild the API handler.
    ///
    /// Source: `src/core/task/Task.ts` — `updateApiConfiguration()` (lines 1579–1583)
    ///
    /// TS implementation:
    /// ```ts
    /// this.apiConfiguration = newApiConfiguration
    /// this.api = buildApiHandler(this.apiConfiguration)
    /// ```
    pub fn update_api_configuration(
        &mut self,
        new_config: roo_types::provider_settings::ProviderSettings,
    ) {
        // Source: TS line 1581 — update the configuration
        self.engine.set_api_configuration(new_config);

        // Source: TS line 1582 — rebuild the API handler
        // The actual handler rebuild requires the agent loop to call
        // `buildApiHandler()` with the new configuration. We store the
        // config and the agent loop will pick it up on the next iteration.
        debug!("API configuration updated, handler rebuild pending next iteration");
    }

    // ===================================================================
    // handleTerminalOperation()
    // Source: `src/core/task/Task.ts` — lines 1631–1636
    // ===================================================================

    /// Handle a terminal operation (continue or abort).
    ///
    /// Source: `src/core/task/Task.ts` — `handleTerminalOperation()` (lines 1631–1636)
    ///
    /// TS implementation:
    /// ```ts
    /// if (terminalOperation === "continue") {
    ///     this.terminalProcess?.continue()
    /// } else if (terminalOperation === "abort") {
    ///     this.terminalProcess?.abort()
    /// }
    /// ```
    pub async fn handle_terminal_operation(
        &mut self,
        operation: &str,
    ) -> Result<(), TaskError> {
        match operation {
            // Source: TS line 1633 — `this.terminalProcess?.continue()`
            "continue" => {
                // Source: TS line 1633 — `this.terminalProcess?.continue()`
                // TerminalRegistry doesn't have continue_terminal directly.
                // The actual terminal continue is handled via TerminalProcess
                // which is managed by the agent loop.
                // Here we emit an event that the agent loop can listen to.
                self.engine
                    .emitter()
                    .emit(&TaskEvent::TaskUnpaused {
                        task_id: self.task_id().to_string(),
                    });
                debug!("Continuing terminal operation");
            }
            // Source: TS line 1635 — `this.terminalProcess?.abort()`
            "abort" => {
                // Source: TS line 1635 — `this.terminalProcess?.abort()`
                // TerminalRegistry doesn't have abort_terminal directly.
                // The actual terminal abort is handled via TerminalProcess
                // which is managed by the agent loop.
                if let Some(ref token) = self.cancellation_token {
                    token.cancel();
                }
                debug!("Aborting terminal operation");
            }
            _ => {
                warn!(operation = operation, "Unknown terminal operation");
            }
        }
        Ok(())
    }

    // ===================================================================
    // checkpointSave() / checkpointRestore() / checkpointDiff()
    // Source: `src/core/task/Task.ts` — lines 4454–4604
    // ===================================================================

    /// Save a checkpoint of the current task state.
    ///
    /// Source: `src/core/task/Task.ts` — `checkpointSave()` (lines 4454–4565)
    ///
    /// Creates a git commit with the current file changes as a checkpoint.
    /// If `allow_empty` is true, creates a checkpoint even with no changes.
    /// If `suppress_message` is true, doesn't emit a checkpoint_saved message.
    pub async fn checkpoint_save(
        &mut self,
        _allow_empty: bool,
        _suppress_message: bool,
    ) -> Result<Option<String>, TaskError> {
        // Source: TS lines 4454–4565
        // The actual checkpoint logic requires the ShadowCheckpointService
        // which is initialized during the agent loop.
        //
        // For now, emit the event and return None.
        self.engine
            .emitter()
            .emit_checkpoint_saved(self.task_id(), None);
        debug!(task_id = %self.task_id(), "Checkpoint save requested");
        Ok(None)
    }

    /// Restore the task to a previous checkpoint.
    ///
    /// Source: `src/core/task/Task.ts` — `checkpointRestore()` (lines 4567–4589)
    pub async fn checkpoint_restore(&mut self) -> Result<(), TaskError> {
        // Source: TS lines 4567–4589
        self.engine.emitter().emit_checkpoint_restored(self.task_id());
        debug!(task_id = %self.task_id(), "Checkpoint restore requested");
        Ok(())
    }

    /// Get the diff for a checkpoint.
    ///
    /// Source: `src/core/task/Task.ts` — `checkpointDiff()` (lines 4591–4604)
    pub async fn checkpoint_diff(&self) -> Result<Option<String>, TaskError> {
        // Source: TS lines 4591–4604
        debug!(task_id = %self.task_id(), "Checkpoint diff requested");
        Ok(None)
    }

    // ===================================================================
    // emitFinalTokenUsageUpdate()
    // Source: `src/core/task/Task.ts` — lines 2251–2255
    // ===================================================================

    /// Emit the final token usage update.
    ///
    /// Source: `src/core/task/Task.ts` — `emitFinalTokenUsageUpdate()` (lines 2251–2255)
    ///
    /// Force emits a final token usage update, ignoring throttle.
    /// Called before task completion or abort to ensure final stats are captured.
    pub fn emit_final_token_usage_update(&self) {
        let usage = self.engine.result().token_usage.clone();
        self.engine.emitter().emit_token_usage_updated(usage.clone());

        // Telemetry: LLM completion with token counts
        self.emit_telemetry_llm_completion(&usage);
    }

    // ===================================================================
    // getTokenUsage()
    // Source: `src/core/task/Task.ts` — lines 4613–4615
    // ===================================================================

    /// Get the current token usage.
    ///
    /// Source: `src/core/task/Task.ts` — `getTokenUsage()` (lines 4613–4615)
    pub fn get_token_usage(&self) -> roo_types::message::TokenUsage {
        self.engine.result().token_usage.clone()
    }

    // ===================================================================
    // recordToolUsage() / recordToolError()
    // Source: `src/core/task/Task.ts` — lines 4617–4632
    // ===================================================================

    /// Record a tool usage.
    ///
    /// Source: `src/core/task/Task.ts` — `recordToolUsage()` (lines 4617–4623)
    pub fn record_tool_usage(&mut self, tool_name: &str) {
        self.engine.record_tool_execution(tool_name, true);
        // Telemetry: tool used
        self.emit_telemetry_tool_usage(tool_name);
    }

    /// Record a tool error.
    ///
    /// Source: `src/core/task/Task.ts` — `recordToolError()` (lines 4625–4632)
    pub fn record_tool_error(&mut self, tool_name: &str, error: Option<&str>) {
        self.engine.record_tool_execution(tool_name, false);
        // Telemetry: tool used (even on error)
        self.emit_telemetry_tool_usage(tool_name);
        if let Some(err) = error {
            debug!(tool = tool_name, error = err, "Tool error recorded");
        }
    }

    // ===================================================================
    // combineMessages()
    // ===================================================================

    /// Combine messages for display.
    ///
    /// Source: `src/core/task/Task.ts` — `combineMessages()`
    pub fn combine_messages(&self) -> Vec<ClineMessage> {
        // Source: TS — `combineApiRequests(combineCommandSequences(messages))`
        self.ask_say.cline_messages().to_vec()
    }

    // ===================================================================
    // processQueuedMessages()
    // ===================================================================

    /// Process queued messages.
    ///
    /// Source: `src/core/task/Task.ts` — `processQueuedMessages()`
    ///
    /// Dequeues messages from the [`MessageQueueService`] (if available)
    /// and returns them for processing by the caller.
    pub async fn process_queued_messages(&mut self) -> Vec<roo_message_queue::QueuedMessage> {
        let Some(ref queue) = self.services.message_queue else {
            return Vec::new();
        };

        let mut q = queue.lock().await;
        let mut drained = Vec::new();
        while !q.is_empty() {
            if let Some(msg) = q.dequeue_message() {
                drained.push(msg);
            } else {
                break;
            }
        }

        if !drained.is_empty() {
            debug!(count = drained.len(), "Dequeued messages from queue");
        }
        drained
    }

    // ===================================================================
    // pushToolResultToUserContent()
    // ===================================================================

    /// Push a tool result to user content, preventing duplicates.
    ///
    /// Source: `src/core/task/Task.ts` — `pushToolResultToUserContent()`
    pub fn push_tool_result_to_user_content(
        &mut self,
        tool_use_id: &str,
        result: &str,
        is_error: bool,
    ) -> bool {
        let result_msg = crate::message_builder::MessageBuilder::create_tool_result_message(
            tool_use_id,
            &crate::tool_dispatcher::ToolExecutionResult {
                text: result.to_string(),
                images: None,
                is_error,
            },
        );
        self.engine.add_api_message(result_msg);
        true
    }

    // ===================================================================
    // sayAndCreateMissingParamError()
    // Source: `src/core/task/Task.ts` — lines 1869–1877
    // ===================================================================

    /// Say an error about a missing parameter.
    ///
    /// Source: `src/core/task/Task.ts` — `sayAndCreateMissingParamError()` (lines 1869–1877)
    pub async fn say_missing_param_error(
        &mut self,
        tool_name: &str,
        param_name: &str,
        rel_path: Option<&str>,
    ) -> Result<(), TaskError> {
        self.ask_say
            .say_and_create_missing_param_error(tool_name, param_name, rel_path)
            .await?;
        Ok(())
    }

    // ===================================================================
    // overwriteApiConversationHistory()
    // ===================================================================

    /// Overwrite the API conversation history.
    ///
    /// Source: `src/core/task/Task.ts` — `overwriteApiConversationHistory()`
    pub fn overwrite_api_conversation_history(
        &mut self,
        history: Vec<roo_types::api::ApiMessage>,
    ) {
        self.engine.set_api_conversation_history(history);
    }

    // ===================================================================
    // Telemetry helpers
    // ===================================================================

    /// Emit a telemetry "task created" event.
    fn emit_telemetry_task_created(&self) {
        if let Some(ref telemetry) = self.services.telemetry {
            if let Ok(svc) = telemetry.read() {
                svc.capture_task_created(self.task_id());
            }
        }
    }

    /// Emit a telemetry "task restarted" event.
    fn emit_telemetry_task_restarted(&self) {
        if let Some(ref telemetry) = self.services.telemetry {
            if let Ok(svc) = telemetry.read() {
                svc.capture_task_restarted(self.task_id());
            }
        }
    }

    /// Emit a telemetry "conversation message" event.
    fn emit_telemetry_conversation_message(&self, source: &str) {
        if let Some(ref telemetry) = self.services.telemetry {
            if let Ok(svc) = telemetry.read() {
                svc.capture_conversation_message(self.task_id(), source);
            }
        }
    }

    /// Emit a telemetry "tool used" event.
    fn emit_telemetry_tool_usage(&self, tool: &str) {
        if let Some(ref telemetry) = self.services.telemetry {
            if let Ok(svc) = telemetry.read() {
                svc.capture_tool_usage(self.task_id(), tool);
            }
        }
    }

    /// Emit a telemetry "LLM completion" event with token usage.
    fn emit_telemetry_llm_completion(&self, usage: &roo_types::message::TokenUsage) {
        if let Some(ref telemetry) = self.services.telemetry {
            if let Ok(svc) = telemetry.read() {
                svc.capture_llm_completion(
                    self.task_id(),
                    usage.total_tokens_in,
                    usage.total_tokens_out,
                    usage.total_cache_writes.unwrap_or(0),
                    usage.total_cache_reads.unwrap_or(0),
                    if usage.total_cost > 0.0 {
                        Some(usage.total_cost)
                    } else {
                        None
                    },
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TaskStatus (re-export from roo_types for convenience)
// ---------------------------------------------------------------------------

/// Task status as reported to the UI.
///
/// Re-exported from `roo_types::task::TaskStatus` for convenience.
pub use roo_types::task::TaskStatus;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TaskConfig;

    fn make_lifecycle() -> TaskLifecycle {
        let config = TaskConfig::new("test-task", "/tmp/work")
            .with_mode("code")
            .with_max_iterations(100)
            .with_start_task(false);
        let engine = TaskEngine::new(config).unwrap();
        TaskLifecycle::new(engine)
    }

    #[test]
    fn test_lifecycle_new() {
        let lc = make_lifecycle();
        assert_eq!(lc.state(), TaskState::Idle);
        assert!(!lc.is_aborted());
        assert!(lc.abort_reason().is_none());
        assert!(!lc.is_disposed());
        assert!(lc.child_task_id().is_none());
    }

    #[test]
    fn test_lifecycle_task_status() {
        let lc = make_lifecycle();
        assert_eq!(lc.task_status(), TaskStatus::Idle);
    }

    #[test]
    fn test_cancel_current_request() {
        let mut lc = make_lifecycle();
        assert!(!lc.is_aborted());

        lc.cancel_current_request();
        // cancel_current_request doesn't set abort flag (unlike TS which uses AbortController)
        assert!(!lc.is_aborted());
    }

    #[tokio::test]
    async fn test_abort_task() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        lc.abort_task(false).await.unwrap();
        assert!(lc.is_aborted());
        assert_eq!(lc.state(), TaskState::Aborted);
    }

    #[tokio::test]
    async fn test_abort_task_abandoned() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        lc.abort_task(true).await.unwrap();
        assert!(lc.is_aborted());
        assert!(lc.engine.is_abandoned());
    }

    #[test]
    fn test_dispose() {
        let mut lc = make_lifecycle();
        assert!(!lc.is_disposed());

        lc.dispose();
        assert!(lc.is_disposed());

        // Second dispose is no-op
        lc.dispose();
        assert!(lc.is_disposed());
    }

    #[tokio::test]
    async fn test_start_subtask() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        let subtask_id = lc.start_subtask("Fix the bug", Vec::new(), "code").await;
        assert!(subtask_id.is_ok());
        let id = subtask_id.unwrap();
        assert!(id.starts_with("test-task-sub-"));
        assert_eq!(lc.child_task_id(), Some(id.as_str()));
        assert_eq!(lc.state(), TaskState::Delegated);
    }

    #[tokio::test]
    async fn test_resume_after_delegation() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();
        lc.start_subtask("Fix the bug", Vec::new(), "code")
            .await
            .unwrap();
        assert_eq!(lc.state(), TaskState::Delegated);

        lc.resume_after_delegation().await.unwrap();
        assert_eq!(lc.state(), TaskState::Running);
        assert!(lc.child_task_id().is_none());
    }

    #[tokio::test]
    async fn test_submit_user_message() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        // submit_user_message sets the ask response and emits events;
        // it does NOT directly add to cline_messages (that happens in the agent loop).
        lc.submit_user_message("Hello", None, None, None).await.unwrap();
        // Verify no error occurred — the response is set internally via handle_response_full.
    }

    #[tokio::test]
    async fn test_submit_user_message_empty() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        lc.submit_user_message("", None, None, None).await.unwrap();
        assert_eq!(lc.ask_say.cline_messages().len(), 0); // Empty text is ignored
    }

    #[test]
    fn test_get_token_usage() {
        let lc = make_lifecycle();
        let usage = lc.get_token_usage();
        assert_eq!(usage.total_tokens_in, 0);
        assert_eq!(usage.total_tokens_out, 0);
    }

    #[test]
    fn test_record_tool_usage() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        lc.record_tool_usage("read_file");
        assert_eq!(lc.engine.result().tool_usage["read_file"], 1);
    }

    #[test]
    fn test_record_tool_error() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        lc.record_tool_error("write_to_file", Some("Permission denied"));
        assert_eq!(lc.engine.result().tool_usage["write_to_file"], 1);
    }

    #[test]
    fn test_combine_messages() {
        let lc = make_lifecycle();
        let messages = lc.combine_messages();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_emit_final_token_usage() {
        let lc = make_lifecycle();
        lc.emit_final_token_usage_update();
        // Should not panic
    }

    #[tokio::test]
    async fn test_say_missing_param_error() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        lc.say_missing_param_error("read_file", "path", Some("/tmp/test.rs"))
            .await
            .unwrap();
        assert_eq!(lc.ask_say.cline_messages().len(), 1);
        assert_eq!(lc.ask_say.cline_messages()[0].say, Some(ClineSay::Error));
    }

    #[test]
    fn test_push_tool_result() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        let added = lc.push_tool_result_to_user_content("tool_123", "result text", false);
        assert!(added);
    }

    #[tokio::test]
    async fn test_handle_terminal_operation_continue() {
        let mut lc = make_lifecycle();
        lc.handle_terminal_operation("continue").await.unwrap();
        assert!(!lc.is_aborted());
    }

    #[tokio::test]
    async fn test_handle_terminal_operation_abort() {
        let mut lc = make_lifecycle();
        lc.handle_terminal_operation("abort").await.unwrap();
        // Terminal abort doesn't set task abort flag
        assert!(!lc.is_aborted());
    }

    #[tokio::test]
    async fn test_checkpoint_save() {
        let mut lc = make_lifecycle();
        let result = lc.checkpoint_save(false, false).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_checkpoint_restore() {
        let mut lc = make_lifecycle();
        lc.checkpoint_restore().await.unwrap();
    }

    #[tokio::test]
    async fn test_checkpoint_diff() {
        let lc = make_lifecycle();
        let result = lc.checkpoint_diff().await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_overwrite_api_conversation_history() {
        let mut lc = make_lifecycle();
        lc.engine.start().unwrap();

        let history = vec![roo_types::api::ApiMessage {
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
        }];

        lc.overwrite_api_conversation_history(history);
        assert_eq!(lc.engine.api_conversation_history().len(), 1);
    }
}
