//! Stop hook retry logic for graceful query termination.
//!
//! When a stop hook rejects a termination attempt, the engine can retry
//! the stop after a configurable number of attempts.

use serde::{Deserialize, Serialize};

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
    use super::{StopHookManager, StopHookResult};

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
}
