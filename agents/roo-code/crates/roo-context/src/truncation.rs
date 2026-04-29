//! Sliding window truncation.
//!
//! Truncates a conversation by tagging messages as hidden instead of removing them,
//! implementing non-destructive sliding window truncation that allows messages to be
//! restored if the user rewinds past the truncation point.
//!
//! Source: `src/core/context-management/index.ts` — `truncateConversation`

use roo_types::api::{ApiMessage, ContentBlock, MessageRole};

/// Result of a truncation operation.
///
/// Source: `src/core/context-management/index.ts` — `TruncationResult`
#[derive(Debug, Clone)]
pub struct TruncationResult {
    /// The messages after truncation (with tagged hidden messages and marker inserted).
    pub messages: Vec<ApiMessage>,
    /// The unique ID for this truncation operation.
    pub truncation_id: String,
    /// The number of messages that were hidden.
    pub messages_removed: usize,
}

/// Truncates a conversation by tagging messages as hidden instead of removing them.
///
/// The first message is always retained, and a specified fraction (rounded to an even
/// number) of messages from the beginning (excluding the first) is tagged with
/// `truncation_parent`. A truncation marker is inserted to track where truncation
/// occurred.
///
/// This implements non-destructive sliding window truncation, allowing messages to be
/// restored if the user rewinds past the truncation point.
///
/// # Arguments
/// * `messages` - The conversation messages
/// * `frac_to_remove` - The fraction (between 0 and 1) of messages (excluding the first)
///   to hide
/// * `task_id` - The task ID for telemetry
///
/// Source: `src/core/context-management/index.ts` — `truncateConversation`
pub fn truncate_conversation(
    messages: &[ApiMessage],
    frac_to_remove: f64,
    task_id: &str,
) -> TruncationResult {
    // H5: Capture telemetry for sliding window truncation
    let telemetry = roo_telemetry::service::TelemetryService::new();
    telemetry.capture_sliding_window_truncation(task_id);

    let truncation_id = uuid::Uuid::now_v7().to_string();

    // Filter to only visible messages (those not already truncated)
    // We need to track original indices to correctly tag messages in the full array
    let visible_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter_map(|(index, msg)| {
            let is_truncated = msg.truncation_parent.is_some();
            let is_marker = msg.is_truncation_marker.unwrap_or(false);
            if !is_truncated && !is_marker {
                Some(index)
            } else {
                None
            }
        })
        .collect();

    // Calculate how many visible messages to truncate (excluding first visible message)
    let visible_count = visible_indices.len();
    let raw_messages_to_remove = ((visible_count - 1) as f64 * frac_to_remove).floor() as usize;
    // Round down to even number to keep user/assistant pairs intact
    let messages_to_remove = raw_messages_to_remove - (raw_messages_to_remove % 2);

    if messages_to_remove == 0 {
        // Nothing to truncate
        return TruncationResult {
            messages: messages.to_vec(),
            truncation_id,
            messages_removed: 0,
        };
    }

    // Get the indices of visible messages to truncate (skip first visible, take next N)
    let indices_to_truncate: std::collections::HashSet<usize> =
        visible_indices[1..messages_to_remove + 1].iter().copied().collect();

    // Tag messages that are being "truncated" (hidden from API calls)
    let mut tagged_messages: Vec<ApiMessage> = messages
        .iter()
        .enumerate()
        .map(|(index, msg)| {
            if indices_to_truncate.contains(&index) {
                let mut tagged = msg.clone();
                tagged.truncation_parent = Some(truncation_id.clone());
                tagged
            } else {
                msg.clone()
            }
        })
        .collect();

    // Find the actual boundary - the index right after the last truncated message
    // If all visible messages except the first are truncated, insert marker at the end
    let first_kept_visible_index = visible_indices
        .get(messages_to_remove + 1)
        .copied()
        .unwrap_or(tagged_messages.len());

    // Insert truncation marker at the boundary position
    let first_kept_ts = messages
        .get(first_kept_visible_index)
        .and_then(|m| m.ts)
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as f64
        });

    let truncation_marker = ApiMessage {
        role: MessageRole::User,
        content: vec![ContentBlock::Text {
            text: format!(
                "[Sliding window truncation: {messages_to_remove} messages hidden to reduce context]"
            ),
        }],
        ts: Some(first_kept_ts - 1.0),
        reasoning: None,
        truncation_parent: None,
        is_truncation_marker: Some(true),
        truncation_id: Some(truncation_id.clone()),
        condense_parent: None,
        is_summary: None,
        condense_id: None,
            reasoning_details: None,
    };

    // Insert marker at the boundary position
    let insert_position = first_kept_visible_index;
    let mut result = Vec::with_capacity(tagged_messages.len() + 1);
    result.extend(tagged_messages.drain(..insert_position));
    result.push(truncation_marker);
    result.extend(tagged_messages);

    TruncationResult {
        messages: result,
        truncation_id,
        messages_removed: messages_to_remove,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ContentBlock, MessageRole};

    fn make_message(role: MessageRole, text: &str, ts: f64) -> ApiMessage {
        ApiMessage {
            role,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning: None,
            ts: Some(ts),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    #[test]
    fn test_truncate_conversation_basic() {
        // 6 messages: user, assistant, user, assistant, user, assistant
        let messages = vec![
            make_message(MessageRole::User, "msg1", 1000.0),
            make_message(MessageRole::Assistant, "msg2", 1001.0),
            make_message(MessageRole::User, "msg3", 1002.0),
            make_message(MessageRole::Assistant, "msg4", 1003.0),
            make_message(MessageRole::User, "msg5", 1004.0),
            make_message(MessageRole::Assistant, "msg6", 1005.0),
        ];

        let result = truncate_conversation(&messages, 0.5, "task_1");

        // frac_to_remove = 0.5, visible_count = 6
        // raw_messages_to_remove = floor((6-1) * 0.5) = floor(2.5) = 2
        // messages_to_remove = 2 - (2 % 2) = 2 (already even)
        assert_eq!(result.messages_removed, 2);

        // Should have original 6 + 1 marker = 7 messages
        assert_eq!(result.messages.len(), 7);

        // First message should NOT be truncated
        assert!(result.messages[0].truncation_parent.is_none());

        // Messages 1 and 2 (indices 1,2) should be truncated
        assert!(result.messages[1].truncation_parent.is_some());
        assert!(result.messages[2].truncation_parent.is_some());

        // Find the truncation marker
        let marker_idx = result
            .messages
            .iter()
            .position(|m| m.is_truncation_marker.unwrap_or(false));
        assert!(marker_idx.is_some());
    }

    #[test]
    fn test_truncate_conversation_too_few_messages() {
        let messages = vec![
            make_message(MessageRole::User, "msg1", 1000.0),
            make_message(MessageRole::Assistant, "msg2", 1001.0),
        ];

        let result = truncate_conversation(&messages, 0.5, "task_1");

        // visible_count = 2, raw = floor(1 * 0.5) = 0, messages_to_remove = 0
        assert_eq!(result.messages_removed, 0);
        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn test_truncate_preserves_first_message() {
        let messages = vec![
            make_message(MessageRole::User, "first", 1000.0),
            make_message(MessageRole::Assistant, "second", 1001.0),
            make_message(MessageRole::User, "third", 1002.0),
            make_message(MessageRole::Assistant, "fourth", 1003.0),
        ];

        let result = truncate_conversation(&messages, 0.5, "task_1");

        // First message should never be truncated
        assert!(result.messages[0].truncation_parent.is_none());
        assert!(matches!(
            &result.messages[0].content[0],
            ContentBlock::Text { text } if text == "first"
        ));
    }

    #[test]
    fn test_truncate_even_alignment() {
        // 5 messages: frac_to_remove = 0.5
        // visible_count = 5, raw = floor(4 * 0.5) = 2, messages_to_remove = 2
        let messages = vec![
            make_message(MessageRole::User, "msg1", 1000.0),
            make_message(MessageRole::Assistant, "msg2", 1001.0),
            make_message(MessageRole::User, "msg3", 1002.0),
            make_message(MessageRole::Assistant, "msg4", 1003.0),
            make_message(MessageRole::User, "msg5", 1004.0),
        ];

        let result = truncate_conversation(&messages, 0.5, "task_1");
        // 2 messages removed (even alignment)
        assert_eq!(result.messages_removed, 2);
    }

    #[test]
    fn test_truncate_skips_already_truncated() {
        let mut msg1 = make_message(MessageRole::User, "msg1", 1000.0);
        msg1.truncation_parent = Some("old_truncation".to_string());

        let messages = vec![
            msg1,
            make_message(MessageRole::Assistant, "msg2", 1001.0),
            make_message(MessageRole::User, "msg3", 1002.0),
            make_message(MessageRole::Assistant, "msg4", 1003.0),
        ];

        let result = truncate_conversation(&messages, 0.5, "task_1");

        // Only 3 visible messages (msg1 is already truncated)
        // visible_count = 3, raw = floor(2 * 0.5) = 1, messages_to_remove = 0 (rounded to even)
        assert_eq!(result.messages_removed, 0);
    }
}
