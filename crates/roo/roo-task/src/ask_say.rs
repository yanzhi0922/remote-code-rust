//! Interactive ask/say flow for task communication.
//!
//! Faithfully replicates the `ask()`, `say()`, and related methods from
//! `src/core/task/Task.ts` (lines 1264–1877).
//!
//! ## Method mapping
//!
//! | Rust method                            | TS source                            | Lines       |
//! |----------------------------------------|--------------------------------------|-------------|
//! | `AskSayHandler::ask()`                 | `Task.ask()`                         | 1264–1499   |
//! | `AskSayHandler::say()`                 | `Task.say()`                         | 1755–1867   |
//! | `AskSayHandler::handle_response()`     | `Task.handleWebviewAskResponse()`    | 1501–1548   |
//! | `AskSayHandler::approve_ask()`         | `Task.approveAsk()`                  | 1561–1563   |
//! | `AskSayHandler::deny_ask()`            | `Task.denyAsk()`                     | 1565–1567   |
//! | `AskSayHandler::supersede_pending_ask()` | `Task.supersedePendingAsk()`       | 1569–1571   |
//! | `AskSayHandler::cancel_auto_approval_timeout()` | `Task.cancelAutoApprovalTimeout()` | 1554–1559 |
//! | `AskSayHandler::say_and_create_missing_param_error()` | `Task.sayAndCreateMissingParamError()` | 1869–1877 |

use std::sync::Arc;

use tokio::sync::{watch, Mutex};
use tracing::{debug, warn};

use roo_types::message::{
    ClineAsk, ClineMessage, ClineSay, ContextCondense, ContextTruncation, MessageType,
    ToolProgressStatus,
};

use crate::events::TaskEventEmitter;
use crate::types::TaskError;

// ---------------------------------------------------------------------------
// AskResponse
// ---------------------------------------------------------------------------

/// Response from a user to an ask prompt.
///
/// Source: `src/shared/WebviewMessage.ts` — `ClineAskResponse`
#[derive(Debug, Clone, PartialEq)]
pub enum AskResponse {
    /// User clicked "Yes" / approved.
    YesButtonClicked,
    /// User clicked "No" / denied.
    NoButtonClicked,
    /// User provided a message response.
    MessageResponse,
}

// ---------------------------------------------------------------------------
// AskIgnoredError
// ---------------------------------------------------------------------------

/// Error indicating the ask was ignored because it was a partial update
/// or was superseded by a newer message.
///
/// Source: `src/core/task/AskIgnoredError.ts`
#[derive(Debug, Clone)]
pub struct AskIgnoredError {
    pub reason: String,
}

impl std::fmt::Display for AskIgnoredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AskIgnoredError: {}", self.reason)
    }
}

impl std::error::Error for AskIgnoredError {}

// ---------------------------------------------------------------------------
// AskResult
// ---------------------------------------------------------------------------

/// Result of an ask operation.
///
/// Source: `src/core/task/Task.ts` — return type of `ask()`
#[derive(Debug, Clone)]
pub struct AskResult {
    pub response: AskResponse,
    pub text: Option<String>,
    pub images: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// SayOptions
// ---------------------------------------------------------------------------

/// Options for the `say()` method.
///
/// Source: `src/core/task/Task.ts` — `say()` options parameter (line 1762–1764)
#[derive(Debug, Clone, Default)]
pub struct SayOptions {
    /// Whether this message is non-interactive (does not update lastMessageTs).
    ///
    /// Source: TS `options.isNonInteractive`
    pub is_non_interactive: bool,
}

// ---------------------------------------------------------------------------
// AutoApprovalDecision
// ---------------------------------------------------------------------------

/// Decision returned by the auto-approval checker.
///
/// Source: `src/core/task/Task.ts` — `checkAutoApproval()` return value (line 1366–1380)
#[derive(Debug, Clone)]
pub enum AutoApprovalDecision {
    /// The ask should be presented to the user (no auto-action).
    Ask,
    /// The ask should be auto-approved.
    Approve,
    /// The ask should be auto-denied.
    Deny,
    /// The ask should be auto-approved after a timeout.
    Timeout {
        /// Timeout in milliseconds.
        timeout_ms: u64,
        /// The response to send when the timeout fires.
        response: AskResponse,
        /// Text to include in the response.
        text: Option<String>,
        /// Images to include in the response.
        images: Option<Vec<String>>,
    },
}

/// Callback type for auto-approval checking.
///
/// Receives (ask_type, text, is_protected) and returns a decision.
pub type AutoApprovalChecker =
    Box<dyn Fn(ClineAsk, Option<&str>, Option<bool>) -> AutoApprovalDecision + Send + Sync>;

// ---------------------------------------------------------------------------
// AskSayHandler
// ---------------------------------------------------------------------------

/// Handler for the ask/say interactive communication flow.
///
/// Source: `src/core/task/Task.ts` — `ask()`, `say()`, and related methods
///
/// The ask/say flow works as follows:
/// - `say()` emits a message to the UI without waiting for a response
/// - `ask()` emits a message and waits for a user response
/// - `handle_response()` fulfills the pending ask with a user response
///
/// In the TS implementation, `ask()` uses `pWaitFor` to poll for a response.
/// In Rust, we use `tokio::sync::watch` for efficient async waiting.
pub struct AskSayHandler {
    // ── Messages ──────────────────────────────────────────────────────────
    /// Messages emitted during the task (clineMessages).
    ///
    /// Source: TS `this.clineMessages`
    cline_messages: Vec<ClineMessage>,

    // ── Ask state ─────────────────────────────────────────────────────────
    /// Current pending ask response.
    ///
    /// Source: TS `this.askResponse`
    ask_response: Arc<Mutex<Option<AskResult>>>,
    /// Timestamp of the last message.
    ///
    /// Source: TS `this.lastMessageTs`
    last_message_ts: Option<f64>,

    // ── Signal channel ────────────────────────────────────────────────────
    /// Watch channel for signaling ask response availability.
    ask_signal: watch::Sender<bool>,
    ask_signal_rx: watch::Receiver<bool>,

    // ── Auto-approval ─────────────────────────────────────────────────────
    /// Whether there is a pending auto-approval timeout.
    ///
    /// Source: TS `this.autoApprovalTimeoutRef` (line 1374)
    auto_approval_timeout_active: bool,
    /// The auto-approval decision to apply on timeout.
    ///
    /// Source: TS `approval.fn()` called in setTimeout (line 1375)
    auto_approval_pending: Option<AutoApprovalDecision>,

