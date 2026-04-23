//! Session restore support.
//!
//! Provides functionality for finding and restoring previous sessions,
//! enabling users to resume conversations from where they left off.

use chrono::{DateTime, Utc};
use rc_core::ConversationEntry;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Information about a session that can be restored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRestoreInfo {
    /// Unique session identifier.
    pub session_id: String,
    /// Working directory the session was started in.
    pub cwd: String,
    /// Model used in the session.
    pub model: String,
    /// Timestamp when the session was created.
    pub timestamp: DateTime<Utc>,
    /// Number of messages in the session.
    pub message_count: usize,
}

impl SessionRestoreInfo {
    /// Create a new session restore info.
    pub fn new(
        session_id: String,
        cwd: String,
        model: String,
        timestamp: DateTime<Utc>,
        message_count: usize,
    ) -> Self {
        Self {
            session_id,
            cwd,
            model,
            timestamp,
            message_count,
        }
    }

    /// Format the session info for display.
    pub fn display_summary(&self) -> String {
        let time = self.timestamp.format("%Y-%m-%d %H:%M");
        format!(
            "[{}] {} ({} messages, model: {})",
            time, self.session_id, self.message_count, self.model
        )
    }
}

/// Find sessions that can be restored.
///
/// In a full implementation this would scan the session storage directory
/// for valid session files. Currently returns an empty list as a placeholder.
pub fn find_restorable_sessions(limit: usize) -> anyhow::Result<Vec<SessionRestoreInfo>> {
    // Placeholder: in production, this would:
    // 1. Scan the session directory for session files
    // 2. Parse each file to extract metadata
    // 3. Sort by timestamp (most recent first)
    // 4. Return up to `limit` results
    let _ = limit;
    Ok(vec![])
}

/// Restore a session by its ID, returning the conversation entries.
///
/// In a full implementation this would load the session file from disk.
/// Currently returns an empty list as a placeholder.
pub fn restore_session(session_id: &str) -> anyhow::Result<Vec<ConversationEntry>> {
    // Placeholder: in production, this would:
    // 1. Locate the session file by ID
    // 2. Parse the JSONL transcript
    // 3. Return the conversation entries
    let _ = session_id;
    Ok(vec![])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_session(id: &str, count: usize) -> SessionRestoreInfo {
        SessionRestoreInfo::new(
            id.to_string(),
            "/tmp/project".to_string(),
            "gpt-4".to_string(),
            Utc::now(),
            count,
        )
    }

    #[test]
    fn session_restore_info_new() {
        let info = make_test_session("sess-1", 10);
        assert_eq!(info.session_id, "sess-1");
        assert_eq!(info.cwd, "/tmp/project");
        assert_eq!(info.model, "gpt-4");
        assert_eq!(info.message_count, 10);
    }

    #[test]
    fn display_summary_format() {
        let info = make_test_session("sess-abc", 42);
        let summary = info.display_summary();
        assert!(summary.contains("sess-abc"));
        assert!(summary.contains("42 messages"));
        assert!(summary.contains("gpt-4"));
    }

    #[test]
    fn display_summary_contains_timestamp() {
        let info = make_test_session("sess-ts", 5);
        let summary = info.display_summary();
        // Should contain a date-like pattern
        assert!(summary.contains("20")); // Year prefix
    }

    #[test]
    fn find_restorable_sessions_returns_ok() {
        let result = find_restorable_sessions(10);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn find_restorable_sessions_respects_limit() {
        let result = find_restorable_sessions(0);
        assert!(result.is_ok());
    }

    #[test]
    fn restore_session_returns_ok() {
        let result = restore_session("nonexistent-session");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn restore_session_with_empty_id() {
        let result = restore_session("");
        assert!(result.is_ok());
    }

    #[test]
    fn session_restore_info_serde_roundtrip() {
        let info = make_test_session("sess-serde", 100);
        let json = serde_json::to_string(&info).expect("serialize");
        let back: SessionRestoreInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.session_id, info.session_id);
        assert_eq!(back.cwd, info.cwd);
        assert_eq!(back.model, info.model);
        assert_eq!(back.message_count, info.message_count);
    }
}
