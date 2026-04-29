//! Loop control for the task engine.
//!
//! Provides [`LoopControl`] which manages iteration counting, consecutive
//! mistake limits, cancellation, and the decision of whether to continue
//! the task loop.
//!
//! Source: `src/core/task/Task.ts` — mistake tracking, loop control

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// LoopControl
// ---------------------------------------------------------------------------

/// Controls the task loop execution.
///
/// Tracks:
/// - Consecutive mistake count and limit
/// - Per-tool-type consecutive mistake counts (for apply_diff, edit_file)
/// - Consecutive no-tool-use count
/// - Maximum iterations
/// - Current iteration
/// - Cancellation state
/// - Whether a tool failed in the current turn
///
/// Source: `src/core/task/Task.ts` — `consecutiveMistakeCount`, `consecutiveMistakeLimit`,
/// `consecutiveMistakeCountForApplyDiff`, `consecutiveMistakeCountForEditFile`,
/// `consecutiveNoToolUseCount`
#[derive(Debug, Clone)]
pub struct LoopControl {
    /// Number of consecutive mistakes (e.g., tool failures, API errors).
    pub consecutive_mistake_count: usize,
    /// Maximum allowed consecutive mistakes before stopping.
    /// When `consecutive_mistake_count >= max_consecutive_mistakes`, the limit is reached.
    pub max_consecutive_mistakes: usize,
    /// Maximum number of iterations (None = unlimited).
    pub max_iterations: Option<usize>,
    /// Current iteration number (0-based).
    pub current_iteration: usize,
    /// Whether the task has been cancelled.
    pub is_cancelled: bool,
    /// Whether a tool failed in the current turn.
    pub did_tool_fail_in_current_turn: bool,
    /// Number of consecutive iterations where the model did not use any tools.
    ///
    /// Source: `src/core/task/Task.ts` — `consecutiveNoToolUseCount`
    pub consecutive_no_tool_use_count: usize,
    /// Number of consecutive iterations where the model did not produce any
    /// assistant messages.
    ///
    /// Source: `src/core/task/Task.ts` — `consecutiveNoAssistantMessagesCount`
    pub consecutive_no_assistant_messages_count: usize,
    /// Per-file consecutive mistake count for apply_diff operations.
    ///
    /// Source: `src/core/task/Task.ts` — `consecutiveMistakeCountForApplyDiff`
    pub consecutive_mistake_count_for_apply_diff: HashMap<String, usize>,
    /// Per-file consecutive mistake count for edit_file operations.
    ///
    /// Source: `src/core/task/Task.ts` — `consecutiveMistakeCountForEditFile`
    pub consecutive_mistake_count_for_edit_file: HashMap<String, usize>,
    /// Whether the one-time mistake limit grace has been used.
    ///
    /// When the mistake limit is reached for the first time, the engine can
    /// reset `consecutive_mistake_count` to 0 and set this flag to `true`,
    /// giving the model one extra chance. If the limit is reached again,
    /// the task terminates.
    pub mistake_grace_used: bool,
    /// Per-tool consecutive error counts.
    ///
    /// Tracks how many times each tool has failed consecutively.
    /// When a tool succeeds, its count is reset to 0.
    /// Used to abort the task when a tool exceeds its error threshold.
    ///
    /// Source: `src/core/task/Task.ts` — consecutive error tracking per tool
    pub consecutive_errors: HashMap<String, u32>,
}

impl LoopControl {
    /// Create a new loop control with the given mistake limit.
    pub fn new(max_consecutive_mistakes: usize) -> Self {
        Self {
            consecutive_mistake_count: 0,
            max_consecutive_mistakes,
            max_iterations: None,
            current_iteration: 0,
            is_cancelled: false,
            did_tool_fail_in_current_turn: false,
            consecutive_no_tool_use_count: 0,
            consecutive_no_assistant_messages_count: 0,
            consecutive_mistake_count_for_apply_diff: HashMap::new(),
            consecutive_mistake_count_for_edit_file: HashMap::new(),
            mistake_grace_used: false,
            consecutive_errors: HashMap::new(),
        }
    }

