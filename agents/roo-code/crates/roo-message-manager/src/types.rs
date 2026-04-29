//! Core types for message management.
//!
//! Defines the data structures used throughout the message manager:
//! - [`ClineMessage`] — UI-facing conversation message
//! - [`ApiMessage`] — API-level conversation history entry
//! - [`RewindOptions`] — options controlling rewind behaviour
//! - [`ContextEventIds`] — IDs collected from removed context events
//! - [`ContextCondenseInfo`] — condense context metadata
//! - [`ContextTruncationInfo`] — sliding window truncation metadata

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// RewindOptions
// ---------------------------------------------------------------------------

/// Options that control how a rewind operation behaves.
///
/// Ported from `RewindOptions` in `core/message-manager/index.ts`.
#[derive(Debug, Clone, Default)]
pub struct RewindOptions {
    /// Whether the message at the target index/timestamp should be **included**
    /// in the set of messages being removed.
    ///
    /// - `true`  → edit scenario (target message is also deleted)
    /// - `false` → delete scenario (target message is kept)
    pub include_target_message: bool,

    /// When `true`, skip the post-truncation cleanup step that removes
    /// orphaned condense / truncation tags.
    pub skip_cleanup: bool,
}

// ---------------------------------------------------------------------------
// ContextEventIds
// ---------------------------------------------------------------------------

/// IDs of context-management events that are being removed during a rewind.
///
/// These IDs are used to locate and remove the corresponding Summary messages
/// and truncation markers in the API conversation history.
#[derive(Debug, Clone, Default)]
pub struct ContextEventIds {
    /// `condenseId` values from `condense_context` cline messages being removed.
    pub condense_ids: HashSet<String>,

    /// `truncationId` values from `sliding_window_truncation` cline messages
    /// being removed.
    pub truncation_ids: HashSet<String>,
}

// ---------------------------------------------------------------------------
// ContextCondenseInfo / ContextTruncationInfo
// ---------------------------------------------------------------------------

/// Metadata attached to a `condense_context` cline message.
#[derive(Debug, Clone)]
pub struct ContextCondenseInfo {
    /// Unique identifier linking this condense event to the corresponding
    /// Summary message in the API history.
    pub condense_id: String,
}

/// Metadata attached to a `sliding_window_truncation` cline message.
#[derive(Debug, Clone)]
pub struct ContextTruncationInfo {
    /// Unique identifier linking this truncation event to the corresponding
    /// truncation marker in the API history.
    pub truncation_id: String,
}

// ---------------------------------------------------------------------------
// ClineMessage
// ---------------------------------------------------------------------------

/// A simplified representation of a UI conversation message.
///
/// Only the fields relevant to rewind / cleanup logic are included.
/// The full type lives in `roo-types::message`.
#[derive(Debug, Clone)]
pub struct ClineMessage {
    /// Unix-millisecond timestamp.
    pub ts: i64,

    /// Message kind, e.g. `"user_feedback"`, `"condense_context"`,
    /// `"sliding_window_truncation"`, etc.
    pub say: String,

    /// Present when `say == "condense_context"`.
    pub context_condense: Option<ContextCondenseInfo>,

    /// Present when `say == "sliding_window_truncation"`.
    pub context_truncation: Option<ContextTruncationInfo>,
}

impl ClineMessage {
    /// Convenience constructor for a plain message.
    pub fn new(ts: i64, say: impl Into<String>) -> Self {
        Self {
            ts,
            say: say.into(),
            context_condense: None,
            context_truncation: None,
        }
    }

    /// Create a `condense_context` message.
    pub fn condense(ts: i64, condense_id: impl Into<String>) -> Self {
        Self {
            ts,
            say: "condense_context".into(),
            context_condense: Some(ContextCondenseInfo {
                condense_id: condense_id.into(),
            }),
            context_truncation: None,
        }
    }

