//! Orphaned artifact detection and cleanup.
//!
//! When messages are rewound, command-output artifacts whose execution IDs no
//! longer correspond to any remaining message timestamp become orphaned. This
//! module provides pure functions to compute the set of valid IDs and detect
//! orphans.

use std::collections::HashSet;

use crate::types::{ApiMessage, ClineMessage};

/// Compute the set of "valid" IDs derived from the timestamps of all remaining
/// messages.
///
/// Each message timestamp (both cline and API) is stringified and added to the
/// set. Artifact files whose names do not appear in this set are considered
/// orphaned.
pub fn compute_valid_ids(cline_messages: &[ClineMessage], api_messages: &[ApiMessage]) -> HashSet<String> {
    let mut ids = HashSet::new();

    for msg in cline_messages {
        ids.insert(msg.ts.to_string());
    }

    for msg in api_messages {
        if let Some(ts) = msg.ts {
            ids.insert(ts.to_string());
        }
    }

    ids
}

/// Given a set of existing artifact IDs and a set of valid IDs, return the
/// orphaned IDs (those in `artifact_ids` but not in `valid_ids`).
pub fn find_orphaned_artifacts<'a>(
    valid_ids: &HashSet<String>,
    artifact_ids: impl IntoIterator<Item = &'a String>,
) -> Vec<&'a String> {
    artifact_ids
        .into_iter()
        .filter(|id| !valid_ids.contains(*id))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_valid_ids_from_cline_messages() {
        let cline = vec![
            ClineMessage::new(100, "user_feedback"),
            ClineMessage::new(200, "assistant"),
        ];
        let api: Vec<ApiMessage> = vec![];

        let ids = compute_valid_ids(&cline, &api);
        assert!(ids.contains("100"));
        assert!(ids.contains("200"));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn compute_valid_ids_from_api_messages() {
        let cline: Vec<ClineMessage> = vec![];
        let api = vec![
            ApiMessage::user(100),
            ApiMessage::assistant(200),
        ];

        let ids = compute_valid_ids(&cline, &api);
        assert!(ids.contains("100"));
        assert!(ids.contains("200"));
    }

    #[test]
    fn compute_valid_ids_skips_api_messages_without_ts() {
        let api = vec![
            ApiMessage::new(None, "user"),
            ApiMessage::assistant(200),
        ];
        let ids = compute_valid_ids(&[], &api);
        assert!(!ids.contains("None")); // should not stringify None
        assert!(ids.contains("200"));
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn compute_valid_ids_deduplicates() {
        let cline = vec![ClineMessage::new(100, "user_feedback")];
        let api = vec![ApiMessage::user(100)];

        let ids = compute_valid_ids(&cline, &api);
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn find_orphaned_artifacts_basic() {
        let valid: HashSet<String> = ["100".into(), "200".into()].into_iter().collect();
        let artifacts: Vec<String> = vec!["100".into(), "200".into(), "300".into()];

        let orphans = find_orphaned_artifacts(&valid, artifacts.iter().map(|s| s));
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0], "300");
    }

    #[test]
    fn find_orphaned_artifacts_none_orphaned() {
        let valid: HashSet<String> = ["100".into(), "200".into()].into_iter().collect();
        let artifacts: Vec<String> = vec!["100".into(), "200".into()];

        let orphans = find_orphaned_artifacts(&valid, artifacts.iter().map(|s| s));
        assert!(orphans.is_empty());
    }

    #[test]
    fn find_orphaned_artifacts_all_orphaned() {
        let valid: HashSet<String> = HashSet::new();
        let artifacts: Vec<String> = vec!["100".into(), "200".into()];

        let orphans = find_orphaned_artifacts(&valid, artifacts.iter().map(|s| s));
        assert_eq!(orphans.len(), 2);
    }

    #[test]
    fn empty_inputs() {
        let ids = compute_valid_ids(&[], &[]);
        assert!(ids.is_empty());

        let orphans = find_orphaned_artifacts(&HashSet::new(), std::iter::empty());
        assert!(orphans.is_empty());
    }
}