    /// Create loop control with a maximum iteration limit.
    pub fn with_max_iterations(max_consecutive_mistakes: usize, max_iterations: usize) -> Self {
        Self {
            consecutive_mistake_count: 0,
            max_consecutive_mistakes,
            max_iterations: Some(max_iterations),
            current_iteration: 0,
            is_cancelled: false,
            did_tool_fail_in_current_turn: false,
            consecutive_no_tool_use_count: 0,
            consecutive_no_assistant_messages_count: 0,
            consecutive_mistake_count_for_apply_diff: HashMap::new(),
            consecutive_mistake_count_for_edit_file: HashMap::new(),
            mistake_grace_used: false,
            consecutive_errors: HashMap::new(),
        }
    }

    /// Check whether the task loop should continue.
    ///
    /// Returns `false` if:
    /// - The task has been cancelled
    /// - The consecutive mistake limit has been reached or exceeded (`>=`)
    /// - The maximum iteration limit has been reached
    ///
    /// **Note**: The mistake limit uses `>=` comparison to match the TypeScript
    /// behavior where `consecutiveMistakeCount >= consecutiveMistakeLimit` triggers
    /// the limit check.
    pub fn should_continue(&self) -> bool {
        if self.is_cancelled {
            return false;
        }

        // BUG FIX: Changed from `>` to `>=` to match TypeScript behavior.
        // TS: `this.consecutiveMistakeCount >= this.consecutiveMistakeLimit`
        if self.max_consecutive_mistakes > 0
            && self.consecutive_mistake_count >= self.max_consecutive_mistakes
        {
            return false;
        }

        if let Some(max) = self.max_iterations {
            if self.current_iteration >= max {
                return false;
            }
        }

        true
    }

    /// Check whether the consecutive mistake limit has been reached.
    ///
    /// Returns `true` when `consecutive_mistake_count >= max_consecutive_mistakes`.
    /// This matches the TypeScript behavior where the limit triggers at equality.
    pub fn is_mistake_limit_reached(&self) -> bool {
        self.max_consecutive_mistakes > 0
            && self.consecutive_mistake_count >= self.max_consecutive_mistakes
    }

    /// Record a mistake (e.g., tool failure).
    ///
    /// Returns `true` if the mistake limit has been reached or exceeded.
    pub fn record_mistake(&mut self) -> bool {
        self.consecutive_mistake_count += 1;
        self.did_tool_fail_in_current_turn = true;
        // BUG FIX: Changed from `>` to `>=` to match TypeScript behavior
        self.consecutive_mistake_count >= self.max_consecutive_mistakes
    }

    /// Record a per-file mistake for apply_diff.
    ///
    /// Source: `src/core/task/Task.ts` — `consecutiveMistakeCountForApplyDiff`
    pub fn record_apply_diff_mistake(&mut self, file_path: &str) -> usize {
        let count = self
            .consecutive_mistake_count_for_apply_diff
            .entry(file_path.to_string())
            .or_insert(0);
        *count += 1;
        *count
    }

    /// Reset the per-file apply_diff mistake count for a specific file.
    pub fn reset_apply_diff_mistake(&mut self, file_path: &str) {
        self.consecutive_mistake_count_for_apply_diff.remove(file_path);
    }

    /// Record a per-file mistake for edit_file.
    ///
    /// Source: `src/core/task/Task.ts` — `consecutiveMistakeCountForEditFile`
    pub fn record_edit_file_mistake(&mut self, file_path: &str) -> usize {
        let count = self
            .consecutive_mistake_count_for_edit_file
            .entry(file_path.to_string())
            .or_insert(0);
        *count += 1;
        *count
    }

    /// Reset the per-file edit_file mistake count for a specific file.
    pub fn reset_edit_file_mistake(&mut self, file_path: &str) {
        self.consecutive_mistake_count_for_edit_file.remove(file_path);
    }

    /// Record that the model did not use any tools in this iteration.
    ///
    /// Increments `consecutive_no_tool_use_count`. When the count reaches 2
    /// or more, also increments `consecutive_mistake_count` to match the TS
    /// behavior where `consecutiveNoToolUseCount >= 2` counts as a mistake.
    ///
    /// Source: `src/core/task/Task.ts` — `initiateTaskLoop()` outer loop
    pub fn record_no_tool_use(&mut self) {
        self.consecutive_no_tool_use_count += 1;
        if self.consecutive_no_tool_use_count >= 2 {
            self.consecutive_mistake_count += 1;
        }
    }

