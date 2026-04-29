//! History management utilities.
//!
//! Functions for filtering and retrieving conversation history,
//! including messages since last summary and effective API history.
//!
//! Source: `src/core/condense/index.ts` — `getMessagesSinceLastSummary`,
//! `getEffectiveApiHistory`

use roo_types::api::{ApiMessage, ContentBlock};

/// Returns the list of all messages since the last summary message, including
/// the summary itself. Returns all messages if there is no summary.
///
/// Note: Summary messages are always created with role: "user" (fresh-start model),
/// so the first message since the last summary is guaranteed to be a user message.
///
/// Source: `src/core/condense/index.ts` — `getMessagesSinceLastSummary`
pub fn get_messages_since_last_summary(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    // Find the index of the last summary message by iterating in reverse
    let last_summary_index = messages
        .iter()
        .rev()
        .position(|message| message.is_summary.unwrap_or(false));

    match last_summary_index {
        None => messages.to_vec(),
        Some(reverse_index) => {
            let index = messages.len() - reverse_index - 1;
            messages[index..].to_vec()
        }
    }
}

/// Filters the API conversation history to get the "effective" messages to send
/// to the API.
///
/// Fresh Start Model:
/// - When a summary exists, return only messages from the summary onwards (fresh start)
/// - Messages with a `condense_parent` pointing to an existing summary are filtered out
///
/// Messages with a `truncation_parent` that points to an existing truncation marker
/// are also filtered out.
///
/// Source: `src/core/condense/index.ts` — `getEffectiveApiHistory`
pub fn get_effective_api_history(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    // Find the most recent summary message
    let last_summary_index = messages
        .iter()
        .rev()
        .position(|msg| msg.is_summary.unwrap_or(false));

    if let Some(reverse_index) = last_summary_index {
        let summary_index = messages.len() - reverse_index - 1;
        let mut messages_from_summary = messages[summary_index..].to_vec();

        // Collect all tool_use IDs from assistant messages in the result
        let mut tool_use_ids = std::collections::HashSet::new();
        for msg in &messages_from_summary {
            if msg.role == roo_types::api::MessageRole::Assistant {
                for block in &msg.content {
                    if let ContentBlock::ToolUse { id, .. } = block {
                        tool_use_ids.insert(id.clone());
                    }
                }
            }
        }

        // Filter out orphan tool_result blocks from user messages
        messages_from_summary = messages_from_summary
            .into_iter()
            .map(|mut msg| {
                if msg.role == roo_types::api::MessageRole::User {
                    let original_len = msg.content.len();
                    msg.content.retain(|block| {
                        if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                            tool_use_ids.contains(tool_use_id)
                        } else {
                            true
                        }
                    });
                    if msg.content.is_empty() {
                        return None;
                    }
                    if msg.content.len() != original_len {
                        return Some(msg);
                    }
                }
                Some(msg)
            })
            .flatten()
            .collect();

        // Still need to filter out any truncated messages within this range
        let mut existing_truncation_ids = std::collections::HashSet::new();
        for msg in &messages_from_summary {
            if msg.is_truncation_marker.unwrap_or(false) {
                if let Some(ref truncation_id) = msg.truncation_id {
                    existing_truncation_ids.insert(truncation_id.clone());
                }
            }
        }

        return messages_from_summary
            .into_iter()
            .filter(|msg| {
                // Filter out truncated messages if their truncation marker exists
                if let Some(ref parent) = msg.truncation_parent {
                    if existing_truncation_ids.contains(parent) {
                        return false;
                    }
                }
                true
            })
            .collect();
    }

    // No summary - filter based on condense_parent and truncation_parent
    // This handles the case of orphaned condense_parent tags (summary was deleted via rewind)

    // Collect all condenseIds of summaries that exist in the current history
    let mut existing_summary_ids = std::collections::HashSet::new();
    // Collect all truncationIds of truncation markers that exist in the current history
    let mut existing_truncation_ids = std::collections::HashSet::new();

    for msg in messages {
        if msg.is_summary.unwrap_or(false) {
            if let Some(ref condense_id) = msg.condense_id {
                existing_summary_ids.insert(condense_id.clone());
            }
        }
        if msg.is_truncation_marker.unwrap_or(false) {
            if let Some(ref truncation_id) = msg.truncation_id {
                existing_truncation_ids.insert(truncation_id.clone());
            }
        }
    }

    // Filter out messages whose condense_parent points to an existing summary
    // or whose truncation_parent points to an existing truncation marker.
    // Messages with orphaned parents (summary/marker was deleted) are included.
    messages
        .iter()
        .filter(|msg| {
            // Filter out condensed messages if their summary exists
            if let Some(ref parent) = msg.condense_parent {
                if existing_summary_ids.contains(parent) {
                    return false;
                }
            }
            // Filter out truncated messages if their truncation marker exists
            if let Some(ref parent) = msg.truncation_parent {
                if existing_truncation_ids.contains(parent) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ContentBlock, MessageRole};

    fn make_message(role: MessageRole, text: &str) -> ApiMessage {
        ApiMessage {
            role,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning: None,
            ts: Some(1000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    fn make_summary_message(text: &str, condense_id: &str) -> ApiMessage {
        ApiMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning: None,
            reasoning_details: None,
            ts: Some(2000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: Some(true),
            condense_id: Some(condense_id.to_string()),
        }
    }

    #[test]
    fn test_get_messages_since_last_summary_no_summary() {
        let messages = vec![
            make_message(MessageRole::User, "hello"),
            make_message(MessageRole::Assistant, "hi"),
        ];
        let result = get_messages_since_last_summary(&messages);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_get_messages_since_last_summary_with_summary() {
        let messages = vec![
            make_message(MessageRole::User, "hello"),
            make_summary_message("summary", "condense_1"),
            make_message(MessageRole::Assistant, "hi"),
        ];
        let result = get_messages_since_last_summary(&messages);
        assert_eq!(result.len(), 2);
        assert!(result[0].is_summary.unwrap_or(false));
    }

    #[test]
    fn test_get_effective_api_history_no_summary() {
        let messages = vec![
            make_message(MessageRole::User, "hello"),
            make_message(MessageRole::Assistant, "hi"),
        ];
        let result = get_effective_api_history(&messages);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_get_effective_api_history_with_condensed_messages() {
        let mut msg1 = make_message(MessageRole::User, "hello");
        msg1.condense_parent = Some("condense_1".to_string());

        let summary = make_summary_message("summary", "condense_1");

        let msg3 = make_message(MessageRole::Assistant, "hi");

        let messages = vec![msg1, summary, msg3];
        let result = get_effective_api_history(&messages);
        // Fresh start: only summary + last message
        assert_eq!(result.len(), 2);
        assert!(result[0].is_summary.unwrap_or(false));
    }

    #[test]
    fn test_get_effective_api_history_with_truncated_messages() {
        let mut msg1 = make_message(MessageRole::User, "hello");
        msg1.truncation_parent = Some("trunc_1".to_string());

        let mut marker = make_message(MessageRole::User, "truncation marker");
        marker.is_truncation_marker = Some(true);
        marker.truncation_id = Some("trunc_1".to_string());

        let msg3 = make_message(MessageRole::Assistant, "hi");

        let messages = vec![msg1, marker, msg3];
        let result = get_effective_api_history(&messages);
        // msg1 filtered out because its truncation_parent matches marker's truncation_id
        assert_eq!(result.len(), 2);
    }
}
