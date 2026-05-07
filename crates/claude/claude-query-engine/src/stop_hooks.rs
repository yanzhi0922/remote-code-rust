//! Runtime stop-hook types plus retry logic for graceful query termination.
//!
//! Mirrors TS `stopHooks.ts` with a 7-phase execution pipeline:
//! 1. saveCacheSafeParams — persist cache parameters
//! 2. Job classification — determine if blocking
//! 3. Fire-and-forget background — prompt suggestion, memory extraction, auto-dream
//! 4. Computer-use cleanup — release resources
//! 5. User-configured Stop/SubagentStop hooks (streaming)
//! 6. TeammateIdle/TaskCompleted — teammate-only hooks
//! 7. Return — final decision

use std::collections::BTreeMap;

use claude_core::{AgentId, Message, SessionId};
use serde::{Deserialize, Serialize};

use crate::config::QuerySource;

/// Generic hook context shared by post-sampling and stop-hook callbacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplHookContext {
    pub session_id: SessionId,
    pub turn: u32,
    pub messages: Vec<Message>,
    pub query_source: QuerySource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub user_context: BTreeMap<String, String>,
    #[serde(default)]
    pub system_context: BTreeMap<String, String>,
}

/// Terminal metadata for a stop-hook invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopHookRequest {
    pub stop_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_text: Option<String>,
}

/// Host-directed outcome of a stop-hook callback.
#[derive(Debug, Clone)]
pub enum StopHookOutcome {
    /// The stop is allowed to proceed immediately.
    Allow,
    /// The engine should append the supplied messages and continue the loop.
    Retry { injected_messages: Vec<Message> },
    /// The current stop attempt should be denied.
    Deny,
}

/// Manages stop hook retry behavior for query termination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopHookManager {
    /// Maximum number of retry attempts for stop hooks.
    max_retries: usize,
    /// Current retry count.
    retry_count: usize,
    /// Whether a stop is currently pending.
    pending_stop: bool,
    /// Reason for the stop request.
    stop_reason: Option<String>,
}

/// Result of a stop hook evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopHookResult {
    /// The stop is allowed to proceed.
    Allow,
    /// The stop should be retried after the hook's feedback is incorporated.
    Retry,
    /// The stop is denied; the query should continue.
    Deny,
}

impl From<&StopHookOutcome> for StopHookResult {
    fn from(value: &StopHookOutcome) -> Self {
        match value {
            StopHookOutcome::Allow => Self::Allow,
            StopHookOutcome::Retry { .. } => Self::Retry,
            StopHookOutcome::Deny => Self::Deny,
        }
    }
}

impl StopHookManager {
    /// Create a new stop hook manager with the given maximum retries.
    #[must_use]
    pub fn new(max_retries: usize) -> Self {
        Self {
            max_retries,
            retry_count: 0,
            pending_stop: false,
            stop_reason: None,
        }
    }

    /// Returns the maximum number of retries.
    #[must_use]
    pub fn max_retries(&self) -> usize {
        self.max_retries
    }

    /// Returns the current retry count.
    #[must_use]
    pub fn retry_count(&self) -> usize {
        self.retry_count
    }

    /// Returns true if a stop is currently pending.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending_stop
    }

    /// Request a stop with the given reason.
    pub fn request_stop(&mut self, reason: impl Into<String>) {
        self.pending_stop = true;
        self.stop_reason = Some(reason.into());
        self.retry_count = 0;
    }

    /// Evaluate a stop hook result. Returns true if the stop should proceed.
    pub fn evaluate(&mut self, result: StopHookResult) -> bool {
        match result {
            StopHookResult::Allow => {
                self.pending_stop = false;
                true
            }
            StopHookResult::Retry => {
                if self.retry_count >= self.max_retries {
                    self.pending_stop = false;
                    true
                } else {
                    self.retry_count += 1;
                    false
                }
            }
            StopHookResult::Deny => {
                self.pending_stop = false;
                false
            }
        }
    }

    /// Cancel a pending stop request.
    pub fn cancel(&mut self) {
        self.pending_stop = false;
        self.stop_reason = None;
        self.retry_count = 0;
    }

    /// Returns the stop reason if a stop is pending.
    #[must_use]
    pub fn stop_reason(&self) -> Option<&str> {
        self.stop_reason.as_deref()
    }

    /// Returns true if retries are exhausted.
    #[must_use]
    pub fn retries_exhausted(&self) -> bool {
        self.retry_count >= self.max_retries
    }

    /// Reset the manager to its initial state.
    pub fn reset(&mut self) {
        self.retry_count = 0;
        self.pending_stop = false;
        self.stop_reason = None;
    }
}

impl Default for StopHookManager {
    fn default() -> Self {
        Self::new(3)
    }
}

