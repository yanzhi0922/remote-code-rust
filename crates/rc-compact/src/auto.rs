//! Auto Compact strategy.
//!
//! Automatically triggers compaction when token usage exceeds a configurable
//! threshold.  Mirrors `services/compact/autoCompact.ts`.

use rc_core::Message;

use crate::engine::compact_conversation;
use crate::prompt::rough_token_count;
use crate::strategy::{
    CompactOptions, CompactStrategy, CompactStrategyType, CompactionResult, ProgressCallback,
    SummaryProvider,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Buffer tokens subtracted from the effective context window to determine
/// the auto-compact threshold.
pub const AUTOCOMPACT_BUFFER_TOKENS: u64 = 13_000;

/// Buffer for the warning threshold.
pub const WARNING_THRESHOLD_BUFFER_TOKENS: u64 = 20_000;

/// Buffer for the error threshold.
pub const ERROR_THRESHOLD_BUFFER_TOKENS: u64 = 20_000;

/// Buffer for the manual-compact blocking limit.
pub const MANUAL_COMPACT_BUFFER_TOKENS: u64 = 3_000;

/// Maximum consecutive auto-compact failures before circuit-breaker trips.
pub const MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES: u32 = 3;

/// Reserve this many tokens for the compact summary output.
const MAX_OUTPUT_TOKENS_FOR_SUMMARY: u64 = 20_000;

// ---------------------------------------------------------------------------
// Auto-compact tracking state
// ---------------------------------------------------------------------------

/// Tracks auto-compact state across turns.
#[derive(Debug, Clone)]
pub struct AutoCompactTrackingState {
    /// Whether compaction has occurred in this session.
    pub compacted: bool,
    /// Monotonically increasing turn counter.
    pub turn_counter: u64,
    /// Unique ID per turn.
    pub turn_id: String,
    /// Consecutive auto-compact failures (circuit breaker).
    pub consecutive_failures: u32,
}

impl Default for AutoCompactTrackingState {
    fn default() -> Self {
        Self {
            compacted: false,
            turn_counter: 0,
            turn_id: uuid::Uuid::new_v4().to_string(),
            consecutive_failures: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Token warning state
// ---------------------------------------------------------------------------

/// Result of checking token usage against thresholds.
#[derive(Debug, Clone)]
pub struct TokenWarningState {
    /// Percentage of context remaining.
    pub percent_left: u32,
    /// Token usage is above the warning threshold.
    pub is_above_warning_threshold: bool,
    /// Token usage is above the error threshold.
    pub is_above_error_threshold: bool,
    /// Token usage is above the auto-compact threshold.
    pub is_above_auto_compact_threshold: bool,
    /// Token usage is at the blocking limit.
    pub is_at_blocking_limit: bool,
}

// ---------------------------------------------------------------------------
// Auto compact strategy
// ---------------------------------------------------------------------------

/// Auto-compact strategy that triggers when token usage exceeds a threshold.
pub struct AutoCompactStrategy {
    /// Effective context window size for the model.
    pub context_window_size: u64,
}

impl AutoCompactStrategy {
    /// Create a new auto-compact strategy for the given context window size.
    pub fn new(context_window_size: u64) -> Self {
        Self {
            context_window_size,
        }
    }

    /// Return the effective context window size (minus reserved output tokens).
    pub fn effective_context_window(&self) -> u64 {
        self.context_window_size
            .saturating_sub(MAX_OUTPUT_TOKENS_FOR_SUMMARY)
    }

    /// Return the auto-compact threshold.
    pub fn auto_compact_threshold(&self) -> u64 {
        self.effective_context_window()
            .saturating_sub(AUTOCOMPACT_BUFFER_TOKENS)
    }

    /// Check if auto-compact should be triggered based on current token usage.
    pub fn should_auto_compact(
        &self,
        token_usage: u64,
        tracking: &AutoCompactTrackingState,
    ) -> bool {
        // Circuit breaker: stop trying after too many consecutive failures
        if tracking.consecutive_failures >= MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES {
            return false;
        }

        token_usage >= self.auto_compact_threshold()
    }

    /// Calculate the token warning state for the given usage.
    pub fn calculate_token_warning_state(&self, token_usage: u64) -> TokenWarningState {
        let threshold = self.auto_compact_threshold();
        let percent_left = if threshold > 0 {
            std::cmp::max(
                0,
                ((threshold.saturating_sub(token_usage)) * 100 / threshold) as u32,
            )
        } else {
            0
        };

        let warning_threshold = threshold.saturating_sub(WARNING_THRESHOLD_BUFFER_TOKENS);
        let error_threshold = threshold.saturating_sub(ERROR_THRESHOLD_BUFFER_TOKENS);
        let blocking_limit = self
            .effective_context_window()
            .saturating_sub(MANUAL_COMPACT_BUFFER_TOKENS);

        TokenWarningState {
            percent_left,
            is_above_warning_threshold: token_usage >= warning_threshold,
            is_above_error_threshold: token_usage >= error_threshold,
            is_above_auto_compact_threshold: token_usage >= threshold,
            is_at_blocking_limit: token_usage >= blocking_limit,
        }
    }
}

#[async_trait::async_trait]
impl CompactStrategy for AutoCompactStrategy {
    fn strategy_type(&self) -> CompactStrategyType {
        CompactStrategyType::Auto
    }

    async fn compact(
        &self,
        messages: &[Message],
        options: &CompactOptions,
        provider: &dyn SummaryProvider,
        progress: Option<&ProgressCallback>,
    ) -> Result<CompactionResult, anyhow::Error> {
        // Delegate to the full compact engine with auto-compact flag set
        let auto_options = CompactOptions {
            is_auto_compact: true,
            ..options.clone()
        };

        let mut result = compact_conversation(messages, &auto_options, provider, progress).await?;
        result.strategy_used = CompactStrategyType::Auto;
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Standalone helper functions
// ---------------------------------------------------------------------------

/// Check whether auto-compact should trigger for the given messages.
///
/// This is a convenience wrapper that estimates token usage from the messages
/// and compares against the threshold.
pub fn should_auto_compact(
    messages: &[Message],
    context_window_size: u64,
    tracking: &AutoCompactTrackingState,
) -> bool {
    let strategy = AutoCompactStrategy::new(context_window_size);
    let token_usage = estimate_messages_tokens(messages);
    strategy.should_auto_compact(token_usage, tracking)
}

/// Execute auto-compact on the given messages.
///
/// Returns `None` if auto-compact is not needed or fails gracefully.
pub async fn auto_compact(
    messages: &[Message],
    context_window_size: u64,
    options: &CompactOptions,
    provider: &dyn SummaryProvider,
    tracking: &mut AutoCompactTrackingState,
) -> Result<Option<CompactionResult>, anyhow::Error> {
    let strategy = AutoCompactStrategy::new(context_window_size);
    let token_usage = estimate_messages_tokens(messages);

    if !strategy.should_auto_compact(token_usage, tracking) {
        return Ok(None);
    }

    match strategy.compact(messages, options, provider, None).await {
        Ok(result) => {
            tracking.compacted = true;
            tracking.consecutive_failures = 0;
            Ok(Some(result))
        }
        Err(e) => {
            tracking.consecutive_failures += 1;
            Err(e)
        }
    }
}

/// Rough token estimation for a slice of messages.
///
/// Each message is estimated via [`rough_token_count`] which already includes
/// a conservative 4/3 padding factor, so no additional padding is applied here.
fn estimate_messages_tokens(messages: &[Message]) -> u64 {
    let mut total: u64 = 0;
    for msg in messages {
        total += estimate_single_message_tokens(msg);
    }
    total
}

/// Estimate tokens for a single message.
fn estimate_single_message_tokens(msg: &Message) -> u64 {
    match msg {
        Message::User(m) => rough_token_count(&m.text),
        Message::Assistant(m) => rough_token_count(&m.text),
        Message::System(m) => rough_token_count(&m.text),
        Message::Progress(m) => rough_token_count(&m.status),
        Message::Attachment(m) => {
            let mut t = m.label.as_deref().map_or(0, rough_token_count);
            for att in &m.attachments {
                t += rough_token_count(&att.data);
            }
            t
        }
        Message::HookResult(m) => rough_token_count(&m.output),
        Message::ToolUseSummary(m) => rough_token_count(&m.summary),
        Message::Tombstone(m) => rough_token_count(&m.summary),
        Message::GroupedToolUse(m) => m.summary.as_deref().map_or(0, rough_token_count),
        Message::CollapsedReadSearch(m) => rough_token_count(&m.summary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_compact_threshold_calculation() {
        let strategy = AutoCompactStrategy::new(200_000);
        let effective = strategy.effective_context_window();
        assert_eq!(effective, 200_000 - MAX_OUTPUT_TOKENS_FOR_SUMMARY);
        let threshold = strategy.auto_compact_threshold();
        assert_eq!(threshold, effective - AUTOCOMPACT_BUFFER_TOKENS);
    }

    #[test]
    fn should_auto_compact_below_threshold() {
        let strategy = AutoCompactStrategy::new(200_000);
        let tracking = AutoCompactTrackingState::default();
        assert!(!strategy.should_auto_compact(100_000, &tracking));
    }

    #[test]
    fn should_auto_compact_above_threshold() {
        let strategy = AutoCompactStrategy::new(200_000);
        let tracking = AutoCompactTrackingState::default();
        let threshold = strategy.auto_compact_threshold();
        assert!(strategy.should_auto_compact(threshold, &tracking));
    }

    #[test]
    fn circuit_breaker_trips() {
        let strategy = AutoCompactStrategy::new(200_000);
        let tracking = AutoCompactTrackingState {
            consecutive_failures: MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES,
            ..AutoCompactTrackingState::default()
        };
        assert!(!strategy.should_auto_compact(u64::MAX, &tracking));
    }

    #[test]
    fn token_warning_state_calculation() {
        let strategy = AutoCompactStrategy::new(200_000);
        let state = strategy.calculate_token_warning_state(0);
        assert!(state.percent_left > 0);
        assert!(!state.is_above_warning_threshold);
        assert!(!state.is_at_blocking_limit);
    }
}