    /// Reset the consecutive no-tool-use count.
    ///
    /// Called when the model successfully uses a tool.
    pub fn reset_no_tool_use_count(&mut self) {
        self.consecutive_no_tool_use_count = 0;
    }

    /// Alias for [`Self::reset_no_tool_use_count`].
    pub fn reset_no_tool_use(&mut self) {
        self.reset_no_tool_use_count();
    }

    /// Record that the model did not produce an assistant message in this iteration.
    ///
    /// Increments `consecutive_no_assistant_messages_count`. When the count
    /// reaches 2 or more, also increments `consecutive_mistake_count`.
    ///
    /// Source: `src/core/task/Task.ts` — `consecutiveNoAssistantMessagesCount`
    pub fn record_no_assistant_message(&mut self) {
        self.consecutive_no_assistant_messages_count += 1;
        if self.consecutive_no_assistant_messages_count >= 2 {
            self.consecutive_mistake_count += 1;
        }
    }

    /// Reset the consecutive no-assistant-messages count.
    ///
    /// Called when the model produces an assistant message.
    pub fn reset_no_assistant_messages_count(&mut self) {
        self.consecutive_no_assistant_messages_count = 0;
    }

    /// Alias for [`Self::reset_no_assistant_messages_count`].
    pub fn reset_no_assistant_message(&mut self) {
        self.reset_no_assistant_messages_count();
    }

    /// Check whether we should retry after receiving an empty response.
    ///
    /// Returns `true` when `consecutive_no_assistant_messages_count < 2`,
    /// allowing up to 2 retries for empty API responses.
    pub fn should_retry_empty_response(&self) -> bool {
        self.consecutive_no_assistant_messages_count < 2
    }

    /// Try to use the one-time mistake limit grace.
    ///
    /// When the consecutive mistake limit is reached for the first time,
    /// this method resets the mistake count and marks grace as used,
    /// giving the model one extra chance. Returns `true` if grace was
    /// applied (i.e., this is the first time the limit was reached).
    ///
    /// If grace has already been used, returns `false` and does nothing.
    pub fn try_use_mistake_grace(&mut self) -> bool {
        if self.mistake_grace_used {
            return false;
        }
        self.mistake_grace_used = true;
        self.consecutive_mistake_count = 0;
        true
    }

    /// Reset the consecutive mistake count.
    ///
    /// Called when a successful action occurs. Also resets per-tool mistake counts.
    pub fn reset_mistake_count(&mut self) {
        self.consecutive_mistake_count = 0;
        self.did_tool_fail_in_current_turn = false;
    }

    /// Increment the iteration counter.
    ///
    /// Returns `true` if the maximum iteration limit has been reached.
    pub fn increment_iteration(&mut self) -> bool {
        self.current_iteration += 1;
        if let Some(max) = self.max_iterations {
            self.current_iteration >= max
        } else {
            false
        }
    }

    /// Cancel the task loop.
    ///
    /// Also resets consecutive error counters on abort (manual intervention),
    /// matching the TypeScript behavior in `abortTask`.
    pub fn cancel(&mut self) {
        self.is_cancelled = true;
        self.consecutive_no_tool_use_count = 0;
        self.consecutive_no_assistant_messages_count = 0;
    }

    /// Reset the turn-level state (called at the beginning of each iteration).
    pub fn reset_turn(&mut self) {
        self.did_tool_fail_in_current_turn = false;
    }

    /// Get the remaining iterations, if a limit is set.
    pub fn remaining_iterations(&self) -> Option<usize> {
        self.max_iterations.map(|max| max.saturating_sub(self.current_iteration))
    }

    // -----------------------------------------------------------------------
    // Per-tool consecutive error tracking
    // -----------------------------------------------------------------------

    /// Record a tool error, incrementing the consecutive error count for the
    /// given tool name.
    ///
    /// Source: `src/core/task/Task.ts` — consecutive error tracking per tool
    pub fn record_tool_error(&mut self, tool_name: &str) -> u32 {
        let count = self
            .consecutive_errors
            .entry(tool_name.to_string())
            .or_insert(0);
        *count += 1;
        *count
    }