// ---------------------------------------------------------------------------
// 7-Phase Stop Hook Pipeline
// ---------------------------------------------------------------------------

/// Base input for all stop hook phases.
/// Mirrors TS `BaseHookInput`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopHookBaseInput {
    pub session_id: SessionId,
    pub turn: u32,
    pub stop_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub query_source: QuerySource,
    pub messages: Vec<Message>,
}

/// Result of the full stop hook pipeline.
#[derive(Debug, Clone)]
pub struct StopHookPipelineResult {
    /// Whether the stop is allowed to proceed.
    pub should_stop: bool,
    /// Messages to inject if retrying.
    pub injected_messages: Vec<Message>,
    /// Blocking errors from phase 2 (job classification).
    pub blocking_errors: Vec<String>,
    /// Whether continuation should be prevented.
    pub prevent_continuation: bool,
    /// Which phases were executed.
    pub phases_executed: Vec<StopHookPhase>,
}

impl Default for StopHookPipelineResult {
    fn default() -> Self {
        Self {
            should_stop: true,
            injected_messages: Vec::new(),
            blocking_errors: Vec::new(),
            prevent_continuation: false,
            phases_executed: Vec::new(),
        }
    }
}

/// Phases in the stop hook pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopHookPhase {
    /// Phase 1: Save cache safe parameters.
    SaveCacheSafeParams,
    /// Phase 2: Job classification (blocking).
    JobClassification,
    /// Phase 3: Fire-and-forget background hooks.
    BackgroundFireAndForget,
    /// Phase 4: Computer-use cleanup.
    ComputerUseCleanup,
    /// Phase 5: User-configured Stop/SubagentStop hooks.
    UserConfiguredStopHooks,
    /// Phase 6: TeammateIdle/TaskCompleted hooks.
    TeammateHooks,
    /// Phase 7: Return.
    Return,
}

impl std::fmt::Display for StopHookPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SaveCacheSafeParams => write!(f, "saveCacheSafeParams"),
            Self::JobClassification => write!(f, "jobClassification"),
            Self::BackgroundFireAndForget => write!(f, "backgroundFireAndForget"),
            Self::ComputerUseCleanup => write!(f, "computerUseCleanup"),
            Self::UserConfiguredStopHooks => write!(f, "userConfiguredStopHooks"),
            Self::TeammateHooks => write!(f, "teammateHooks"),
            Self::Return => write!(f, "return"),
        }
    }
}

/// Trait for individual phase handlers in the pipeline.
/// Each phase receives the base input and can modify the pipeline result.
#[async_trait::async_trait]
pub trait StopHookPhaseHandler: Send + Sync {
    /// Execute this phase. Returns Ok(()) to continue, Err to block.
    async fn execute(
        &self,
        input: &StopHookBaseInput,
        result: &mut StopHookPipelineResult,
    ) -> anyhow::Result<()>;
}

/// Orchestrates the 7-phase stop hook pipeline.
pub struct StopHookPipeline {
    /// Phase handlers indexed by phase.
    handlers: Vec<(StopHookPhase, Box<dyn StopHookPhaseHandler>)>,
}

impl StopHookPipeline {
    /// Create a new empty pipeline.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Register a handler for a phase.
    pub fn register_phase(
        &mut self,
        phase: StopHookPhase,
        handler: Box<dyn StopHookPhaseHandler>,
    ) {
        self.handlers.push((phase, handler));
    }

    /// Execute all registered phases in order.
    /// Phases 1-2 are blocking. Phases 3-4 are fire-and-forget.
    /// Phases 5-6 are user-configured hooks.
    pub async fn execute(
        &self,
        input: &StopHookBaseInput,
    ) -> StopHookPipelineResult {
        let mut result = StopHookPipelineResult::default();

        for (phase, handler) in &self.handlers {
            result.phases_executed.push(*phase);

            match phase {
                // Phases 1-2: Blocking — errors stop the pipeline
                StopHookPhase::SaveCacheSafeParams
                | StopHookPhase::JobClassification => {
                    if let Err(err) = handler.execute(input, &mut result).await {
                        result.blocking_errors.push(err.to_string());
                        if *phase == StopHookPhase::JobClassification {
                            result.prevent_continuation = true;
                        }
                    }
                }

                // Phase 3: Fire-and-forget — errors are logged but don't block
                StopHookPhase::BackgroundFireAndForget => {
                    if let Err(err) = handler.execute(input, &mut result).await {
                        tracing::warn!("Background stop hook failed: {err:#}");
                    }
                }

                // Phase 4: Computer-use cleanup — errors are logged
                StopHookPhase::ComputerUseCleanup => {
                    if let Err(err) = handler.execute(input, &mut result).await {
                        tracing::warn!("Computer-use cleanup hook failed: {err:#}");
                    }
                }

                // Phases 5-6: User-configured hooks — can inject messages
                StopHookPhase::UserConfiguredStopHooks
                | StopHookPhase::TeammateHooks => {
                    if let Err(err) = handler.execute(input, &mut result).await {
                        tracing::warn!("Stop hook phase {phase} failed: {err:#}");
                    }
                }

                StopHookPhase::Return => {
                    // Terminal phase — no action
                }
            }
        }

        result
    }
}

