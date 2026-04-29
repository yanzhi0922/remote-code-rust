//! Max output tokens recovery — triggered when model output is truncated.
//!
//! Inspired by Claude Code's `query.ts` (lines 1188–1256): when the model
//! hits the `max_output_tokens` limit, this module provides an escalating
//! recovery strategy:
//!
//! 1. **Escalation** — retry the same request with a higher token limit
//!    (8k → 16k → 64k).
//! 2. **Continuation** — inject a meta-user-message asking the model to
//!    continue from where it left off.
//! 3. **Exhaustion** — after `max_recoveries` attempts, surface the error.

use rc_core::Message;

use crate::preprocessing::create_continuation_message;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum number of recovery attempts per query.
pub const DEFAULT_MAX_RECOVERIES: usize = 3;

/// Escalation tiers for max_output_tokens (in tokens).
pub const DEFAULT_ESCALATION_TOKENS: [usize; 3] = [8192, 16384, 65536];

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Action to take when the model output is truncated.
#[derive(Debug, Clone)]
pub enum MaxTokensRecoveryAction {
    /// Retry the same request with a higher `max_tokens` value.
    Escalate {
        /// The new `max_tokens` to use.
        new_max_tokens: usize,
    },
    /// Inject a continuation message and keep the current token limit.
    ContinueWithMessage {
        /// The `max_tokens` to use for the continuation request.
        max_tokens: usize,
        /// The continuation message to append.
        continuation_message: Message,
    },
    /// Recovery exhausted — the error should be surfaced to the user.
    Exhausted,
}

/// Manages recovery from max-output-tokens truncation.
///
/// Tracks how many recovery attempts have been made and decides the
/// appropriate action for each truncation event.
#[derive(Debug, Clone)]
pub struct MaxTokensRecovery {
    /// Number of recovery attempts already made in the current query.
    pub recovery_count: usize,
    /// Maximum number of recovery attempts before giving up.
    pub max_recoveries: usize,
    /// Escalation tiers: each entry is a `max_tokens` value to try.
    pub escalation_tokens: [usize; 3],
}

impl Default for MaxTokensRecovery {
    fn default() -> Self {
        Self {
            recovery_count: 0,
            max_recoveries: DEFAULT_MAX_RECOVERIES,
            escalation_tokens: DEFAULT_ESCALATION_TOKENS,
        }
    }
}

impl MaxTokensRecovery {
    /// Create a new recovery handler with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new recovery handler with a custom max-recoveries limit.
    #[must_use]
    pub fn with_max_recoveries(mut self, max: usize) -> Self {
        self.max_recoveries = max;
        self
    }

    /// Create a new recovery handler with custom escalation tiers.
    #[must_use]
    pub fn with_escalation_tokens(mut self, tokens: [usize; 3]) -> Self {
        self.escalation_tokens = tokens;
        self
    }

    /// Returns `true` if recovery is still possible.
    #[must_use]
    pub fn can_recover(&self) -> bool {
        self.recovery_count < self.max_recoveries
    }

    /// Reset the recovery counter (e.g. at the start of a new query).
    pub fn reset(&mut self) {
        self.recovery_count = 0;
    }

    /// Determine the recovery action for a truncation event.
    ///
    /// * `current_max_tokens` — the `max_tokens` value used in the truncated request.
    ///
    /// Returns the appropriate [`MaxTokensRecoveryAction`]:
    /// - If an escalation tier exists above `current_max_tokens`, returns
    ///   [`Escalate`](MaxTokensRecoveryAction::Escalate).
    /// - Otherwise, if recovery attempts remain, returns
    ///   [`ContinueWithMessage`](MaxTokensRecoveryAction::ContinueWithMessage).
    /// - If all attempts are exhausted, returns
    ///   [`Exhausted`](MaxTokensRecoveryAction::Exhausted).
    pub fn handle_truncation(
        &mut self,
        current_max_tokens: usize,
    ) -> Option<MaxTokensRecoveryAction> {
        if !self.can_recover() {
            return Some(MaxTokensRecoveryAction::Exhausted);
        }

        // Try escalation first: find the first tier above current_max_tokens
        for &tier_tokens in &self.escalation_tokens {
            if tier_tokens > current_max_tokens {
                self.recovery_count += 1;
                return Some(MaxTokensRecoveryAction::Escalate {
                    new_max_tokens: tier_tokens,
                });
            }
        }

        // No escalation tier available — use continuation message
        self.recovery_count += 1;
        Some(MaxTokensRecoveryAction::ContinueWithMessage {
            max_tokens: current_max_tokens,
            continuation_message: create_continuation_message(),
        })
    }

