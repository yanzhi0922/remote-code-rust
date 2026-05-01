//! Runtime stop-hook types plus retry logic for graceful query termination.
//!
//! The compat query engine uses these types to expose a real terminal
//! assistant-response hook seam to the host runtime. This is the engine-side
//! equivalent of Claude Code's stop-hook stage.

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
