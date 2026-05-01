//! Bridge messaging protocol with JSON serialization.
//!
//! Defines the wire format for messages exchanged between remote-code and
//! the IDE over the bridge.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Direction of a bridge message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    /// Message is going to the IDE.
    ToIde,
    /// Message is coming from the IDE.
    FromIde,
}

/// Payload variants carried by a bridge message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgePayload {
    /// Request to open a file.
    FileOpen {
        /// Absolute path of the file to open.
        path: String,
        /// Optional line number to scroll to.
        line: Option<u32>,
    },
    /// Apply a diff to a file.
    DiffApply {
        /// Path of the file to patch.
        path: String,
        /// Unified diff content.
        diff: String,
    },
    /// Diagnostic information from the IDE.
    Diagnostic {
        /// File path the diagnostic refers to.
        path: String,
        /// Severity level.
        severity: String,
        /// Diagnostic message.
        message: String,
    },
    /// Status update notification.
    StatusUpdate {
        /// New status string.
        status: String,
        /// Optional detail.
        detail: Option<String>,
    },
    /// Selection range changed in the editor.
    SelectionChanged {
        /// File path.
        path: String,
        /// Start line (0-based).
        start_line: u32,
        /// End line (0-based).
        end_line: u32,
    },
}

/// A single message on the bridge wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMessage {
    /// Unique message identifier.
    pub id: String,
    /// Direction of the message.
    pub direction: MessageDirection,
    /// The payload.
    pub payload: BridgePayload,
}

impl BridgeMessage {
    /// Create a new message with a generated ID.
    pub fn new(direction: MessageDirection, payload: BridgePayload) -> Self {
        Self {
            id: generate_id(),
            direction,
            payload,
        }
    }

    /// Create a message with a specific ID (useful for testing).
    pub fn with_id(id: String, direction: MessageDirection, payload: BridgePayload) -> Self {
        Self {
            id,
            direction,
            payload,
        }
    }

    /// Serialize the message to a JSON string.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserialize a message from a JSON string.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    /// Return the payload type as a string for routing.
    pub fn payload_type(&self) -> &'static str {
        match &self.payload {
            BridgePayload::FileOpen { .. } => "file_open",
            BridgePayload::DiffApply { .. } => "diff_apply",
            BridgePayload::Diagnostic { .. } => "diagnostic",
            BridgePayload::StatusUpdate { .. } => "status_update",
            BridgePayload::SelectionChanged { .. } => "selection_changed",
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a simple unique message ID.
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!("msg-{}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_to_from_json() {
        let msg = BridgeMessage::with_id(
            "test-1".into(),
            MessageDirection::ToIde,
            BridgePayload::FileOpen {
                path: "/tmp/a.rs".into(),
                line: Some(42),
            },
        );
        let json = msg.to_json().expect("serialize");
        let back = BridgeMessage::from_json(&json).expect("deserialize");
        assert_eq!(back.id, "test-1");
        assert_eq!(back.direction, MessageDirection::ToIde);
    }

    #[test]
    fn payload_file_open() {
        let msg = BridgeMessage::new(
            MessageDirection::ToIde,
            BridgePayload::FileOpen {
                path: "x.rs".into(),
                line: None,
            },
        );
        assert_eq!(msg.payload_type(), "file_open");
    }

    #[test]
    fn payload_diff_apply() {
        let msg = BridgeMessage::new(
            MessageDirection::ToIde,
            BridgePayload::DiffApply {
                path: "y.rs".into(),
                diff: "+++ ".into(),
            },
        );
        assert_eq!(msg.payload_type(), "diff_apply");
    }

    #[test]
    fn payload_diagnostic() {
        let msg = BridgeMessage::new(
            MessageDirection::FromIde,
            BridgePayload::Diagnostic {
                path: "z.rs".into(),
                severity: "error".into(),
                message: "type mismatch".into(),
            },
        );
        assert_eq!(msg.payload_type(), "diagnostic");
    }

    #[test]
    fn payload_status_update() {
        let msg = BridgeMessage::new(
            MessageDirection::ToIde,
            BridgePayload::StatusUpdate {
                status: "ready".into(),
                detail: Some("all good".into()),
            },
        );
        assert_eq!(msg.payload_type(), "status_update");
    }

    #[test]
    fn payload_selection_changed() {
        let msg = BridgeMessage::new(
            MessageDirection::FromIde,
            BridgePayload::SelectionChanged {
                path: "a.rs".into(),
                start_line: 10,
                end_line: 20,
            },
        );
        assert_eq!(msg.payload_type(), "selection_changed");
    }

    #[test]
    fn generate_id_unique() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn direction_serde() {
        let d = MessageDirection::ToIde;
        let json = serde_json::to_string(&d).expect("s");
        assert_eq!(json, "\"to_ide\"");
    }

    #[test]
    fn message_new_has_id() {
        let msg = BridgeMessage::new(
            MessageDirection::FromIde,
            BridgePayload::StatusUpdate {
                status: "ok".into(),
                detail: None,
            },
        );
        assert!(!msg.id.is_empty());
    }

    #[test]
    fn roundtrip_all_payload_types() {
        let payloads: Vec<BridgePayload> = vec![
            BridgePayload::FileOpen {
                path: "a".into(),
                line: Some(1),
            },
            BridgePayload::DiffApply {
                path: "b".into(),
                diff: "d".into(),
            },
            BridgePayload::Diagnostic {
                path: "c".into(),
                severity: "warn".into(),
                message: "m".into(),
            },
            BridgePayload::StatusUpdate {
                status: "s".into(),
                detail: Some("d".into()),
            },
            BridgePayload::SelectionChanged {
                path: "e".into(),
                start_line: 1,
                end_line: 2,
            },
        ];
        for payload in payloads {
            let msg = BridgeMessage::with_id("x".into(), MessageDirection::ToIde, payload);
            let json = msg.to_json().expect("json");
            let back = BridgeMessage::from_json(&json).expect("parse");
            assert_eq!(msg.payload, back.payload);
        }
    }
}