impl Default for StopHookPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use claude_core::{ConversationEntry, Message, PermissionMode, SessionId};

    use crate::config::ProcessUserInputContext;

    use super::{
        ReplHookContext, StopHookManager, StopHookOutcome, StopHookRequest, StopHookResult,
    };

    #[test]
    fn stop_hook_allows_immediate_stop() {
        let mut mgr = StopHookManager::new(3);
        mgr.request_stop("user requested");
        assert!(mgr.is_pending());
        let should_stop = mgr.evaluate(StopHookResult::Allow);
        assert!(should_stop);
        assert!(!mgr.is_pending());
    }

    #[test]
    fn stop_hook_retries_then_allows() {
        let mut mgr = StopHookManager::new(2);
        mgr.request_stop("budget");
        assert!(!mgr.evaluate(StopHookResult::Retry));
        assert!(!mgr.evaluate(StopHookResult::Retry));
        // After max retries, should force-stop
        assert!(mgr.evaluate(StopHookResult::Retry));
    }

    #[test]
    fn stop_hook_deny_cancels_stop() {
        let mut mgr = StopHookManager::new(3);
        mgr.request_stop("user");
        let should_stop = mgr.evaluate(StopHookResult::Deny);
        assert!(!should_stop);
        assert!(!mgr.is_pending());
    }

    #[test]
    fn stop_hook_cancel_clears_state() {
        let mut mgr = StopHookManager::new(3);
        mgr.request_stop("test");
        mgr.cancel();
        assert!(!mgr.is_pending());
        assert!(mgr.stop_reason().is_none());
    }

    #[test]
    fn stop_hook_retries_exhausted() {
        let mut mgr = StopHookManager::new(1);
        assert!(!mgr.retries_exhausted());
        mgr.request_stop("test");
        mgr.evaluate(StopHookResult::Retry);
        assert!(mgr.retries_exhausted());
    }

    #[test]
    fn stop_hook_reset_clears_all() {
        let mut mgr = StopHookManager::new(3);
        mgr.request_stop("test");
        mgr.evaluate(StopHookResult::Retry);
        mgr.reset();
        assert!(!mgr.is_pending());
        assert_eq!(mgr.retry_count(), 0);
    }

    #[test]
    fn stop_hook_default_is_3_retries() {
        let mgr = StopHookManager::default();
        assert_eq!(mgr.max_retries(), 3);
    }

    #[test]
    fn stop_hook_outcome_maps_to_retry_result() {
        assert_eq!(
            StopHookResult::from(&StopHookOutcome::Allow),
            StopHookResult::Allow
        );
        assert_eq!(
            StopHookResult::from(&StopHookOutcome::Retry {
                injected_messages: vec![Message::from(ConversationEntry::user("retry"))],
            }),
            StopHookResult::Retry
        );
        assert_eq!(
            StopHookResult::from(&StopHookOutcome::Deny),
            StopHookResult::Deny
        );
    }

    #[test]
    fn repl_hook_context_carries_prompt_and_context_maps() {
        let session_id = SessionId::new();
        let mut process =
            ProcessUserInputContext::new(session_id.clone(), PermissionMode::Default, "mock");
        process.system_prompt = Some("system".to_owned());
        process
            .user_context
            .insert("currentDate".to_owned(), "Today".to_owned());
        process
            .system_context
            .insert("gitStatus".to_owned(), "clean".to_owned());

        let context = ReplHookContext {
            session_id,
            turn: 2,
            messages: vec![Message::from(ConversationEntry::user("hello"))],
            query_source: process.query_source,
            agent_id: process.agent_id.clone(),
            system_prompt: process.system_prompt.clone(),
            user_context: process.user_context.clone(),
            system_context: process.system_context.clone(),
        };

        assert_eq!(context.turn, 2);
        assert_eq!(context.system_prompt.as_deref(), Some("system"));
        assert_eq!(
            context.user_context.get("currentDate").map(String::as_str),
            Some("Today")
        );
        assert_eq!(
            context.system_context.get("gitStatus").map(String::as_str),
            Some("clean")
        );
    }

    #[test]
    fn stop_hook_request_carries_terminal_metadata() {
        let request = StopHookRequest {
            stop_reason: "end_turn".to_owned(),
            final_text: Some("done".to_owned()),
        };

        assert_eq!(request.stop_reason, "end_turn");
        assert_eq!(request.final_text.as_deref(), Some("done"));
    }
}