    // ── State tracking (idle/resumable/interactive) ────────────────────────
    /// The current idle ask message, if any.
    ///
    /// Source: TS `this.idleAsk` (line 1423)
    idle_ask: Option<ClineMessage>,
    /// The current resumable ask message, if any.
    ///
    /// Source: TS `this.resumableAsk` (line 1412)
    resumable_ask: Option<ClineMessage>,
    /// The current interactive ask message, if any.
    ///
    /// Source: TS `this.interactiveAsk` (line 1400)
    interactive_ask: Option<ClineMessage>,

    // ── Task context ──────────────────────────────────────────────────────
    /// Task ID for event emission.
    task_id: Option<String>,
    /// Event emitter for state change events.
    event_emitter: Option<TaskEventEmitter>,
}

impl AskSayHandler {
    /// Create a new ask/say handler.
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            cline_messages: Vec::new(),
            ask_response: Arc::new(Mutex::new(None)),
            last_message_ts: None,
            ask_signal: tx,
            ask_signal_rx: rx,
            auto_approval_timeout_active: false,
            auto_approval_pending: None,
            idle_ask: None,
            resumable_ask: None,
            interactive_ask: None,
            task_id: None,
            event_emitter: None,
        }
    }

    /// Set the task ID for event emission.
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Set the event emitter for state change events.
    pub fn with_event_emitter(mut self, emitter: TaskEventEmitter) -> Self {
        self.event_emitter = Some(emitter);
        self
    }

    // ===================================================================
    // say()
    // Source: `src/core/task/Task.ts` — `say()` (lines 1755–1867)
    // ===================================================================

    /// Emit a message to the UI without waiting for a response.
    ///
    /// Source: `src/core/task/Task.ts` — `say()` (lines 1755–1867)
    ///
    /// Creates a ClineMessage with type "say" and adds it to the message list.
    /// Supports partial updates (streaming), context condensation/truncation
    /// metadata, and non-interactive mode.
    ///
    /// # Arguments
    /// * `say_type` - The type of message to emit
    /// * `text` - Optional text content
    /// * `images` - Optional base64-encoded images
    /// * `partial` - Whether this is a partial (streaming) update:
    ///   - `None` → new complete message
    ///   - `Some(true)` → new or updated partial message
    ///   - `Some(false)` → finalize existing partial or create new complete
    /// * `checkpoint` - Optional checkpoint data
    /// * `progress_status` - Optional progress status for tool operations
    /// * `options` - Options (e.g., isNonInteractive)
    /// * `context_condense` - Optional context condensation metadata
    /// * `context_truncation` - Optional context truncation metadata
    pub async fn say(
        &mut self,
        say_type: ClineSay,
        text: Option<String>,
        images: Option<Vec<String>>,
        partial: Option<bool>,
        checkpoint: Option<serde_json::Value>,
        progress_status: Option<ToolProgressStatus>,
        options: SayOptions,
        context_condense: Option<ContextCondense>,
        context_truncation: Option<ContextTruncation>,
    ) -> Result<(), TaskError> {
        // Source: TS line 1768–1770 — abort check
        // (abort check is handled at TaskLifecycle level)

        match partial {
            // ── partial=true: new or updated partial message ───────────
            // Source: TS lines 1778–1804
            Some(true) => {
                let is_updating_previous = self.is_updating_previous_say_partial(&say_type);

                if is_updating_previous {
                    // Source: TS lines 1779–1785 — update existing partial
                    if let Some(last) = self.cline_messages.last_mut() {
                        last.text = text.clone();
                        last.images = images.clone();
                        last.partial = Some(true);
                        last.progress_status = progress_status.clone();
                        // Clone before releasing the mutable borrow
                        let msg = last.clone();
                        let _ = last;
                        self.emit_message_updated(msg);
                    }
                } else {
                    // Source: TS lines 1786–1804 — new partial message
                    let say_ts = now_ts();
                    if !options.is_non_interactive {
                        self.last_message_ts = Some(say_ts);
                    }
                    let message = ClineMessage {
                        ts: say_ts,
                        r#type: MessageType::Say,
                        ask: None,
                        say: Some(say_type),
                        text,
                        images,
                        partial: Some(true),
                        reasoning: None,
                        conversation_history_index: None,
                        checkpoint: None,
                        progress_status,
                        context_condense,
                        context_truncation,
                        is_protected: None,
                        api_protocol: None,
                        is_answered: None,
                    };
                    self.add_to_cline_messages(message);
                }
            }

            // ── partial=false: finalize partial or new complete ─────────
            // Source: TS lines 1805–1843
            Some(false) => {
                let is_updating_previous = self.is_updating_previous_say_partial(&say_type);

                if is_updating_previous {
                    // Source: TS lines 1809–1824 — finalize existing partial
                    if let Some(last) = self.cline_messages.last_mut() {
                        if !options.is_non_interactive {
                            self.last_message_ts = Some(last.ts);
                        }
                        last.text = text;
                        last.images = images;
                        last.partial = Some(false);
                        last.progress_status = progress_status;
                        // Clone before releasing the mutable borrow
                        let msg = last.clone();
                        let _ = last;
                        self.emit_message_updated(msg);
                    }
                } else {
                    // Source: TS lines 1825–1842 — new complete message
                    let say_ts = now_ts();
                    if !options.is_non_interactive {
                        self.last_message_ts = Some(say_ts);
                    }
                    let message = ClineMessage {
                        ts: say_ts,
                        r#type: MessageType::Say,
                        ask: None,
                        say: Some(say_type),
                        text,
                        images,
                        partial: None,
                        reasoning: None,
                        conversation_history_index: None,
                        checkpoint: None,
                        progress_status,
                        context_condense,
                        context_truncation,
                        is_protected: None,
                        api_protocol: None,
                        is_answered: None,
                    };
                    self.add_to_cline_messages(message);
                }
            }

            // ── partial=None: new non-partial message ──────────────────
            // Source: TS lines 1844–1866
            None => {
                let say_ts = now_ts();
                // Source: TS lines 1848–1854 — non-interactive messages
                // don't update lastMessageTs
                if !options.is_non_interactive {
                    self.last_message_ts = Some(say_ts);
                }
                let message = ClineMessage {
                    ts: say_ts,
                    r#type: MessageType::Say,
                    ask: None,
                    say: Some(say_type),
                    text,
                    images,
                    partial: None,
                    reasoning: None,
                    conversation_history_index: None,
                    checkpoint,
                    progress_status,
                    context_condense,
                    context_truncation,
                    is_protected: None,
                    api_protocol: None,
                    is_answered: None,
                };
                self.add_to_cline_messages(message);
            }
        }

        Ok(())
    }

    /// Convenience method: emit a simple say message (no partial, no extras).
    ///
    /// This is the backward-compatible interface used by existing callers.
    pub async fn say_simple(
        &mut self,
        say_type: ClineSay,
        text: Option<String>,
        images: Option<Vec<String>>,
    ) -> Result<(), TaskError> {
        self.say(
            say_type,
            text,
            images,
            None,  // partial
            None,  // checkpoint
            None,  // progress_status
            SayOptions::default(),
            None,  // context_condense
            None,  // context_truncation
        )
        .await
    }

    /// Emit a partial (streaming) say message.
    ///
    /// Source: `src/core/task/Task.ts` — `say()` with `partial` parameter
    pub async fn say_partial(
        &mut self,
        say_type: ClineSay,
        text: Option<String>,
        partial: bool,
    ) -> Result<(), TaskError> {
        self.say(
            say_type,
            text,
            None,
            Some(partial),
            None,
            None,
            SayOptions::default(),
            None,
            None,
        )
        .await
    }

    // ===================================================================
    // ask()
    // Source: `src/core/task/Task.ts` — `ask()` (lines 1264–1499)
    // ===================================================================

    /// Ask the user a question and wait for a response.
    ///
    /// Source: `src/core/task/Task.ts` — `ask()` (lines 1264–1499)
    ///
    /// Creates a ClineMessage with type "ask" and waits for the user to respond.
    /// Supports partial updates (streaming), auto-approval, state mutation
    /// (idle/resumable/interactive), and message queue draining.
    ///
    /// # Arguments
    /// * `ask_type` - The type of question being asked
    /// * `text` - Optional text for the question
    /// * `partial` - Whether this is a partial (streaming) update:
    ///   - `None` → new complete message
    ///   - `Some(true)` → new or updated partial message → returns AskIgnoredError
    ///   - `Some(false)` → finalize existing partial or create new complete
    /// * `progress_status` - Optional progress status for tool operations
    /// * `is_protected` - Whether this ask is protected from auto-approval
    ///
    /// # Returns
    /// * `Ok(AskResult)` - The user's response
    /// * `Err(AskIgnoredError)` - If this was a partial update or was superseded
    pub async fn ask(
        &mut self,
        ask_type: ClineAsk,
        text: Option<String>,
        partial: Option<bool>,
        progress_status: Option<ToolProgressStatus>,
        is_protected: Option<bool>,
    ) -> Result<AskResult, AskIgnoredError> {
        // Source: TS lines 1285–1359 — handle partial messages
        let ask_ts = self.handle_ask_partial(ask_type, text.clone(), partial, progress_status.clone(), is_protected)?;

        // ── Auto-approval check ─────────────────────────────────────────
        // Source: TS lines 1361–1380
        self.handle_auto_approval(ask_type, text.as_deref(), is_protected);

        // ── State mutation (idle/resumable/interactive) ─────────────────
        // Source: TS lines 1382–1444
        // NOTE: State mutation with timeouts is handled at the TaskLifecycle
        // level because it requires access to the message queue service.
        // Here we just track the ask type for state queries.
        self.track_ask_state(ask_type, ask_ts, partial);

        // ── Wait for askResponse to be set ──────────────────────────────
        // Source: TS lines 1447–1472 — pWaitFor loop
        let result = self.wait_for_ask_response(ask_ts).await?;

        // ── Check if superseded ─────────────────────────────────────────
        // Source: TS lines 1474–1479
        if self.last_message_ts != Some(ask_ts) {
            return Err(AskIgnoredError {
                reason: "superseded".to_string(),
            });
        }

        // ── Extract result and clean up ─────────────────────────────────
        // Source: TS lines 1481–1498
        self.clear_ask_state();

        Ok(result)
    }

    /// Handle partial message logic for ask().
    ///
    /// Source: `src/core/task/Task.ts` — `ask()` lines 1285–1359
    ///
    /// Returns the ask timestamp for non-partial messages.
    /// Returns AskIgnoredError for partial messages.
    fn handle_ask_partial(
        &mut self,
        ask_type: ClineAsk,
        text: Option<String>,
        partial: Option<bool>,
        progress_status: Option<ToolProgressStatus>,
        is_protected: Option<bool>,
    ) -> Result<f64, AskIgnoredError> {
        match partial {
            // ── partial=true: update or create partial ──────────────────
            // Source: TS lines 1291–1313
            Some(true) => {
                let is_updating_previous = self.is_updating_previous_ask_partial(&ask_type);

                if is_updating_previous {
                    // Source: TS lines 1292–1304 — update existing partial
                    if let Some(last) = self.cline_messages.last_mut() {
                        last.text = text;
                        last.partial = Some(true);
                        last.progress_status = progress_status;
                        last.is_protected = is_protected;
                        let msg = last.clone();
                        let _ = last;
                        self.emit_message_updated(msg);
                    }
                    return Err(AskIgnoredError {
                        reason: "updating existing partial".to_string(),
                    });
                } else {
                    // Source: TS lines 1305–1313 — new partial message
                    let ask_ts = now_ts();
                    self.last_message_ts = Some(ask_ts);
                    let message = ClineMessage {
                        ts: ask_ts,
                        r#type: MessageType::Ask,
                        ask: Some(ask_type),
                        say: None,
                        text,
                        images: None,
                        partial: Some(true),
                        reasoning: None,
                        conversation_history_index: None,
                        checkpoint: None,
                        progress_status,
                        context_condense: None,
                        context_truncation: None,
                        is_protected,
                        api_protocol: None,
                        is_answered: None,
                    };
                    self.add_to_cline_messages(message);
                    return Err(AskIgnoredError {
                        reason: "new partial".to_string(),
                    });
                }
            }

            // ── partial=false: finalize partial or new complete ─────────
            // Source: TS lines 1314–1350
            Some(false) => {
                let is_updating_previous = self.is_updating_previous_ask_partial(&ask_type);

                if is_updating_previous {
                    // Source: TS lines 1315–1340 — finalize existing partial
                    // NOTE: We keep the same ts to avoid UI flickering
                    // (see TS comment about "Bug for the history books")
                    self.clear_ask_response_fields();
                    let ask_ts = if let Some(last) = self.cline_messages.last() {
                        last.ts
                    } else {
                        now_ts()
                    };
                    self.last_message_ts = Some(ask_ts);
                    if let Some(last) = self.cline_messages.last_mut() {
                        last.text = text;
                        last.partial = Some(false);
                        last.progress_status = progress_status;
                        last.is_protected = is_protected;
                        let msg = last.clone();
                        let _ = last;
                        self.emit_message_updated(msg);
                    }
                    Ok(ask_ts)
                } else {
                    // Source: TS lines 1341–1349 — new complete message
                    self.clear_ask_response_fields();
                    let ask_ts = now_ts();
                    self.last_message_ts = Some(ask_ts);
                    let message = ClineMessage {
                        ts: ask_ts,
                        r#type: MessageType::Ask,
                        ask: Some(ask_type),
                        say: None,
                        text,
                        images: None,
                        partial: None,
                        reasoning: None,
                        conversation_history_index: None,
                        checkpoint: None,
                        progress_status,
                        context_condense: None,
                        context_truncation: None,
                        is_protected,
                        api_protocol: None,
                        is_answered: None,
                    };
                    self.add_to_cline_messages(message);
                    Ok(ask_ts)
                }
            }

            // ── partial=None: new non-partial message ───────────────────
            // Source: TS lines 1351–1359
            None => {
                self.clear_ask_response_fields();
                let ask_ts = now_ts();
                self.last_message_ts = Some(ask_ts);
                let message = ClineMessage {
                    ts: ask_ts,
                    r#type: MessageType::Ask,
                    ask: Some(ask_type),
                    say: None,
                    text,
                    images: None,
                    partial: None,
                    reasoning: None,
                    conversation_history_index: None,
                    checkpoint: None,
                    progress_status,
                    context_condense: None,
                    context_truncation: None,
                    is_protected,
                    api_protocol: None,
                    is_answered: None,
                };
                self.add_to_cline_messages(message);
                Ok(ask_ts)
            }
        }
    }

    /// Handle auto-approval logic.
    ///
    /// Source: `src/core/task/Task.ts` — `ask()` lines 1361–1380
    ///
    /// In the TS version, this calls `checkAutoApproval()` which inspects
    /// the user's auto-approval settings and returns a decision:
    /// - `"approve"` → immediately approve the ask
    /// - `"deny"` → immediately deny the ask
    /// - `"timeout"` → schedule auto-approval after a delay
    /// - `"ask"` → present to the user (default)
    ///
    /// In Rust, the auto-approval state is provided externally via
    /// [`set_auto_approval_decision`]. If a decision has been set,
    /// it is applied here.
    fn handle_auto_approval(
        &mut self,
        _ask_type: ClineAsk,
        _text: Option<&str>,
        _is_protected: Option<bool>,
    ) {
        // Check if there's a pending auto-approval decision
        if let Some(decision) = self.auto_approval_pending.take() {
            match decision {
                AutoApprovalDecision::Approve => {
                    debug!("Auto-approving ask");
                    // Note: approve_ask is async, but we can't call it here.
                    // Instead, we set the response directly.
                    // The caller (TaskLifecycle) should handle this.
                }
                AutoApprovalDecision::Deny => {
                    debug!("Auto-denying ask");
                }
                AutoApprovalDecision::Timeout { timeout_ms, .. } => {
                    debug!(timeout_ms = timeout_ms, "Scheduling auto-approval timeout");
                    self.auto_approval_timeout_active = true;
                    // The actual timeout is handled by the TaskLifecycle
                    // which has access to tokio::time::sleep
                }
                AutoApprovalDecision::Ask => {
                    // No auto-action needed
                }
            }
        }
    }

    /// Set an auto-approval decision to be applied on the next ask.
    ///
    /// Source: `src/core/task/Task.ts` — `checkAutoApproval()` (line 1366)
    ///
    /// This is called by the TaskLifecycle before invoking `ask()`.
    pub fn set_auto_approval_decision(&mut self, decision: AutoApprovalDecision) {
        self.auto_approval_pending = Some(decision);
    }

    /// Track the ask state (idle/resumable/interactive).
    ///
    /// Source: `src/core/task/Task.ts` — `ask()` lines 1382–1428
    ///
    /// In the TS version, this sets up timeouts that transition the task
    /// to idle/resumable/interactive states after `statusMutationTimeout`
    /// (2 seconds). The state is only mutated when:
    /// - `isStatusMutable` is true (complete, blocking, no queued messages)
    /// - The ask type matches the corresponding category
    ///
    /// In Rust, we set the state immediately (without the 2-second delay)
    /// because the timeout-based approach requires spawning a separate task.
    /// The TaskLifecycle can override this by calling the state mutation
    /// methods directly with a delay.
    fn track_ask_state(&mut self, ask_type: ClineAsk, ask_ts: f64, partial: Option<bool>) {
        // Only track state for complete, blocking messages
        if partial.is_some() {
            return;
        }

        // Source: TS lines 1391–1428 — state mutation based on ask type
        // TS uses a 2-second timeout (`statusMutationTimeout = 2_000`)
        // before transitioning state. We set it immediately for simplicity.
        //
        // Source: TS lines 1394–1405 — interactive ask
        if ask_type.is_interactive() {
            if let Some(msg) = self.find_message_by_timestamp(ask_ts) {
                self.interactive_ask = Some(msg.clone());
                self.emit_task_interactive();
                // Source: TS line 1402 — also emit interactionRequired to webview
                self.emit_interaction_required();
            }
        // Source: TS lines 1406–1416 — resumable ask
        } else if ask_type.is_resumable() {
            if let Some(msg) = self.find_message_by_timestamp(ask_ts) {
                self.resumable_ask = Some(msg.clone());
                self.emit_task_resumable();
            }
        // Source: TS lines 1417–1428 — idle ask
        } else if ask_type.is_idle() {
            if let Some(msg) = self.find_message_by_timestamp(ask_ts) {
                self.idle_ask = Some(msg.clone());
                self.emit_task_idle();
            }
        }
    }

    /// Clear ask state and emit TaskActive event.
    ///
    /// Source: `src/core/task/Task.ts` — `ask()` lines 1490–1498
    ///
    /// TS implementation:
    /// ```ts
    /// if (this.idleAsk || this.resumableAsk || this.interactiveAsk) {
    ///     this.idleAsk = undefined
    ///     this.resumableAsk = undefined
    ///     this.interactiveAsk = undefined
    ///     this.emit(RooCodeEventName.TaskActive, this.taskId)
    /// }
    /// this.emit(RooCodeEventName.TaskAskResponded)
    /// ```
    fn clear_ask_state(&mut self) {
        let had_state = self.idle_ask.is_some()
            || self.resumable_ask.is_some()
            || self.interactive_ask.is_some();

        self.idle_ask = None;
        self.resumable_ask = None;
        self.interactive_ask = None;

        // Source: TS line 1494 — emit TaskActive when clearing state
        if had_state {
            self.emit_task_active();
        }

        // Source: TS line 1497 — emit TaskAskResponded
        self.emit_task_ask_responded();
    }

    /// Wait for the ask response to be set.
    ///
    /// Source: `src/core/task/Task.ts` — `ask()` lines 1447–1472 (`pWaitFor`)
    async fn wait_for_ask_response(&self, ask_ts: f64) -> Result<AskResult, AskIgnoredError> {
        let mut rx = self.ask_signal_rx.clone();
        let response = self.ask_response.clone();

        loop {
            // Check if response is already available
            {
                let guard = response.lock().await;
                if let Some(result) = guard.as_ref() {
                    return Ok(result.clone());
                }
            }

            // Check if the message was superseded
            // Source: TS line 1474 — `this.lastMessageTs !== askTs`
            if let Some(ts) = self.last_message_ts {
                if (ts - ask_ts).abs() > 0.001 {
                    // Message was superseded — check if there's a response
                    let guard = response.lock().await;
                    if let Some(result) = guard.as_ref() {
                        return Ok(result.clone());
                    }
                    // No response but superseded
                    warn!("Ask was superseded without a response");
                }
            }

            // Wait for signal
            // Source: TS line 1471 — `{ interval: 100 }`
            if rx.changed().await.is_err() {
                // Channel closed — task was likely aborted
                return Err(AskIgnoredError {
                    reason: "ask channel closed (task likely aborted)".to_string(),
                });
            }
        }
    }

    // ===================================================================
    // handle_response() / handleWebviewAskResponse()
    // Source: `src/core/task/Task.ts` — lines 1501–1548
    // ===================================================================

    /// Handle a response from the webview/user to a pending ask.
    ///
    /// Source: `src/core/task/Task.ts` — `handleWebviewAskResponse()` (lines 1501–1548)
    ///
    /// This method:
    /// 1. Cancels any pending auto-approval timeout
    /// 2. Sets the ask response
    /// 3. On `messageResponse`: triggers checkpoint save
    /// 4. Marks the last follow-up question as answered
    /// 5. Marks the last tool-approval ask as answered (on yesButtonClicked)
    pub async fn handle_response(
        &self,
        ask_response: AskResponse,
        text: Option<String>,
        images: Option<Vec<String>>,
    ) {
        // Source: TS lines 1502–1503 — cancel auto-approval timeout
        // (In our implementation, this is tracked at the TaskLifecycle level)

        // Source: TS lines 1505–1507 — set the response
        let result = AskResult {
            response: ask_response.clone(),
            text: text.clone(),
            images: images.clone(),
        };

        {
            let mut guard = self.ask_response.lock().await;
            *guard = Some(result);
        }

        // Signal that a response is available
        let _ = self.ask_signal.send(true);

        // NOTE: The following operations require mutable access to cline_messages,
        // which we can't do through &self. These are handled by the caller
        // (TaskLifecycle) which has mutable access.
        //
        // In the TS version (lines 1509–1547):
        // - checkpointSave on messageResponse
        // - Mark followup as answered on messageResponse/yesButtonClicked
        // - Mark tool-approval ask as answered on yesButtonClicked
    }

    /// Handle response with message mutation (marks followups/tool-asks as answered).
    ///
    /// Source: `src/core/task/Task.ts` — `handleWebviewAskResponse()` (lines 1509–1547)
    ///
    /// This is the full version that also handles:
    /// - Marking followup questions as answered
    /// - Marking tool-approval asks as answered
    pub async fn handle_response_full(
        &mut self,
        ask_response: AskResponse,
        text: Option<String>,
        images: Option<Vec<String>>,
    ) -> bool {
        // Cancel auto-approval timeout
        self.cancel_auto_approval_timeout();

        // Set the response
        let result = AskResult {
            response: ask_response.clone(),
            text: text.clone(),
            images: images.clone(),
        };

        {
            let mut guard = self.ask_response.lock().await;
            *guard = Some(result);
        }

        // Signal that a response is available
        let _ = self.ask_signal.send(true);

        let mut checkpoint_needed = false;

        // Source: TS lines 1512–1514 — checkpoint on messageResponse
        if ask_response == AskResponse::MessageResponse {
            checkpoint_needed = true;
        }

        // Source: TS lines 1517–1532 — mark followup as answered
        if ask_response == AskResponse::MessageResponse
            || ask_response == AskResponse::YesButtonClicked
        {
            self.mark_last_followup_answered();
        }

        // Source: TS lines 1534–1547 — mark tool-approval ask as answered
        if ask_response == AskResponse::YesButtonClicked {
            self.mark_last_tool_ask_answered();
        }

        checkpoint_needed
    }

    // ===================================================================
    // approveAsk() / denyAsk()
    // Source: `src/core/task/Task.ts` — lines 1561–1567
    // ===================================================================

    /// Approve the current ask (auto-approve).
    ///
    /// Source: `src/core/task/Task.ts` — `approveAsk()` (lines 1561–1563)
    pub async fn approve_ask(&self, text: Option<String>, images: Option<Vec<String>>) {
        self.handle_response(AskResponse::YesButtonClicked, text, images)
            .await;
    }

    /// Deny the current ask (auto-deny).
    ///
    /// Source: `src/core/task/Task.ts` — `denyAsk()` (lines 1565–1567)
    pub async fn deny_ask(&self, text: Option<String>, images: Option<Vec<String>>) {
        self.handle_response(AskResponse::NoButtonClicked, text, images)
            .await;
    }

    // ===================================================================
    // supersedePendingAsk()
    // Source: `src/core/task/Task.ts` — lines 1569–1571
    // ===================================================================

    /// Supersede the pending ask by updating the timestamp.
    ///
    /// Source: `src/core/task/Task.ts` — `supersedePendingAsk()` (lines 1569–1571)
    ///
    /// This causes any pending `ask()` call to detect that its message
    /// was superseded and throw an `AskIgnoredError`.
    pub fn supersede_pending_ask(&mut self) {
        self.last_message_ts = Some(now_ts());
    }

    // ===================================================================
    // cancelAutoApprovalTimeout()
    // Source: `src/core/task/Task.ts` — lines 1554–1559
    // ===================================================================

    /// Cancel any pending auto-approval timeout.
    ///
    /// Source: `src/core/task/Task.ts` — `cancelAutoApprovalTimeout()` (lines 1554–1559)
    ///
    /// Called when the user interacts (types, clicks buttons, etc.) to
    /// prevent the timeout from firing.
    pub fn cancel_auto_approval_timeout(&mut self) {
        if self.auto_approval_timeout_active {
            self.auto_approval_timeout_active = false;
            debug!("Auto-approval timeout cancelled");
        }
    }

    // ===================================================================
    // sayAndCreateMissingParamError()
    // Source: `src/core/task/Task.ts` — lines 1869–1877
    // ===================================================================

    /// Say an error about a missing parameter and return a tool error string.
    ///
    /// Source: `src/core/task/Task.ts` — `sayAndCreateMissingParamError()` (lines 1869–1877)
    ///
    /// TS implementation:
    /// ```ts
    /// await this.say("error",
    ///     `Roo tried to use ${toolName}${relPath ? ` for '${relPath.toPosix()}'` : ""}
    ///      without value for required parameter '${paramName}'. Retrying...`)
    /// return formatResponse.toolError(formatResponse.missingToolParameterError(paramName))
    /// ```
    pub async fn say_and_create_missing_param_error(
        &mut self,
        tool_name: &str,
        param_name: &str,
        rel_path: Option<&str>,
    ) -> Result<String, TaskError> {
        // Source: TS lines 1870–1875 — format the error message
        let path_info = rel_path
            .map(|p| {
                // Source: TS `relPath.toPosix()` — normalize path separators
                let posix_path = p.replace('\\', "/");
                format!(" for '{}'", posix_path)
            })
            .unwrap_or_default();
        let text = format!(
            "Roo tried to use {}{} without value for required parameter '{}'. Retrying...",
            tool_name, path_info, param_name
        );
        self.say_simple(ClineSay::Error, Some(text), None)
            .await?;

        // Source: TS line 1876 — `formatResponse.toolError(formatResponse.missingToolParameterError(paramName))`
        // Returns a tool error string that matches the TS formatResponse.missingToolParameterError
        Ok(format!(
            "Error: Missing required parameter '{}' for tool '{}'.",
            param_name, tool_name
        ))
    }

    // ===================================================================
    // Getters
    // ===================================================================

    /// Get all cline messages.
    pub fn cline_messages(&self) -> &[ClineMessage] {
        &self.cline_messages
    }

    /// Get a mutable reference to cline messages.
    pub fn cline_messages_mut(&mut self) -> &mut Vec<ClineMessage> {
        &mut self.cline_messages
    }

    /// Get the last message timestamp.
    pub fn last_message_ts(&self) -> Option<f64> {
        self.last_message_ts
    }

    /// Get the idle ask message, if any.
    pub fn idle_ask(&self) -> Option<&ClineMessage> {
        self.idle_ask.as_ref()
    }

    /// Get the resumable ask message, if any.
    pub fn resumable_ask(&self) -> Option<&ClineMessage> {
        self.resumable_ask.as_ref()
    }

    /// Get the interactive ask message, if any.
    pub fn interactive_ask(&self) -> Option<&ClineMessage> {
        self.interactive_ask.as_ref()
    }

    /// Check if the handler is in an idle state.
    pub fn is_idle(&self) -> bool {
        self.idle_ask.is_some()
    }

    /// Check if the handler is in a resumable state.
    pub fn is_resumable(&self) -> bool {
        self.resumable_ask.is_some()
    }

    /// Check if the handler is in an interactive state.
    pub fn is_interactive(&self) -> bool {
        self.interactive_ask.is_some()
    }

    /// Find a message by timestamp.
    ///
    /// Source: `src/core/task/Task.ts` — `findMessageByTimestamp()` (lines 1251–1259)
    pub fn find_message_by_timestamp(&self, ts: f64) -> Option<&ClineMessage> {
        for i in (0..self.cline_messages.len()).rev() {
            if (self.cline_messages[i].ts - ts).abs() < 0.001 {
                return Some(&self.cline_messages[i]);
            }
        }
        None
    }

    /// Find a mutable message by timestamp.
    pub fn find_message_by_timestamp_mut(&mut self, ts: f64) -> Option<&mut ClineMessage> {
        for i in (0..self.cline_messages.len()).rev() {
            if (self.cline_messages[i].ts - ts).abs() < 0.001 {
                return Some(&mut self.cline_messages[i]);
            }
        }
        None
    }

    /// Overwrite all cline messages.
    ///
    /// Source: `src/core/task/Task.ts` — `overwriteClineMessages()` (lines 1177–1190)
    pub fn overwrite_cline_messages(&mut self, messages: Vec<ClineMessage>) {
        self.cline_messages = messages;
    }

    /// Update a specific cline message.
    ///
    /// Source: `src/core/task/Task.ts` — `updateClineMessage()` (lines 1192–1209)
    pub fn update_cline_message(&mut self, message: &ClineMessage) {
        if let Some(existing) = self
            .cline_messages
            .iter_mut()
            .rev()
            .find(|m| (m.ts - message.ts).abs() < 0.001)
        {
            *existing = message.clone();
        }
    }

    // ===================================================================
    // Private helpers
    // ===================================================================

    /// Check if the last message is a partial say of the given type.
    fn is_updating_previous_say_partial(&self, say_type: &ClineSay) -> bool {
        self.cline_messages
            .last()
            .map(|last| {
                last.partial == Some(true)
                    && last.r#type == MessageType::Say
                    && last.say == Some(*say_type)
            })
            .unwrap_or(false)
    }

    /// Check if the last message is a partial ask of the given type.
    fn is_updating_previous_ask_partial(&self, ask_type: &ClineAsk) -> bool {
        self.cline_messages
            .last()
            .map(|last| {
                last.partial == Some(true)
                    && last.r#type == MessageType::Ask
                    && last.ask == Some(*ask_type)
            })
            .unwrap_or(false)
    }

    /// Add a message to cline_messages and emit a MessageCreated event.
    ///
    /// Source: `src/core/task/Task.ts` — `addToClineMessages()` (lines 1156–1174)
    fn add_to_cline_messages(&mut self, message: ClineMessage) {
        let say_type = message.say;
        self.cline_messages.push(message);
        debug!(say_type = ?say_type, "Message added to clineMessages");
    }

    /// Emit a message updated event.
    fn emit_message_updated(&self, message: ClineMessage) {
        if let Some(ref emitter) = self.event_emitter {
            emitter.emit_message_updated(message);
        }
    }

    /// Emit a task interactive event.
    fn emit_task_interactive(&self) {
        if let Some(ref emitter) = self.event_emitter {
            if let Some(ref task_id) = self.task_id {
                emitter.emit_task_interactive(task_id);
            }
        }
    }

    /// Emit a task idle event.
    fn emit_task_idle(&self) {
        if let Some(ref emitter) = self.event_emitter {
            if let Some(ref task_id) = self.task_id {
                emitter.emit_task_idle(task_id);
            }
        }
    }

    /// Emit a task resumable event.
    fn emit_task_resumable(&self) {
        if let Some(ref emitter) = self.event_emitter {
            if let Some(ref task_id) = self.task_id {
                emitter.emit_task_resumable(task_id);
            }
        }
    }

    /// Emit a task active event.
    ///
    /// Source: TS line 1494 — `this.emit(RooCodeEventName.TaskActive, this.taskId)`
    fn emit_task_active(&self) {
        if let Some(ref emitter) = self.event_emitter {
            if let Some(ref task_id) = self.task_id {
                emitter.emit_task_active(task_id);
            }
        }
    }

    /// Emit an interaction required event.
    ///
    /// Source: TS line 1402 — `provider?.postMessageToWebview({ type: "interactionRequired" })`
    fn emit_interaction_required(&self) {
        if let Some(ref emitter) = self.event_emitter {
            if let Some(ref task_id) = self.task_id {
                emitter.emit_interaction_required(task_id);
            }
        }
    }

    /// Emit a task ask responded event.
    ///
    /// Source: TS line 1497 — `this.emit(RooCodeEventName.TaskAskResponded)`
    fn emit_task_ask_responded(&self) {
        if let Some(ref emitter) = self.event_emitter {
            if let Some(ref task_id) = self.task_id {
                emitter.emit_task_ask_responded(task_id);
            }
        }
    }

    /// Clear the ask response fields.
    ///
    /// Source: TS lines 1318–1320, 1343–1345, 1353–1355
    fn clear_ask_response_fields(&self) {
        // We can't clear the Mutex directly from a non-async context,
        // so we use a blocking lock here (acceptable since it's very brief).
        if let Ok(mut guard) = self.ask_response.try_lock() {
            *guard = None;
        }
    }

    /// Mark the last unanswered followup message as answered.
    ///
    /// Source: TS lines 1517–1532
    fn mark_last_followup_answered(&mut self) {
        let idx = self
            .cline_messages
            .iter()
            .rposition(|m| m.r#type == MessageType::Ask && m.ask == Some(ClineAsk::Followup) && m.is_answered != Some(true));

        if let Some(i) = idx {
            self.cline_messages[i].is_answered = Some(true);
            debug!("Marked followup at index {} as answered", i);
        }
    }

    /// Mark the last unanswered tool-approval ask as answered.
    ///
    /// Source: TS lines 1534–1547
    fn mark_last_tool_ask_answered(&mut self) {
        let idx = self
            .cline_messages
            .iter()
            .rposition(|m| m.r#type == MessageType::Ask && m.ask == Some(ClineAsk::Tool) && m.is_answered != Some(true));

        if let Some(i) = idx {
            self.cline_messages[i].is_answered = Some(true);
            self.emit_message_updated(self.cline_messages[i].clone());
            debug!("Marked tool-approval ask at index {} as answered", i);
        }
    }
}

impl Default for AskSayHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get the current timestamp in milliseconds.
fn now_ts() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ask_say_handler_new() {
        let handler = AskSayHandler::new();
        assert!(handler.cline_messages().is_empty());
        assert!(handler.last_message_ts().is_none());
        assert!(!handler.is_idle());
        assert!(!handler.is_resumable());
        assert!(!handler.is_interactive());
    }

    #[tokio::test]
    async fn test_say_creates_message() {
        let mut handler = AskSayHandler::new();
        handler
            .say_simple(ClineSay::Text, Some("Hello".to_string()), None)
            .await
            .unwrap();

        assert_eq!(handler.cline_messages().len(), 1);
        let msg = &handler.cline_messages()[0];
        assert_eq!(msg.r#type, MessageType::Say);
        assert_eq!(msg.say, Some(ClineSay::Text));
        assert_eq!(msg.text, Some("Hello".to_string()));
    }

    #[tokio::test]
    async fn test_say_with_context_condense() {
        let mut handler = AskSayHandler::new();
        let condense = ContextCondense {
            cost: 0.5,
            prev_context_tokens: 10000,
            new_context_tokens: 5000,
            summary: "Test summary".to_string(),
            condense_id: Some("condense-123".to_string()),
        };
        handler
            .say(
                ClineSay::CondenseContext,
                None,
                None,
                None,
                None,
                None,
                SayOptions { is_non_interactive: true },
                Some(condense),
                None,
            )
            .await
            .unwrap();

        assert_eq!(handler.cline_messages().len(), 1);
        let msg = &handler.cline_messages()[0];
        assert_eq!(msg.say, Some(ClineSay::CondenseContext));
        assert!(msg.context_condense.is_some());
        assert!(handler.last_message_ts().is_none()); // is_non_interactive didn't update ts
    }

    #[tokio::test]
    async fn test_say_partial_update() {
        let mut handler = AskSayHandler::new();

        // First partial
        handler
            .say_partial(ClineSay::Text, Some("Hello".to_string()), true)
            .await
            .unwrap();
        assert_eq!(handler.cline_messages().len(), 1);

        // Update partial
        handler
            .say_partial(ClineSay::Text, Some("Hello world".to_string()), true)
            .await
            .unwrap();
        assert_eq!(handler.cline_messages().len(), 1); // Updated, not added
        assert_eq!(
            handler.cline_messages()[0].text,
            Some("Hello world".to_string())
        );

        // Finalize
        handler
            .say_partial(ClineSay::Text, Some("Hello world!".to_string()), false)
            .await
            .unwrap();
        assert_eq!(handler.cline_messages().len(), 1);
        assert_eq!(handler.cline_messages()[0].partial, Some(false));
    }

    #[tokio::test]
    async fn test_say_non_interactive_does_not_update_ts() {
        let mut handler = AskSayHandler::new();

        // Set an initial ts
        handler.last_message_ts = Some(1000.0);

        handler
            .say(
                ClineSay::CondenseContext,
                Some("condensing".to_string()),
                None,
                None,
                None,
                None,
                SayOptions { is_non_interactive: true },
                None,
                None,
            )
            .await
            .unwrap();

        // ts should NOT have been updated
        assert_eq!(handler.last_message_ts(), Some(1000.0));
    }

    #[tokio::test]
    async fn test_ask_partial_returns_error() {
        let mut handler = AskSayHandler::new();
        let result = handler
            .ask(ClineAsk::Followup, Some("Question?".to_string()), Some(true), None, None)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.reason.contains("partial"));
    }

    #[tokio::test]
    async fn test_ask_and_respond() {
        let mut handler = AskSayHandler::new();

        // Spawn a task that will respond after a short delay
        let response = handler.ask_response.clone();
        let signal = handler.ask_signal.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let mut guard = response.lock().await;
            *guard = Some(AskResult {
                response: AskResponse::YesButtonClicked,
                text: Some("Yes".to_string()),
                images: None,
            });
            let _ = signal.send(true);
        });

        let result = handler
            .ask(ClineAsk::Tool, Some("Approve?".to_string()), None, None, None)
            .await;
        assert!(result.is_ok());
        let ask_result = result.unwrap();
        assert_eq!(ask_result.response, AskResponse::YesButtonClicked);
        assert_eq!(ask_result.text, Some("Yes".to_string()));
    }

    #[tokio::test]
    async fn test_handle_response() {
        let handler = AskSayHandler::new();
        handler
            .handle_response(
                AskResponse::MessageResponse,
                Some("My response".to_string()),
                None,
            )
            .await;

        let guard = handler.ask_response.lock().await;
        assert!(guard.is_some());
        let result = guard.as_ref().unwrap();
        assert_eq!(result.response, AskResponse::MessageResponse);
        assert_eq!(result.text, Some("My response".to_string()));
    }

    #[tokio::test]
    async fn test_handle_response_full_marks_followup() {
        let mut handler = AskSayHandler::new();

        // Add a followup ask message
        let ts = now_ts();
        handler.cline_messages.push(ClineMessage {
            ts,
            r#type: MessageType::Ask,
            ask: Some(ClineAsk::Followup),
            say: None,
            text: Some("What do you think?".to_string()),
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

        // Handle response with messageResponse
        let checkpoint = handler
            .handle_response_full(
                AskResponse::MessageResponse,
                Some("My answer".to_string()),
                None,
            )
            .await;

        assert!(checkpoint); // checkpoint needed for messageResponse
        assert_eq!(handler.cline_messages()[0].is_answered, Some(true));
    }

    #[tokio::test]
    async fn test_handle_response_full_marks_tool_ask() {
        let mut handler = AskSayHandler::new();

        // Add a tool ask message
        let ts = now_ts();
        handler.cline_messages.push(ClineMessage {
            ts,
            r#type: MessageType::Ask,
            ask: Some(ClineAsk::Tool),
            say: None,
            text: Some("Approve write?".to_string()),
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

        // Handle response with yesButtonClicked
        let checkpoint = handler
            .handle_response_full(
                AskResponse::YesButtonClicked,
                None,
                None,
            )
            .await;

        assert!(!checkpoint); // no checkpoint for yesButtonClicked
        assert_eq!(handler.cline_messages()[0].is_answered, Some(true));
    }

    #[tokio::test]
    async fn test_approve_deny_ask() {
        let handler = AskSayHandler::new();

        handler
            .approve_ask(Some("ok".to_string()), None)
            .await;
        {
            let guard = handler.ask_response.lock().await;
            assert_eq!(
                guard.as_ref().unwrap().response,
                AskResponse::YesButtonClicked
            );
        }

        // Reset and deny
        {
            let mut guard = handler.ask_response.lock().await;
            *guard = None;
        }

        handler.deny_ask(None, None).await;
        {
            let guard = handler.ask_response.lock().await;
            assert_eq!(
                guard.as_ref().unwrap().response,
                AskResponse::NoButtonClicked
            );
        }
    }

    #[test]
    fn test_supersede_pending_ask() {
        let mut handler = AskSayHandler::new();
        assert!(handler.last_message_ts().is_none());
        handler.supersede_pending_ask();
        assert!(handler.last_message_ts().is_some());
    }

    #[test]
    fn test_cancel_auto_approval_timeout() {
        let mut handler = AskSayHandler::new();
        assert!(!handler.auto_approval_timeout_active);

        handler.auto_approval_timeout_active = true;
        handler.cancel_auto_approval_timeout();
        assert!(!handler.auto_approval_timeout_active);
    }

    #[test]
    fn test_find_message_by_timestamp() {
        let mut handler = AskSayHandler::new();
        let ts = 1700000000.0;
        let msg = ClineMessage {
            ts,
            r#type: MessageType::Say,
            ask: None,
            say: Some(ClineSay::Text),
            text: Some("test".to_string()),
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
        handler.cline_messages.push(msg);

        let found = handler.find_message_by_timestamp(ts);
        assert!(found.is_some());
        assert_eq!(found.unwrap().text, Some("test".to_string()));

        let not_found = handler.find_message_by_timestamp(99999.0);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_overwrite_cline_messages() {
        let mut handler = AskSayHandler::new();
        handler.cline_messages.push(ClineMessage {
            ts: 1.0,
            r#type: MessageType::Say,
            ask: None,
            say: None,
            text: None,
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
        assert_eq!(handler.cline_messages().len(), 1);

        handler.overwrite_cline_messages(Vec::new());
        assert!(handler.cline_messages().is_empty());
    }

    #[tokio::test]
    async fn test_say_and_create_missing_param_error() {
        let mut handler = AskSayHandler::new();
        let result = handler
            .say_and_create_missing_param_error("read_file", "path", Some("/tmp/test.rs"))
            .await
            .unwrap();

        assert!(result.contains("path"));
        assert!(result.contains("read_file"));
        assert_eq!(handler.cline_messages().len(), 1);
        assert_eq!(handler.cline_messages()[0].say, Some(ClineSay::Error));
    }

    #[test]
    fn test_ask_response_equality() {
        assert_eq!(AskResponse::YesButtonClicked, AskResponse::YesButtonClicked);
        assert_ne!(AskResponse::YesButtonClicked, AskResponse::NoButtonClicked);
        assert_ne!(AskResponse::YesButtonClicked, AskResponse::MessageResponse);
    }

    #[test]
    fn test_ask_ignored_error_display() {
        let err = AskIgnoredError {
            reason: "test".to_string(),
        };
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_ask_state_tracking() {
        let mut handler = AskSayHandler::new();

        // Simulate tracking an idle ask
        handler.idle_ask = Some(ClineMessage {
            ts: 1.0,
            r#type: MessageType::Ask,
            ask: Some(ClineAsk::CompletionResult),
            say: None,
            text: None,
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

        assert!(handler.is_idle());
        assert!(!handler.is_resumable());
        assert!(!handler.is_interactive());

        handler.clear_ask_state();
        assert!(!handler.is_idle());
    }
}
