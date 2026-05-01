//! Post-truncation cleanup logic.
//!
//! After API history is truncated, orphaned `condense_parent` / `truncation_parent`
//! tags may remain on messages whose target summary or truncation marker was deleted.
//! This module provides [`cleanup_after_truncation`] which clears those orphaned
//! references so the affected messages become active again.
//!
//! This is a faithful port of `cleanupAfterTruncation` from
//! `src/core/condense/index.ts`. The TS version only takes `messages: ApiMessage[]`
//! and only clears parent references — it does **not** remove orphaned Summary
//! messages or truncation markers (that is handled by the caller in
//! `truncateApiHistoryWithCleanup` steps 3–4).

use std::collections::HashSet;

use crate::types::ApiMessage;

/// Clear orphaned `condense_parent` and `truncation_parent` references after a
/// truncation operation (rewind / delete).
///
/// When a summary message or truncation marker is deleted, messages that were
/// tagged with its ID should have their parent reference cleared so they become
/// active again.
///
/// This function should be called after any operation that truncates the API
/// history to ensure messages are properly restored when their summary or
/// truncation marker is deleted.
///
/// # Algorithm (matches TS `cleanupAfterTruncation`)
///
/// 1. Collect all `condenseId` values from Summary messages that still exist.
/// 2. Collect all `truncationId` values from truncation markers that still exist.
/// 3. For each message, clear `condense_parent` if the target summary no longer
///    exists, and clear `truncation_parent` if the target marker no longer exists.
///
/// Source: `src/core/condense/index.ts` — `cleanupAfterTruncation`
pub fn cleanup_after_truncation(api_history: &[ApiMessage]) -> Vec<ApiMessage> {
    // Collect all condenseIds of summaries that still exist
    let existing_summary_ids: HashSet<String> = api_history
        .iter()
        .filter(|m| m.is_summary)
        .filter_map(|m| m.condense_id.clone())
        .collect();

    // Collect all truncationIds of truncation markers that still exist
    let existing_truncation_ids: HashSet<String> = api_history
        .iter()
        .filter(|m| m.is_truncation_marker)
        .filter_map(|m| m.truncation_id.clone())
        .collect();

    // Clear orphaned parent references for messages whose summary or
    // truncation marker was deleted
    api_history
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

/// Detect orphaned condense IDs — Summary messages whose `condense_context`
/// event is absent from the given set of active condense IDs.
///
/// This is a utility function not present in the TS source but useful for
/// callers that need to identify orphans.
pub fn find_orphaned_condense_ids<'a>(
    api_history: &'a [ApiMessage],
    active_condense_ids: &HashSet<String>,
) -> Vec<&'a str> {
    api_history
        .iter()
        .filter(|m| m.is_summary)
        .filter_map(|m| m.condense_id.as_deref())
        .filter(|id| !active_condense_ids.contains(*id))
        .collect()
}

/// Detect orphaned truncation IDs — truncation markers whose
/// `sliding_window_truncation` event is absent from the given set of active
/// truncation IDs.
///
/// This is a utility function not present in the TS source but useful for
/// callers that need to identify orphans.
pub fn find_orphaned_truncation_ids<'a>(
    api_history: &'a [ApiMessage],
    active_truncation_ids: &HashSet<String>,
) -> Vec<&'a str> {
    api_history
        .iter()
        .filter(|m| m.is_truncation_marker)
        .filter_map(|m| m.truncation_id.as_deref())
        .filter(|id| !active_truncation_ids.contains(*id))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- cleanup_after_truncation tests --------------------------------------

    #[test]
    fn no_orphans_no_changes() {
        let mut msg1 = ApiMessage::user(100);
        msg1.condense_parent = Some("condense_1".into());

        let summary = ApiMessage::summary(Some(150), "condense_1");

        let messages = vec![msg1, summary];
        let result = cleanup_after_truncation(&messages);
        // condense_parent should be kept because summary still exists
        assert_eq!(result[0].condense_parent.as_deref(), Some("condense_1"));
    }

    #[test]
    fn clears_orphaned_condense_parent() {
        let mut msg1 = ApiMessage::user(100);
        msg1.condense_parent = Some("condense_deleted".into());

        // No summary with condense_id "condense_deleted" exists
        let msg2 = ApiMessage::assistant(200);

        let messages = vec![msg1, msg2];
        let result = cleanup_after_truncation(&messages);
        // condense_parent should be cleared because summary was deleted
        assert!(result[0].condense_parent.is_none());
    }

    #[test]
    fn clears_orphaned_truncation_parent() {
        let mut msg1 = ApiMessage::user(100);
        msg1.truncation_parent = Some("trunc_deleted".into());

        // No truncation marker with truncation_id "trunc_deleted" exists
        let msg2 = ApiMessage::assistant(200);

        let messages = vec![msg1, msg2];
        let result = cleanup_after_truncation(&messages);
        // truncation_parent should be cleared because marker was deleted
        assert!(result[0].truncation_parent.is_none());
    }

    #[test]
    fn mixed_orphans() {
        let mut msg1 = ApiMessage::user(100);
        msg1.condense_parent = Some("condense_deleted".into());
        msg1.truncation_parent = Some("trunc_deleted".into());

        let mut msg2 = ApiMessage::user(200);
        msg2.condense_parent = Some("condense_exists".into());

        let summary = ApiMessage::summary(Some(250), "condense_exists");

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

    #[test]
    fn keeps_valid_truncation_parent() {
        let mut msg1 = ApiMessage::user(100);
        msg1.truncation_parent = Some("trunc_valid".into());

        let marker = ApiMessage::truncation_marker(Some(150), "trunc_valid");

        let messages = vec![msg1, marker];
        let result = cleanup_after_truncation(&messages);
        assert_eq!(result[0].truncation_parent.as_deref(), Some("trunc_valid"));
    }

    #[test]
    fn empty_input() {
        let result = cleanup_after_truncation(&[]);
        assert!(result.is_empty());
    }

    // -- find_orphaned_* tests -----------------------------------------------

    #[test]
    fn find_orphaned_condense_ids_works() {
        let api = vec![
            ApiMessage::summary(Some(100), "c1"),
            ApiMessage::summary(Some(200), "c2"),
        ];
        let active: HashSet<String> = ["c1".into()].into_iter().collect();

        let orphans = find_orphaned_condense_ids(&api, &active);
        assert_eq!(orphans, vec!["c2"]);
    }

    #[test]
    fn find_orphaned_truncation_ids_works() {
        let api = vec![
            ApiMessage::truncation_marker(Some(100), "t1"),
            ApiMessage::truncation_marker(Some(200), "t2"),
        ];
        let active: HashSet<String> = ["t1".into()].into_iter().collect();

        let orphans = find_orphaned_truncation_ids(&api, &active);
        assert_eq!(orphans, vec!["t2"]);
    }
}
