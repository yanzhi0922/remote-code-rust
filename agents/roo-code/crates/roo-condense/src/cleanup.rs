//! Post-truncation cleanup utilities.
//!
//! Cleans up orphaned `condense_parent` and `truncation_parent` references
//! after a truncation operation (rewind/delete).
//!
//! Source: `src/core/condense/index.ts` — `cleanupAfterTruncation`

use roo_types::api::ApiMessage;

/// Cleans up orphaned `condense_parent` and `truncation_parent` references
/// after a truncation operation (rewind/delete).
///
/// When a summary message or truncation marker is deleted, messages that were
/// tagged with its ID should have their parent reference cleared so they become
/// active again.
///
/// This function should be called after any operation that truncates the API
/// history to ensure messages are properly restored when their summary or
/// truncation marker is deleted.
///
/// Source: `src/core/condense/index.ts` — `cleanupAfterTruncation`
pub fn cleanup_after_truncation(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    // Collect all condenseIds of summaries that still exist
    let mut existing_summary_ids = std::collections::HashSet::new();
    // Collect all truncationIds of truncation markers that still exist
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

    // Clear orphaned parent references for messages whose summary or
    // truncation marker was deleted
    messages
        .iter()
        .map(|msg| {
            let mut needs_update = false;

            // Check for orphaned condense_parent
            if let Some(ref parent) = msg.condense_parent {
                if !existing_summary_ids.contains(parent) {
                    needs_update = true;
                }
            }

            // Check for orphaned truncation_parent
            if let Some(ref parent) = msg.truncation_parent {
                if !existing_truncation_ids.contains(parent) {
                    needs_update = true;
                }
            }

            if needs_update {
                let mut result = msg.clone();

                // Keep condense_parent only if its summary still exists
                if let Some(ref parent) = msg.condense_parent {
                    if !existing_summary_ids.contains(parent) {
                        result.condense_parent = None;
                    }
                }

                // Keep truncation_parent only if its truncation marker still exists
                if let Some(ref parent) = msg.truncation_parent {
                    if !existing_truncation_ids.contains(parent) {
                        result.truncation_parent = None;
                    }
                }

                result
            } else {
                msg.clone()
            }
        })
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

    #[test]
    fn test_cleanup_no_orphans() {
        let mut msg1 = make_message(MessageRole::User, "hello");
        msg1.condense_parent = Some("condense_1".to_string());

        let mut summary = make_message(MessageRole::User, "summary");
        summary.is_summary = Some(true);
        summary.condense_id = Some("condense_1".to_string());

        let messages = vec![msg1, summary];
        let result = cleanup_after_truncation(&messages);
        // condense_parent should be kept because summary still exists
        assert!(result[0].condense_parent.is_some());
    }

    #[test]
    fn test_cleanup_orphaned_condense_parent() {
        let mut msg1 = make_message(MessageRole::User, "hello");
        msg1.condense_parent = Some("condense_1".to_string());

        // No summary with condense_id "condense_1" exists
        let msg2 = make_message(MessageRole::Assistant, "hi");

        let messages = vec![msg1, msg2];
        let result = cleanup_after_truncation(&messages);
        // condense_parent should be cleared because summary was deleted
        assert!(result[0].condense_parent.is_none());
    }

    #[test]
    fn test_cleanup_orphaned_truncation_parent() {
        let mut msg1 = make_message(MessageRole::User, "hello");
        msg1.truncation_parent = Some("trunc_1".to_string());

        // No truncation marker with truncation_id "trunc_1" exists
        let msg2 = make_message(MessageRole::Assistant, "hi");

        let messages = vec![msg1, msg2];
        let result = cleanup_after_truncation(&messages);
        // truncation_parent should be cleared because marker was deleted
        assert!(result[0].truncation_parent.is_none());
    }

    #[test]
    fn test_cleanup_mixed_orphans() {
        let mut msg1 = make_message(MessageRole::User, "hello");
        msg1.condense_parent = Some("condense_deleted".to_string());
        msg1.truncation_parent = Some("trunc_deleted".to_string());

        let mut msg2 = make_message(MessageRole::User, "world");
        msg2.condense_parent = Some("condense_exists".to_string());

        let mut summary = make_message(MessageRole::User, "summary");
        summary.is_summary = Some(true);
        summary.condense_id = Some("condense_exists".to_string());

        let messages = vec![msg1, msg2, summary];
        let result = cleanup_after_truncation(&messages);
        // msg1: both parents cleared (orphaned)
        assert!(result[0].condense_parent.is_none());
        assert!(result[0].truncation_parent.is_none());
        // msg2: condense_parent kept (summary exists)
        assert_eq!(
            result[1].condense_parent.as_deref(),
            Some("condense_exists")
        );
    }
}
