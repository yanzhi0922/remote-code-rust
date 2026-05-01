//! Tool repetition detection.
//!
//! Corresponds to `src/core/tools/ToolRepetitionDetector.ts`.

use serde_json::Value;

// ---------------------------------------------------------------------------
// ToolRepetitionDetector
// ---------------------------------------------------------------------------

/// Detects consecutive identical tool calls to prevent the AI from getting
/// stuck in a loop.
///
/// Source: `src/core/tools/ToolRepetitionDetector.ts`
#[derive(Debug, Clone)]
pub struct ToolRepetitionDetector {
    /// Maximum number of identical consecutive tool calls allowed.
    max_repetitions: usize,
    /// The serialized JSON of the previous tool call.
    previous_tool_call_json: Option<String>,
    /// Count of consecutive identical tool calls.
    consecutive_identical_count: usize,
}

impl ToolRepetitionDetector {
    /// Creates a new detector with the given repetition limit.
    ///
    /// A limit of 0 means unlimited (no detection).
    pub fn new(max_repetitions: usize) -> Self {
        Self {
            max_repetitions,
            previous_tool_call_json: None,
            consecutive_identical_count: 0,
        }
    }

    /// Creates a detector with the default limit of 3.
    pub fn with_default_limit() -> Self {
        Self::new(3)
    }

    /// Checks if the current tool call is identical to the previous one
    /// and records it. Returns `true` if execution should be allowed,
    /// `false` if the repetition limit has been reached.
    ///
    /// When the limit is reached, the detector resets itself to allow
    /// recovery if the user guides the AI past this point.
    pub fn check_and_record(&mut self, tool_name: &str, params: &Value) -> bool {
        let current_json = self.serialize(tool_name, params);

        if self.previous_tool_call_json.as_deref() == Some(&current_json) {
            self.consecutive_identical_count += 1;
        } else {
            self.consecutive_identical_count = 0;
            self.previous_tool_call_json = Some(current_json);
        }

        // Check if limit is reached (0 means unlimited)
        if self.max_repetitions > 0 && self.consecutive_identical_count >= self.max_repetitions {
            // Reset counters to allow recovery
            self.consecutive_identical_count = 0;
            self.previous_tool_call_json = None;

            tracing::warn!(
                "Tool repetition limit reached for tool: {}",
                tool_name
            );

            return false;
        }

        true
    }

    /// Checks if the given tool call would be a repetition without recording it.
    pub fn is_repeating(&self, tool_name: &str, params: &Value) -> bool {
        let current_json = self.serialize(tool_name, params);
        self.previous_tool_call_json.as_deref() == Some(&current_json)
            && self.consecutive_identical_count >= self.max_repetitions.saturating_sub(1)
    }

    /// Resets the detector state.
    pub fn reset(&mut self) {
        self.previous_tool_call_json = None;
        self.consecutive_identical_count = 0;
    }

    /// Serializes a tool call into a canonical JSON string for comparison.
    fn serialize(&self, tool_name: &str, params: &Value) -> String {
        let mut obj = serde_json::Map::new();
        obj.insert("name".into(), Value::String(tool_name.to_string()));
        obj.insert("params".into(), params.clone());
        // Sort keys for canonical representation
        let mut sorted = serde_json::Map::new();
        let mut keys: Vec<String> = obj.keys().cloned().collect();
        keys.sort();
        for key in &keys {
            sorted.insert(key.clone(), obj.remove(key).unwrap());
        }
        Value::Object(sorted).to_string()
    }

    /// Returns the current consecutive identical call count.
    pub fn consecutive_count(&self) -> usize {
        self.consecutive_identical_count
    }

    /// Returns the maximum repetitions allowed.
    pub fn max_repetitions(&self) -> usize {
        self.max_repetitions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_no_repetition_allows() {
        let mut detector = ToolRepetitionDetector::new(3);
        assert!(detector.check_and_record("read_file", &json!({"path": "test.rs"})));
    }

    #[test]
    fn test_repetition_below_limit_allows() {
        let mut detector = ToolRepetitionDetector::new(3);
        let params = json!({"path": "test.rs"});
        assert!(detector.check_and_record("read_file", &params));
        assert!(detector.check_and_record("read_file", &params));
        assert!(detector.check_and_record("read_file", &params));
    }

    #[test]
    fn test_repetition_at_limit_blocks() {
        let mut detector = ToolRepetitionDetector::new(3);
        let params = json!({"path": "test.rs"});
        assert!(detector.check_and_record("read_file", &params)); // count = 0
        assert!(detector.check_and_record("read_file", &params)); // count = 1
        assert!(detector.check_and_record("read_file", &params)); // count = 2
        assert!(!detector.check_and_record("read_file", &params)); // count = 3, blocked
    }

    #[test]
    fn test_different_tool_resets_counter() {
        let mut detector = ToolRepetitionDetector::new(3);
        let params = json!({"path": "test.rs"});
        assert!(detector.check_and_record("read_file", &params));
        assert!(detector.check_and_record("read_file", &params));
        // Different tool resets counter
        assert!(detector.check_and_record("write_to_file", &json!({"path": "other.rs"})));
        assert_eq!(detector.consecutive_count(), 0);
    }

    #[test]
    fn test_different_params_resets_counter() {
        let mut detector = ToolRepetitionDetector::new(3);
        assert!(detector.check_and_record("read_file", &json!({"path": "a.rs"})));
        assert!(detector.check_and_record("read_file", &json!({"path": "a.rs"})));
        // Different params resets counter
        assert!(detector.check_and_record("read_file", &json!({"path": "b.rs"})));
        assert_eq!(detector.consecutive_count(), 0);
    }

    #[test]
    fn test_zero_limit_means_unlimited() {
        let mut detector = ToolRepetitionDetector::new(0);
        let params = json!({"path": "test.rs"});
        for _ in 0..10 {
            assert!(detector.check_and_record("read_file", &params));
        }
    }

    #[test]
    fn test_reset_clears_state() {
        let mut detector = ToolRepetitionDetector::new(3);
        let params = json!({"path": "test.rs"});
        detector.check_and_record("read_file", &params);
        detector.check_and_record("read_file", &params);
        assert_eq!(detector.consecutive_count(), 1);
        detector.reset();
        assert_eq!(detector.consecutive_count(), 0);
        assert!(detector.previous_tool_call_json.is_none());
    }

    #[test]
    fn test_after_block_allows_again() {
        let mut detector = ToolRepetitionDetector::new(2);
        let params = json!({"path": "test.rs"});
        assert!(detector.check_and_record("read_file", &params)); // count = 0
        assert!(detector.check_and_record("read_file", &params)); // count = 1
        assert!(!detector.check_and_record("read_file", &params)); // blocked, resets
        // After block, should allow again (detector was reset)
        assert!(detector.check_and_record("read_file", &params));
    }

    #[test]
    fn test_is_repeating() {
        let mut detector = ToolRepetitionDetector::new(3);
        let params = json!({"path": "test.rs"});
        detector.check_and_record("read_file", &params);
        detector.check_and_record("read_file", &params);
        // After 2 identical calls (count=1, limit=3), not yet repeating
        assert!(!detector.is_repeating("read_file", &params));
    }

    #[test]
    fn test_with_default_limit() {
        let detector = ToolRepetitionDetector::with_default_limit();
        assert_eq!(detector.max_repetitions(), 3);
    }
}
