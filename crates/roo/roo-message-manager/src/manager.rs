//! Core [`MessageManager`] implementation.
//!
//! `MessageManager` provides centralized handling for all conversation rewind
//! operations. It operates on in-memory `Vec<ClineMessage>` and
//! `Vec<ApiMessage>` and returns the modified lists, keeping the caller in
//! full control of persistence.
//!
//! Ported from `core/message-manager/index.ts`.

use std::collections::HashSet;

use crate::artifact::compute_valid_ids;
use crate::cleanup::cleanup_after_truncation;
use crate::types::{ApiMessage, ClineMessage, ContextEventIds, RewindOptions};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by [`MessageManager`].
#[derive(Debug, thiserror::Error)]
pub enum MessageManagerError {
    /// No cline message with the requested timestamp was found.
    #[error("Message with timestamp {ts} not found in clineMessages")]
    TimestampNotFound {
        ts: i64,
    },

    /// The requested index is out of bounds.
    #[error("Index {index} is out of bounds (len={len})")]
    IndexOutOfBounds {
        index: usize,
        len: usize,
    },
}

// ---------------------------------------------------------------------------
// RewindResult
// ---------------------------------------------------------------------------

/// The outcome of a rewind operation.
#[derive(Debug, Clone)]
pub struct RewindResult {
    /// Cline messages after truncation.
    pub cline_messages: Vec<ClineMessage>,

    /// API history after truncation + cleanup.
    pub api_history: Vec<ApiMessage>,

    /// IDs of context events that were removed.
    pub removed_context_event_ids: ContextEventIds,

    /// Set of artifact IDs that are still valid after the rewind.
    pub valid_artifact_ids: HashSet<String>,
}

// ---------------------------------------------------------------------------
// MessageManager
// ---------------------------------------------------------------------------

/// Centralized manager for conversation rewind operations.
///
/// Unlike the TypeScript version (which holds a `&Task` reference and mutates
/// state directly), this Rust implementation is **pure**: it borrows the
/// current message lists and returns new ones, leaving persistence to the
/// caller.
///
/// # TS Source Mapping
///
/// | TS Method                          | Rust Method                        |
/// |------------------------------------|------------------------------------|
/// | `rewindToTimestamp`                | [`rewind_to_timestamp`]            |
/// | `rewindToIndex`                    | [`rewind_to_index`]                |
/// | `performRewind`                    | [`perform_rewind`]                 |
/// | `collectRemovedContextEventIds`    | [`collect_removed_context_event_ids`] |
/// | `truncateClineMessages`            | inline in `perform_rewind`         |
/// | `truncateApiHistoryWithCleanup`    | [`truncate_api_history_with_cleanup`] |
/// | `cleanupOrphanedArtifacts`         | handled via [`compute_valid_ids`]  |
pub struct MessageManager;