    /// Returns the current recovery count.
    #[must_use]
    pub fn recovery_count(&self) -> usize {
        self.recovery_count
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Test 1: Escalation from 8k to 16k ----

    #[test]
    fn escalation_from_8k_to_16k() {
        let mut recovery = MaxTokensRecovery::new();
        let action = recovery
            .handle_truncation(8192)
            .expect("8192-token truncation should produce a recovery action");

        match action {
            MaxTokensRecoveryAction::Escalate { new_max_tokens } => {
                assert_eq!(new_max_tokens, 16384);
            }
            other => panic!("Expected Escalate, got {other:?}"),
        }
        assert_eq!(recovery.recovery_count(), 1);
    }

    // ---- Test 2: Escalation from 16k to 64k ----

    #[test]
    fn escalation_from_16k_to_64k() {
        let mut recovery = MaxTokensRecovery::new();
        let action = recovery
            .handle_truncation(16384)
            .expect("16384-token truncation should produce a recovery action");

        match action {
            MaxTokensRecoveryAction::Escalate { new_max_tokens } => {
                assert_eq!(new_max_tokens, 65536);
            }
            other => panic!("Expected Escalate, got {other:?}"),
        }
    }

    // ---- Test 3: Continuation message when no escalation tier available ----

    #[test]
    fn continuation_message_when_no_escalation() {
        let mut recovery = MaxTokensRecovery::new();
        let action = recovery
            .handle_truncation(65536)
            .expect("65536-token truncation should produce a recovery action");

        match action {
            MaxTokensRecoveryAction::ContinueWithMessage { max_tokens, .. } => {
                assert_eq!(max_tokens, 65536);
            }
            other => panic!("Expected ContinueWithMessage, got {other:?}"),
        }
    }

    // ---- Test 4: Exhaustion after max recoveries ----

    #[test]
    fn exhaustion_after_max_recoveries() {
        let mut recovery = MaxTokensRecovery::new().with_max_recoveries(1);
        let _ = recovery.handle_truncation(8192);
        let action = recovery
            .handle_truncation(8192)
            .expect("second recovery attempt should report exhaustion");

        assert!(matches!(action, MaxTokensRecoveryAction::Exhausted));
    }

    // ---- Test 5: Can recover check ----

    #[test]
    fn can_recover_check() {
        let mut recovery = MaxTokensRecovery::new().with_max_recoveries(2);
        assert!(recovery.can_recover());

        let _ = recovery.handle_truncation(8192);
        assert!(recovery.can_recover());

        let _ = recovery.handle_truncation(8192);
        assert!(!recovery.can_recover());
    }

    // ---- Test 6: Reset allows new recoveries ----

    #[test]
    fn reset_allows_new_recoveries() {
        let mut recovery = MaxTokensRecovery::new().with_max_recoveries(1);
        let _ = recovery.handle_truncation(8192);
        assert!(!recovery.can_recover());

        recovery.reset();
        assert!(recovery.can_recover());
        assert_eq!(recovery.recovery_count(), 0);
    }

    // ---- Test 7: Custom escalation tiers ----

    #[test]
    fn custom_escalation_tiers() {
        let mut recovery = MaxTokensRecovery::new().with_escalation_tokens([4096, 8192, 32768]);
        let action = recovery
            .handle_truncation(4096)
            .expect("custom escalation tier should produce a recovery action");

        match action {
            MaxTokensRecoveryAction::Escalate { new_max_tokens } => {
                assert_eq!(new_max_tokens, 8192);
            }
            other => panic!("Expected Escalate, got {other:?}"),
        }
    }

    // ---- Test 8: Zero max_tokens triggers first escalation ----

    #[test]
    fn zero_max_tokens_triggers_first_escalation() {
        let mut recovery = MaxTokensRecovery::new();
        let action = recovery
            .handle_truncation(0)
            .expect("zero max_tokens should produce a recovery action");

        match action {
            MaxTokensRecoveryAction::Escalate { new_max_tokens } => {
                assert_eq!(new_max_tokens, 8192);
            }
            other => panic!("Expected Escalate, got {other:?}"),
        }
    }

    // ---- Test 9: Multiple escalations in sequence ----

    #[test]
    fn multiple_escalations_in_sequence() {
        let mut recovery = MaxTokensRecovery::new().with_max_recoveries(5);

        // 0 → 8192
        let a1 = recovery
            .handle_truncation(0)
            .expect("first escalation should produce a recovery action");
        match a1 {
            MaxTokensRecoveryAction::Escalate { new_max_tokens } => {
                assert_eq!(new_max_tokens, 8192);
            }
            other => panic!("Expected Escalate, got {other:?}"),
        }

        // 8192 → 16384
        let a2 = recovery
            .handle_truncation(8192)
            .expect("second escalation should produce a recovery action");
        match a2 {
            MaxTokensRecoveryAction::Escalate { new_max_tokens } => {
                assert_eq!(new_max_tokens, 16384);
            }
            other => panic!("Expected Escalate, got {other:?}"),
        }

        // 16384 → 65536
        let a3 = recovery
            .handle_truncation(16384)
            .expect("third escalation should produce a recovery action");
        match a3 {
            MaxTokensRecoveryAction::Escalate { new_max_tokens } => {
                assert_eq!(new_max_tokens, 65536);
            }
            other => panic!("Expected Escalate, got {other:?}"),
        }

        // 65536 → continuation
        let a4 = recovery
            .handle_truncation(65536)
            .expect("continuation recovery should produce a recovery action");
        assert!(matches!(
            a4,
            MaxTokensRecoveryAction::ContinueWithMessage { .. }
        ));

        assert_eq!(recovery.recovery_count(), 4);
    }

    // ---- Test 10: Default values are correct ----

    #[test]
    fn default_values() {
        let recovery = MaxTokensRecovery::new();
        assert_eq!(recovery.recovery_count(), 0);
        assert_eq!(recovery.max_recoveries, DEFAULT_MAX_RECOVERIES);
        assert_eq!(recovery.escalation_tokens, DEFAULT_ESCALATION_TOKENS);
    }
}