    /// Create a `sliding_window_truncation` message.
    pub fn truncation(ts: i64, truncation_id: impl Into<String>) -> Self {
        Self {
            ts,
            say: "sliding_window_truncation".into(),
            context_condense: None,
            context_truncation: Some(ContextTruncationInfo {
                truncation_id: truncation_id.into(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// ApiMessage
// ---------------------------------------------------------------------------

/// A simplified representation of an API conversation history entry.
///
/// Only the fields relevant to rewind / cleanup logic are included.
#[derive(Debug, Clone)]
pub struct ApiMessage {
    /// Unix-millisecond timestamp. `None` for synthetic entries.
    pub ts: Option<i64>,

    /// `"user"` or `"assistant"`.
    pub role: String,

    /// `true` when this entry is a Summary produced by context condensation.
    pub is_summary: bool,

    /// Links this Summary to its `condense_context` cline message.
    pub condense_id: Option<String>,

    /// Points to the `condense_id` of the summary that replaces this message.
    /// Messages with `condense_parent` are filtered out when sending to API if
    /// the summary still exists.
    pub condense_parent: Option<String>,

    /// `true` when this entry is a truncation marker produced by the sliding
    /// window.
    pub is_truncation_marker: bool,

    /// Links this marker to its `sliding_window_truncation` cline message.
    pub truncation_id: Option<String>,

    /// Points to the `truncation_id` of the marker that hides this message.
    /// Messages with `truncation_parent` are filtered out when sending to API
    /// if the marker still exists.
    pub truncation_parent: Option<String>,
}

impl ApiMessage {
    /// Convenience constructor for a regular API message.
    pub fn new(ts: Option<i64>, role: impl Into<String>) -> Self {
        Self {
            ts,
            role: role.into(),
            is_summary: false,
            condense_id: None,
            condense_parent: None,
            is_truncation_marker: false,
            truncation_id: None,
            truncation_parent: None,
        }
    }

    /// Create a user message.
    pub fn user(ts: i64) -> Self {
        Self::new(Some(ts), "user")
    }

    /// Create an assistant message.
    pub fn assistant(ts: i64) -> Self {
        Self::new(Some(ts), "assistant")
    }

    /// Create a Summary message linked to a condense event.
    ///
    /// In the TS source, summary messages have `role: "user"` (fresh-start model).
    pub fn summary(ts: Option<i64>, condense_id: impl Into<String>) -> Self {
        Self {
            ts,
            role: "user".into(),
            is_summary: true,
            condense_id: Some(condense_id.into()),
            condense_parent: None,
            is_truncation_marker: false,
            truncation_id: None,
            truncation_parent: None,
        }
    }

    /// Create a truncation marker linked to a sliding-window event.
    ///
    /// In the TS source, truncation markers have `role: "user"`.
    pub fn truncation_marker(ts: Option<i64>, truncation_id: impl Into<String>) -> Self {
        Self {
            ts,
            role: "user".into(),
            is_summary: false,
            condense_id: None,
            condense_parent: None,
            is_truncation_marker: true,
            truncation_id: Some(truncation_id.into()),
            truncation_parent: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cline_message_new() {
        let msg = ClineMessage::new(100, "user_feedback");
        assert_eq!(msg.ts, 100);
        assert_eq!(msg.say, "user_feedback");
        assert!(msg.context_condense.is_none());
        assert!(msg.context_truncation.is_none());
    }

    #[test]
    fn cline_message_condense() {
        let msg = ClineMessage::condense(200, "condense-1");
        assert_eq!(msg.ts, 200);
        assert_eq!(msg.say, "condense_context");
        assert_eq!(
            msg.context_condense.as_ref().unwrap().condense_id,
            "condense-1"
        );
        assert!(msg.context_truncation.is_none());
    }

    #[test]
    fn cline_message_truncation() {
        let msg = ClineMessage::truncation(300, "trunc-1");
        assert_eq!(msg.ts, 300);
        assert_eq!(msg.say, "sliding_window_truncation");
        assert!(msg.context_condense.is_none());
        assert_eq!(
            msg.context_truncation.as_ref().unwrap().truncation_id,
            "trunc-1"
        );
    }

    #[test]
    fn api_message_user() {
        let msg = ApiMessage::user(100);
        assert_eq!(msg.ts, Some(100));
        assert_eq!(msg.role, "user");
        assert!(!msg.is_summary);
        assert!(!msg.is_truncation_marker);
    }

    #[test]
    fn api_message_assistant() {
        let msg = ApiMessage::assistant(200);
        assert_eq!(msg.ts, Some(200));
        assert_eq!(msg.role, "assistant");
        assert!(!msg.is_summary);
        assert!(!msg.is_truncation_marker);
    }

    #[test]
    fn api_message_summary() {
        let msg = ApiMessage::summary(Some(300), "condense-1");
        assert!(msg.is_summary);
        assert_eq!(msg.role, "user"); // TS: fresh-start model uses "user"
        assert_eq!(msg.condense_id.as_deref(), Some("condense-1"));
        assert!(msg.condense_parent.is_none());
        assert!(!msg.is_truncation_marker);
        assert!(msg.truncation_id.is_none());
        assert!(msg.truncation_parent.is_none());
    }

    #[test]
    fn api_message_truncation_marker() {
        let msg = ApiMessage::truncation_marker(Some(400), "trunc-1");
        assert!(msg.is_truncation_marker);
        assert_eq!(msg.role, "user"); // TS: truncation markers use "user"
        assert_eq!(msg.truncation_id.as_deref(), Some("trunc-1"));
        assert!(msg.truncation_parent.is_none());
        assert!(!msg.is_summary);
        assert!(msg.condense_id.is_none());
        assert!(msg.condense_parent.is_none());
    }

    #[test]
    fn rewind_options_default() {
        let opts = RewindOptions::default();
        assert!(!opts.include_target_message);
        assert!(!opts.skip_cleanup);
    }

    #[test]
    fn context_event_ids_default() {
        let ids = ContextEventIds::default();
        assert!(ids.condense_ids.is_empty());
        assert!(ids.truncation_ids.is_empty());
    }
}