impl MessageManager {
    /// Create a new `MessageManager`.
    pub fn new() -> Self {
        Self
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Rewind conversation to a specific timestamp.
    ///
    /// This is the **single entry point** for all message deletion operations.
    ///
    /// # Errors
    ///
    /// Returns [`MessageManagerError::TimestampNotFound`] if no cline message
    /// with the given timestamp exists.
    pub fn rewind_to_timestamp(
        &self,
        cline_messages: &[ClineMessage],
        api_history: &[ApiMessage],
        ts: i64,
        options: RewindOptions,
    ) -> Result<RewindResult, MessageManagerError> {
        // Find the index in clineMessages
        let cline_index = cline_messages
            .iter()
            .position(|m| m.ts == ts)
            .ok_or(MessageManagerError::TimestampNotFound { ts })?;

        // Calculate the actual cutoff index
        let cutoff_index = if options.include_target_message {
            cline_index + 1
        } else {
            cline_index
        };

        Ok(self.perform_rewind(cline_messages, api_history, cutoff_index, ts, options))
    }

    /// Rewind conversation to a specific index in clineMessages.
    ///
    /// Keeps messages `[0, to_index)` and removes `[to_index, end)`.
    ///
    /// # Errors
    ///
    /// Returns [`MessageManagerError::IndexOutOfBounds`] if `to_index` exceeds
    /// the length of `cline_messages`.
    pub fn rewind_to_index(
        &self,
        cline_messages: &[ClineMessage],
        api_history: &[ApiMessage],
        to_index: usize,
        options: RewindOptions,
    ) -> Result<RewindResult, MessageManagerError> {
        if to_index > cline_messages.len() {
            return Err(MessageManagerError::IndexOutOfBounds {
                index: to_index,
                len: cline_messages.len(),
            });
        }

        let cutoff_ts = cline_messages
            .get(to_index)
            .map(|m| m.ts)
            .unwrap_or(0);

        Ok(self.perform_rewind(cline_messages, api_history, to_index, cutoff_ts, options))
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Internal method that performs the actual rewind operation.
    ///
    /// Maps to TS `performRewind`.
    fn perform_rewind(
        &self,
        cline_messages: &[ClineMessage],
        api_history: &[ApiMessage],
        to_index: usize,
        cutoff_ts: i64,
        options: RewindOptions,
    ) -> RewindResult {
        // Step 1: Collect context event IDs from messages being removed
        let removed_ids = Self::collect_removed_context_event_ids(cline_messages, to_index);

        // Step 2: Truncate clineMessages (matches TS `truncateClineMessages`)
        let new_cline = cline_messages[..to_index].to_vec();

        // Step 3: Truncate and clean API history (matches TS `truncateApiHistoryWithCleanup`)
        let new_api = Self::truncate_api_history_with_cleanup(
            api_history,
            &new_cline,
            cutoff_ts,
            &removed_ids,
            options.skip_cleanup,
        );

        // Step 4: Compute valid artifact IDs (matches TS `cleanupOrphanedArtifacts`)
        let valid_artifact_ids = compute_valid_ids(&new_cline, &new_api);

        RewindResult {
            cline_messages: new_cline,
            api_history: new_api,
            removed_context_event_ids: removed_ids,
            valid_artifact_ids,
        }
    }

    /// Collect `condenseId` and `truncationId` values from context-management
    /// events that will be removed during the rewind.
    ///
    /// Maps to TS `collectRemovedContextEventIds`.
    pub fn collect_removed_context_event_ids(
        cline_messages: &[ClineMessage],
        from_index: usize,
    ) -> ContextEventIds {
        let mut condense_ids = HashSet::new();
        let mut truncation_ids = HashSet::new();

        for msg in cline_messages.iter().skip(from_index) {
            if msg.say == "condense_context" {
                if let Some(ref info) = msg.context_condense {
                    condense_ids.insert(info.condense_id.clone());
                }
            }

            if msg.say == "sliding_window_truncation" {
                if let Some(ref info) = msg.context_truncation {
                    truncation_ids.insert(info.truncation_id.clone());
                }
            }
        }

        ContextEventIds {
            condense_ids,
            truncation_ids,
        }
    }

    /// Truncate API history by timestamp, remove orphaned summaries/markers,
    /// and clean up orphaned tags.
    ///
    /// Maps to TS `truncateApiHistoryWithCleanup`.
    ///
    /// # Timestamp race handling
    ///
    /// Due to async execution during streaming, `clineMessage` timestamps may
    /// not perfectly align with API message timestamps. If there is no exact
    /// match but there are earlier messages, we find the first API user message
    /// at or after the cutoff and use its timestamp as the actual boundary.
    ///
    /// # Steps (matching TS source)
    ///
    /// 1. Determine the actual cutoff timestamp
    /// 2. Filter by the actual cutoff timestamp
    /// 3. Remove Summaries whose `condense_context` was removed
    /// 4. Remove truncation markers whose event was removed
    /// 5. Cleanup orphaned tags via [`cleanup_after_truncation`]
    pub fn truncate_api_history_with_cleanup(
        api_history: &[ApiMessage],
        _cline_messages: &[ClineMessage],
        cutoff_ts: i64,
        removed_ids: &ContextEventIds,
        skip_cleanup: bool,
    ) -> Vec<ApiMessage> {
        let mut api: Vec<ApiMessage> = api_history.to_vec();

        // Step 1: Determine the actual cutoff timestamp
        let has_exact_match = api.iter().any(|m| m.ts == Some(cutoff_ts));
        let has_msg_before_cutoff = api.iter().any(|m| m.ts.is_some_and(|ts| ts < cutoff_ts));

        let actual_cutoff = if !has_exact_match && has_msg_before_cutoff {
            // Race condition: find the first user message at or after cutoff
            let first_user_idx = api
                .iter()
                .position(|m| m.ts.is_some_and(|ts| ts >= cutoff_ts) && m.role == "user");

            match first_user_idx {
                Some(idx) => api[idx].ts.unwrap_or(cutoff_ts),
                None => cutoff_ts,
            }
        } else {
            cutoff_ts
        };

        // Step 2: Filter by the actual cutoff timestamp
        api.retain(|m| m.ts.is_none_or(|ts| ts < actual_cutoff));

        // Step 3: Remove Summaries whose condense_context was removed
        if !removed_ids.condense_ids.is_empty() {
            api.retain(|msg| {
                if msg.is_summary {
                    if let Some(ref condense_id) = msg.condense_id {
                        if removed_ids.condense_ids.contains(condense_id) {
                            return false;
                        }
                    }
                }
                true
            });
        }

        // Step 4: Remove truncation markers whose event was removed
        if !removed_ids.truncation_ids.is_empty() {
            api.retain(|msg| {
                if msg.is_truncation_marker {
                    if let Some(ref truncation_id) = msg.truncation_id {
                        if removed_ids.truncation_ids.contains(truncation_id) {
                            return false;
                        }
                    }
                }
                true
            });
        }

        // Step 5: Cleanup orphaned tags (unless skipped)
        // Matches TS: `cleanupAfterTruncation(apiHistory)` — only clears parent refs
        if !skip_cleanup {
            api = cleanup_after_truncation(&api);
        }

        api
    }
}

impl Default for MessageManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> MessageManager {
        MessageManager::new()
    }

    // -- rewind_to_timestamp tests ------------------------------------------

    #[test]
    fn rewind_to_timestamp_basic() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::new(200, "assistant"),
            ClineMessage::new(300, "user_feedback"),
            ClineMessage::new(400, "assistant"),
        ];
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::assistant(200),
            ApiMessage::user(300),
            ApiMessage::assistant(400),
        ];

        let mgr = make_manager();
        let result = mgr
            .rewind_to_timestamp(&cline, &api, 300, RewindOptions::default())
            .unwrap();

        assert_eq!(result.cline_messages.len(), 2);
        assert_eq!(result.cline_messages[0].ts, 100);
        assert_eq!(result.cline_messages[1].ts, 200);
        // API messages with ts < 300 are kept
        assert!(result.api_history.iter().all(|m| m.ts.unwrap_or(0) < 300));
    }

    #[test]
    fn rewind_to_timestamp_include_target() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::new(200, "assistant"),
            ClineMessage::new(300, "user_feedback"),
        ];
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::assistant(200),
            ApiMessage::user(300),
        ];

        let mgr = make_manager();
        let result = mgr
            .rewind_to_timestamp(
                &cline,
                &api,
                200,
                RewindOptions {
                    include_target_message: true,
                    skip_cleanup: false,
                },
            )
            .unwrap();

        // include_target_message=true → cutoff_index = index+1 = 2
        // keeps only [0, 2) = first two messages
        assert_eq!(result.cline_messages.len(), 2);
        assert_eq!(result.cline_messages[0].ts, 100);
        assert_eq!(result.cline_messages[1].ts, 200);
    }

    #[test]
    fn rewind_to_timestamp_not_found() {
        let cline = vec![ClineMessage::new(100, "user_feedback")];
        let api = vec![ApiMessage::user(100)];

        let mgr = make_manager();
        let err = mgr
            .rewind_to_timestamp(&cline, &api, 999, RewindOptions::default())
            .unwrap_err();

        match err {
            MessageManagerError::TimestampNotFound { ts } => assert_eq!(ts, 999),
            _ => panic!("Expected TimestampNotFound error"),
        }
    }

    // -- rewind_to_index tests ----------------------------------------------

    #[test]
    fn rewind_to_index_basic() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::new(200, "assistant"),
            ClineMessage::new(300, "user_feedback"),
        ];
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::assistant(200),
            ApiMessage::user(300),
        ];

        let mgr = make_manager();
        let result = mgr
            .rewind_to_index(&cline, &api, 2, RewindOptions::default())
            .unwrap();

        assert_eq!(result.cline_messages.len(), 2);
        assert_eq!(result.cline_messages[0].ts, 100);
        assert_eq!(result.cline_messages[1].ts, 200);
    }

    #[test]
    fn rewind_to_index_out_of_bounds() {
        let cline = vec![ClineMessage::new(100, "user_feedback")];
        let api = vec![ApiMessage::user(100)];

        let mgr = make_manager();
        let err = mgr
            .rewind_to_index(&cline, &api, 5, RewindOptions::default())
            .unwrap_err();

        match err {
            MessageManagerError::IndexOutOfBounds { index, len } => {
                assert_eq!(index, 5);
                assert_eq!(len, 1);
            }
            _ => panic!("Expected IndexOutOfBounds error"),
        }
    }

    #[test]
    fn rewind_to_index_zero() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::new(200, "assistant"),
        ];
        let api = vec![ApiMessage::user(100), ApiMessage::assistant(200)];

        let mgr = make_manager();
        let result = mgr
            .rewind_to_index(&cline, &api, 0, RewindOptions::default())
            .unwrap();

        assert!(result.cline_messages.is_empty());
        assert!(result.api_history.is_empty());
    }

    // -- empty list handling ------------------------------------------------

    #[test]
    fn empty_cline_messages() {
        let cline: Vec<ClineMessage> = vec![];
        let api: Vec<ApiMessage> = vec![];

        let mgr = make_manager();
        let result = mgr
            .rewind_to_index(&cline, &api, 0, RewindOptions::default())
            .unwrap();

        assert!(result.cline_messages.is_empty());
        assert!(result.api_history.is_empty());
    }

    // -- collect_removed_context_event_ids tests -----------------------------

    #[test]
    fn collect_removed_ids_finds_condense() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::condense(200, "c1"),
            ClineMessage::condense(300, "c2"),
        ];

        let ids = MessageManager::collect_removed_context_event_ids(&cline, 1);
        assert!(ids.condense_ids.contains("c1"));
        assert!(ids.condense_ids.contains("c2"));
        assert!(ids.truncation_ids.is_empty());
    }

    #[test]
    fn collect_removed_ids_finds_truncation() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::truncation(200, "t1"),
        ];

        let ids = MessageManager::collect_removed_context_event_ids(&cline, 1);
        assert!(ids.truncation_ids.contains("t1"));
        assert!(ids.condense_ids.is_empty());
    }

    #[test]
    fn collect_removed_ids_from_end() {
        let cline = vec![
            ClineMessage::condense(100, "c1"),
            ClineMessage::truncation(200, "t1"),
        ];

        let ids = MessageManager::collect_removed_context_event_ids(&cline, 0);
        assert!(ids.condense_ids.contains("c1"));
        assert!(ids.truncation_ids.contains("t1"));
    }

    // -- truncate_api_history_with_cleanup tests -----------------------------

    #[test]
    fn truncate_api_removes_orphaned_summary() {
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::summary(Some(150), "condense-1"),
            ApiMessage::assistant(200),
        ];
        let cline = vec![ClineMessage::new(100, "user_feedback")];
        let removed = ContextEventIds {
            condense_ids: HashSet::from(["condense-1".into()]),
            truncation_ids: HashSet::new(),
        };

        let result = MessageManager::truncate_api_history_with_cleanup(
            &api, &cline, 300, &removed, false,
        );

        // Summary with condense-1 should be removed (it was in removed_ids)
        assert!(!result.iter().any(|m| m.is_summary));
    }

    #[test]
    fn truncate_api_removes_orphaned_truncation_marker() {
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::truncation_marker(Some(150), "trunc-1"),
            ApiMessage::assistant(200),
        ];
        let cline = vec![ClineMessage::new(100, "user_feedback")];
        let removed = ContextEventIds {
            condense_ids: HashSet::new(),
            truncation_ids: HashSet::from(["trunc-1".into()]),
        };

        let result = MessageManager::truncate_api_history_with_cleanup(
            &api, &cline, 300, &removed, false,
        );

        assert!(!result.iter().any(|m| m.is_truncation_marker));
    }

    #[test]
    fn truncate_api_handles_race_condition() {
        // clineMessage ts=150, but no API message has ts=150
        // API messages: user@100, assistant@120, user@200
        // Should find user@200 as the first user message >= 150
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::assistant(120),
            ApiMessage::user(200),
            ApiMessage::assistant(250),
        ];
        let cline: Vec<ClineMessage> = vec![];
        let removed = ContextEventIds::default();

        let result = MessageManager::truncate_api_history_with_cleanup(
            &api, &cline, 150, &removed, false,
        );

        // Should keep messages with ts < 200 (the resolved cutoff)
        assert!(result.iter().all(|m| m.ts.unwrap_or(0) < 200));
        assert_eq!(result.len(), 2); // user@100, assistant@120
    }

    #[test]
    fn truncate_api_skip_cleanup() {
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::summary(Some(150), "orphan"),
            ApiMessage::assistant(200),
        ];
        let cline: Vec<ClineMessage> = vec![];
        let removed = ContextEventIds::default();

        let result = MessageManager::truncate_api_history_with_cleanup(
            &api, &cline, 300, &removed, true, // skip_cleanup = true
        );

        // With skip_cleanup, orphaned parent tags are NOT cleared
        // The summary is NOT removed because it wasn't in removed_ids
        assert!(result.iter().any(|m| m.is_summary));
    }

    #[test]
    fn truncate_api_clears_orphaned_parent_via_cleanup() {
        // When a summary is removed by step 3, messages with condense_parent
        // pointing to it should have their parent cleared by step 5
        let mut msg_with_parent = ApiMessage::user(100);
        msg_with_parent.condense_parent = Some("condense-1".into());

        let api = vec![
            msg_with_parent,
            ApiMessage::summary(Some(150), "condense-1"),
            ApiMessage::assistant(200),
        ];
        let cline: Vec<ClineMessage> = vec![];
        let removed = ContextEventIds {
            condense_ids: HashSet::from(["condense-1".into()]),
            truncation_ids: HashSet::new(),
        };

        let result = MessageManager::truncate_api_history_with_cleanup(
            &api, &cline, 300, &removed, false,
        );

        // Summary removed by step 3, then parent cleared by step 5
        assert!(result[0].condense_parent.is_none());
    }

    // -- integration: full rewind workflow ----------------------------------

    #[test]
    fn full_rewind_workflow() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::new(200, "assistant"),
            ClineMessage::condense(250, "c1"),
            ClineMessage::new(300, "user_feedback"),
            ClineMessage::new(400, "assistant"),
        ];
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::assistant(200),
            ApiMessage::summary(Some(220), "c1"),
            ApiMessage::user(300),
            ApiMessage::assistant(400),
        ];

        let mgr = make_manager();
        // Rewind to ts=300 (the second user_feedback), not including target
        let result = mgr
            .rewind_to_timestamp(&cline, &api, 300, RewindOptions::default())
            .unwrap();

        // Cline: keeps [100, 200, condense@250] (3 messages before index 3)
        assert_eq!(result.cline_messages.len(), 3);

        // API: keeps messages with ts < 300
        // user@100, assistant@200, summary@220 (c1 is still valid because condense@250 is kept)
        assert_eq!(result.api_history.len(), 3);

        // The condense ID c1 should NOT be in removed set
        assert!(!result.removed_context_event_ids.condense_ids.contains("c1"));
    }

    #[test]
    fn full_rewind_removes_condense() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::condense(150, "c1"),
            ClineMessage::new(200, "user_feedback"),
            ClineMessage::new(300, "assistant"),
        ];
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::summary(Some(120), "c1"),
            ApiMessage::user(200),
            ApiMessage::assistant(300),
        ];

        let mgr = make_manager();

        // Rewind to ts=150 (the condense message), include_target_message=false
        // → cutoff_index = index_of(150) = 1, keeps [0, 1) = [100]
        // condense@150 is removed → c1 is in removed_ids → summary c1 is orphaned
        let result = mgr
            .rewind_to_timestamp(&cline, &api, 150, RewindOptions::default())
            .unwrap();

        assert_eq!(result.cline_messages.len(), 1);
        assert_eq!(result.cline_messages[0].ts, 100);
        assert!(result.removed_context_event_ids.condense_ids.contains("c1"));
        // Summary c1 should be removed from API history
        assert!(!result.api_history.iter().any(|m| m.is_summary));
    }

    #[test]
    fn multi_step_rewind() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::new(200, "assistant"),
            ClineMessage::new(300, "user_feedback"),
            ClineMessage::new(400, "assistant"),
            ClineMessage::new(500, "user_feedback"),
        ];
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::assistant(200),
            ApiMessage::user(300),
            ApiMessage::assistant(400),
            ApiMessage::user(500),
        ];

        let mgr = make_manager();

        // First rewind to ts=300
        let result1 = mgr
            .rewind_to_timestamp(&cline, &api, 300, RewindOptions::default())
            .unwrap();

        assert_eq!(result1.cline_messages.len(), 2);
        assert_eq!(result1.api_history.len(), 2);

        // Second rewind on the result to ts=100
        let result2 = mgr
            .rewind_to_timestamp(
                &result1.cline_messages,
                &result1.api_history,
                100,
                RewindOptions::default(),
            )
            .unwrap();

        assert_eq!(result2.cline_messages.len(), 0);
        assert_eq!(result2.api_history.len(), 0);
    }

    #[test]
    fn rewind_preserves_api_messages_without_ts() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::new(200, "assistant"),
        ];
        let api = vec![
            ApiMessage::new(None, "system"),
            ApiMessage::user(100),
            ApiMessage::assistant(200),
        ];

        let mgr = make_manager();
        let result = mgr
            .rewind_to_timestamp(&cline, &api, 200, RewindOptions::default())
            .unwrap();

        // Messages without ts are always kept (filter: !m.ts || m.ts < cutoff)
        assert_eq!(result.api_history.len(), 2); // system + user@100
        assert!(result.api_history[0].ts.is_none());
    }
}