    /// Record a tool success, resetting the consecutive error count for the
    /// given tool name to 0.
    ///
    /// Source: `src/core/task/Task.ts` — consecutive error tracking per tool
    pub fn record_tool_success(&mut self, tool_name: &str) {
        self.consecutive_errors.remove(tool_name);
    }

    /// Check whether the task should be aborted due to too many consecutive
    /// errors from a specific tool.
    ///
    /// Returns `true` when the consecutive error count for `tool_name` is
    /// greater than or equal to `max`.
    ///
    /// Source: `src/core/task/Task.ts` — consecutive error threshold check
    pub fn should_abort_on_errors(&self, tool_name: &str, max: u32) -> bool {
        if max == 0 {
            return false;
        }
        self.consecutive_errors
            .get(tool_name)
            .map_or(false, |&count| count >= max)
    }
}

impl Default for LoopControl {
    fn default() -> Self {
        // Default: allow up to 3 consecutive mistakes, no iteration limit
        Self::new(3)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_control_default() {
        let lc = LoopControl::default();
        assert_eq!(lc.consecutive_mistake_count, 0);
        assert_eq!(lc.max_consecutive_mistakes, 3);
        assert!(lc.max_iterations.is_none());
        assert_eq!(lc.current_iteration, 0);
        assert!(!lc.is_cancelled);
        assert!(!lc.did_tool_fail_in_current_turn);
        assert_eq!(lc.consecutive_no_tool_use_count, 0);
        assert_eq!(lc.consecutive_no_assistant_messages_count, 0);
        assert!(lc.consecutive_mistake_count_for_apply_diff.is_empty());
        assert!(lc.consecutive_mistake_count_for_edit_file.is_empty());
        assert!(!lc.mistake_grace_used);
        assert!(lc.consecutive_errors.is_empty());
    }

    #[test]
    fn test_loop_control_should_continue_initially() {
        let lc = LoopControl::new(3);
        assert!(lc.should_continue());
    }

    #[test]
    fn test_loop_control_should_stop_on_cancel() {
        let mut lc = LoopControl::new(3);
        lc.cancel();
        assert!(!lc.should_continue());
    }

    #[test]
    fn test_loop_control_record_mistake_under_limit() {
        let mut lc = LoopControl::new(3);
        let exceeded = lc.record_mistake();
        assert!(!exceeded);
        assert_eq!(lc.consecutive_mistake_count, 1);
        assert!(lc.did_tool_fail_in_current_turn);
    }

    #[test]
    fn test_loop_control_record_mistake_at_limit() {
        let mut lc = LoopControl::new(3);
        lc.record_mistake(); // 1
        lc.record_mistake(); // 2
        let exceeded = lc.record_mistake(); // 3 — count == limit
        assert!(exceeded); // 3 >= 3 is true (FIXED: was false with >)
        assert!(!lc.should_continue()); // 3 >= 3 triggers stop
    }

    #[test]
    fn test_loop_control_mistake_limit_boundary() {
        // With limit=3, should stop at exactly 3 mistakes (>= comparison)
        let mut lc = LoopControl::new(3);
        assert!(lc.should_continue()); // 0 mistakes

        lc.record_mistake(); // 1
        assert!(lc.should_continue()); // 1 < 3

        lc.record_mistake(); // 2
        assert!(lc.should_continue()); // 2 < 3

        lc.record_mistake(); // 3
        assert!(!lc.should_continue()); // 3 >= 3 → STOP (FIXED)
    }

    #[test]
    fn test_loop_control_mistake_limit_with_zero() {
        // limit=0 means no limit checking (matches TS: `if (this.consecutiveMistakeLimit > 0 && ...)`)
        let mut lc = LoopControl::new(0);
        lc.record_mistake();
        lc.record_mistake();
        lc.record_mistake();
        assert!(lc.should_continue()); // limit=0 disables check
    }

    #[test]
    fn test_loop_control_is_mistake_limit_reached() {
        let mut lc = LoopControl::new(3);
        assert!(!lc.is_mistake_limit_reached());
        lc.record_mistake(); // 1
        assert!(!lc.is_mistake_limit_reached());
        lc.record_mistake(); // 2
        assert!(!lc.is_mistake_limit_reached());
        lc.record_mistake(); // 3
        assert!(lc.is_mistake_limit_reached()); // 3 >= 3
    }

    #[test]
    fn test_loop_control_reset_mistake_count() {
        let mut lc = LoopControl::new(3);
        lc.record_mistake();
        lc.record_mistake();
        assert_eq!(lc.consecutive_mistake_count, 2);

        lc.reset_mistake_count();
        assert_eq!(lc.consecutive_mistake_count, 0);
        assert!(!lc.did_tool_fail_in_current_turn);
    }

    #[test]
    fn test_loop_control_increment_iteration_no_limit() {
        let mut lc = LoopControl::new(3);
        for _ in 0..100 {
            let reached = lc.increment_iteration();
            assert!(!reached);
        }
        assert_eq!(lc.current_iteration, 100);
    }

    #[test]
    fn test_loop_control_increment_iteration_with_limit() {
        let mut lc = LoopControl::with_max_iterations(3, 5);

        assert!(!lc.increment_iteration()); // 1
        assert!(!lc.increment_iteration()); // 2
        assert!(!lc.increment_iteration()); // 3
        assert!(!lc.increment_iteration()); // 4
        assert!(lc.increment_iteration());  // 5 = limit
    }

    #[test]
    fn test_loop_control_should_stop_at_max_iterations() {
        let mut lc = LoopControl::with_max_iterations(3, 2);
        lc.increment_iteration(); // 1
        lc.increment_iteration(); // 2
        assert!(!lc.should_continue()); // 2 >= 2
    }

    #[test]
    fn test_loop_control_remaining_iterations() {
        let mut lc = LoopControl::with_max_iterations(3, 10);
        assert_eq!(lc.remaining_iterations(), Some(10));
        lc.increment_iteration();
        assert_eq!(lc.remaining_iterations(), Some(9));
    }

    #[test]
    fn test_loop_control_remaining_iterations_no_limit() {
        let lc = LoopControl::new(3);
        assert_eq!(lc.remaining_iterations(), None);
    }

    #[test]
    fn test_loop_control_reset_turn() {
        let mut lc = LoopControl::new(3);
        lc.record_mistake();
        assert!(lc.did_tool_fail_in_current_turn);
        lc.reset_turn();
        assert!(!lc.did_tool_fail_in_current_turn);
        // Mistake count should NOT be reset
        assert_eq!(lc.consecutive_mistake_count, 1);
    }

    #[test]
    fn test_loop_control_consecutive_no_tool_use() {
        let mut lc = LoopControl::new(3);
        assert_eq!(lc.consecutive_no_tool_use_count, 0);

        lc.record_no_tool_use();
        assert_eq!(lc.consecutive_no_tool_use_count, 1);
        assert_eq!(lc.consecutive_mistake_count, 0); // count < 2, no mistake yet

        lc.record_no_tool_use();
        assert_eq!(lc.consecutive_no_tool_use_count, 2);
        assert_eq!(lc.consecutive_mistake_count, 1); // count >= 2, mistake recorded

        lc.reset_no_tool_use_count();
        assert_eq!(lc.consecutive_no_tool_use_count, 0);
    }

    #[test]
    fn test_loop_control_reset_no_tool_use_alias() {
        let mut lc = LoopControl::new(3);
        lc.record_no_tool_use();
        lc.record_no_tool_use();
        assert_eq!(lc.consecutive_no_tool_use_count, 2);

        lc.reset_no_tool_use(); // alias
        assert_eq!(lc.consecutive_no_tool_use_count, 0);
    }

    #[test]
    fn test_loop_control_per_file_apply_diff_mistakes() {
        let mut lc = LoopControl::new(3);

        let count = lc.record_apply_diff_mistake("/path/to/file.rs");
        assert_eq!(count, 1);

        let count = lc.record_apply_diff_mistake("/path/to/file.rs");
        assert_eq!(count, 2);

        // Different file should have its own count
        let count = lc.record_apply_diff_mistake("/path/to/other.rs");
        assert_eq!(count, 1);

        // Reset specific file
        lc.reset_apply_diff_mistake("/path/to/file.rs");
        assert_eq!(lc.consecutive_mistake_count_for_apply_diff.get("/path/to/file.rs"), None);
        assert_eq!(lc.consecutive_mistake_count_for_apply_diff.get("/path/to/other.rs"), Some(&1));
    }

    #[test]
    fn test_loop_control_per_file_edit_file_mistakes() {
        let mut lc = LoopControl::new(3);

        let count = lc.record_edit_file_mistake("/path/to/file.rs");
        assert_eq!(count, 1);

        let count = lc.record_edit_file_mistake("/path/to/file.rs");
        assert_eq!(count, 2);

        lc.reset_edit_file_mistake("/path/to/file.rs");
        assert_eq!(lc.consecutive_mistake_count_for_edit_file.get("/path/to/file.rs"), None);
    }

    #[test]
    fn test_loop_control_full_scenario() {
        let mut lc = LoopControl::with_max_iterations(3, 10);

        // Iteration 1: success
        assert!(lc.should_continue());
        lc.increment_iteration();
        lc.reset_mistake_count();

        // Iteration 2: mistake
        assert!(lc.should_continue());
        lc.increment_iteration();
        let exceeded = lc.record_mistake();
        assert!(!exceeded); // 1 < 3

        // Iteration 3: another mistake
        assert!(lc.should_continue());
        lc.increment_iteration();
        let exceeded = lc.record_mistake();
        assert!(!exceeded); // 2 < 3

        // Now at 2 mistakes, should still continue
        assert!(lc.should_continue());

        // Third mistake hits the limit
        let exceeded = lc.record_mistake();
        assert!(exceeded); // 3 >= 3 (FIXED)
        assert!(!lc.should_continue());
    }

    #[test]
    fn test_loop_control_cancel_idempotent() {
        let mut lc = LoopControl::new(3);
        lc.cancel();
        lc.cancel();
        assert!(lc.is_cancelled);
        assert!(!lc.should_continue());
    }

    #[test]
    fn test_loop_control_cancel_resets_counters() {
        // Source: `src/core/task/Task.ts` — abortTask resets counters
        let mut lc = LoopControl::new(3);
        lc.record_no_tool_use();
        lc.record_no_tool_use();
        lc.record_no_assistant_message();
        assert_eq!(lc.consecutive_no_tool_use_count, 2);
        assert_eq!(lc.consecutive_no_assistant_messages_count, 1);
        // record_no_tool_use() twice → consecutive_mistake_count = 1
        // record_no_assistant_message() once → no mistake increment (count < 2)
        assert_eq!(lc.consecutive_mistake_count, 1);

        lc.cancel();
        assert_eq!(lc.consecutive_no_tool_use_count, 0);
        assert_eq!(lc.consecutive_no_assistant_messages_count, 0);
    }

    #[test]
    fn test_loop_control_consecutive_no_assistant_messages() {
        let mut lc = LoopControl::new(3);
        assert_eq!(lc.consecutive_no_assistant_messages_count, 0);

        lc.record_no_assistant_message();
        assert_eq!(lc.consecutive_no_assistant_messages_count, 1);
        assert_eq!(lc.consecutive_mistake_count, 0); // count < 2, no mistake yet

        lc.record_no_assistant_message();
        assert_eq!(lc.consecutive_no_assistant_messages_count, 2);
        assert_eq!(lc.consecutive_mistake_count, 1); // count >= 2, mistake recorded

        lc.reset_no_assistant_messages_count();
        assert_eq!(lc.consecutive_no_assistant_messages_count, 0);
    }

    #[test]
    fn test_loop_control_reset_no_assistant_message_alias() {
        let mut lc = LoopControl::new(3);
        lc.record_no_assistant_message();
        lc.record_no_assistant_message();
        assert_eq!(lc.consecutive_no_assistant_messages_count, 2);

        lc.reset_no_assistant_message(); // alias
        assert_eq!(lc.consecutive_no_assistant_messages_count, 0);
    }

    #[test]
    fn test_loop_control_should_retry_empty_response() {
        let mut lc = LoopControl::new(3);
        assert!(lc.should_retry_empty_response()); // count = 0

        lc.record_no_assistant_message();
        assert!(lc.should_retry_empty_response()); // count = 1

        lc.record_no_assistant_message();
        assert!(!lc.should_retry_empty_response()); // count = 2
    }

    #[test]
    fn test_loop_control_no_tool_use_accumulates_mistakes() {
        let mut lc = LoopControl::new(3);
        // Three consecutive no-tool-use iterations
        lc.record_no_tool_use(); // count=1, mistakes=0
        lc.record_no_tool_use(); // count=2, mistakes=1
        lc.record_no_tool_use(); // count=3, mistakes=2
        assert_eq!(lc.consecutive_no_tool_use_count, 3);
        assert_eq!(lc.consecutive_mistake_count, 2);
        assert!(lc.should_continue()); // 2 < 3

        // One more would hit mistake limit
        lc.record_no_tool_use(); // count=4, mistakes=3
        assert_eq!(lc.consecutive_mistake_count, 3);
        assert!(!lc.should_continue()); // 3 >= 3
    }

    #[test]
    fn test_loop_control_try_use_mistake_grace_first_time() {
        let mut lc = LoopControl::new(3);
        // Reach the mistake limit
        lc.record_mistake(); // 1
        lc.record_mistake(); // 2
        lc.record_mistake(); // 3
        assert!(!lc.should_continue()); // 3 >= 3

        // Use grace: should reset mistakes and allow continuing
        let granted = lc.try_use_mistake_grace();
        assert!(granted);
        assert!(lc.mistake_grace_used);
        assert_eq!(lc.consecutive_mistake_count, 0);
        assert!(lc.should_continue()); // 0 < 3
    }

    #[test]
    fn test_loop_control_try_use_mistake_grace_already_used() {
        let mut lc = LoopControl::new(3);
        lc.record_mistake();
        lc.record_mistake();
        lc.record_mistake();

        // First grace: succeeds
        assert!(lc.try_use_mistake_grace());
        assert_eq!(lc.consecutive_mistake_count, 0);

        // Accumulate mistakes again
        lc.record_mistake(); // 1
        lc.record_mistake(); // 2
        lc.record_mistake(); // 3
        assert!(!lc.should_continue()); // 3 >= 3

        // Second grace: denied
        let granted = lc.try_use_mistake_grace();
        assert!(!granted);
        assert_eq!(lc.consecutive_mistake_count, 3);
        assert!(!lc.should_continue()); // still at limit
    }

    // ---- consecutive_errors tests ----

    #[test]
    fn test_consecutive_errors_record_and_check() {
        let mut lc = LoopControl::new(3);

        // No errors initially
        assert!(!lc.should_abort_on_errors("read_file", 3));

        // Record errors
        let count = lc.record_tool_error("read_file");
        assert_eq!(count, 1);
        assert!(!lc.should_abort_on_errors("read_file", 3));

        lc.record_tool_error("read_file"); // 2
        assert!(!lc.should_abort_on_errors("read_file", 3));

        lc.record_tool_error("read_file"); // 3
        assert!(lc.should_abort_on_errors("read_file", 3));
    }

    #[test]
    fn test_consecutive_errors_success_resets() {
        let mut lc = LoopControl::new(3);

        lc.record_tool_error("write_to_file");
        lc.record_tool_error("write_to_file");
        assert_eq!(*lc.consecutive_errors.get("write_to_file").unwrap(), 2);

        // Success resets the count
        lc.record_tool_success("write_to_file");
        assert!(lc.consecutive_errors.get("write_to_file").is_none());
        assert!(!lc.should_abort_on_errors("write_to_file", 2));
    }

    #[test]
    fn test_consecutive_errors_per_tool_independent() {
        let mut lc = LoopControl::new(3);

        lc.record_tool_error("read_file");
        lc.record_tool_error("read_file");
        lc.record_tool_error("write_to_file");

        // Each tool has its own count
        assert_eq!(*lc.consecutive_errors.get("read_file").unwrap(), 2);
        assert_eq!(*lc.consecutive_errors.get("write_to_file").unwrap(), 1);

        // Resetting one doesn't affect the other
        lc.record_tool_success("read_file");
        assert!(lc.consecutive_errors.get("read_file").is_none());
        assert_eq!(*lc.consecutive_errors.get("write_to_file").unwrap(), 1);
    }

    #[test]
    fn test_consecutive_errors_zero_max_never_aborts() {
        let mut lc = LoopControl::new(3);

        // Even with many errors, max=0 means never abort
        for _ in 0..100 {
            lc.record_tool_error("apply_diff");
        }
        assert!(!lc.should_abort_on_errors("apply_diff", 0));
    }
}
