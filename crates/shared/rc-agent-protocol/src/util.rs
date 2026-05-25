//! Shared utilities used by all adapter implementations.

use std::collections::HashSet;

use crate::events::UnifiedAgentEvent;
use crate::types::AgentCapability;

/// Extract a human-readable message from a `catch_unwind` panic payload.
///
/// Panics in Rust can carry `&str`, `String`, or arbitrary types. This function
/// attempts to extract the most common cases and falls back to a generic message.
pub fn extract_panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_owned()
    }
}

/// Create an error event from a panic payload.
///
/// Convenience wrapper that combines [`extract_panic_message`] with
/// [`UnifiedAgentEvent::Error`] construction.
pub fn panic_to_error_event(
    session_id: &str,
    prefix: &str,
    payload: Box<dyn std::any::Any + Send>,
) -> UnifiedAgentEvent {
    let msg = extract_panic_message(&payload);
    UnifiedAgentEvent::Error {
        session_id: session_id.to_owned(),
        message: format!("{prefix}: {msg}"),
        recoverable: false,
    }
}

/// Build the standard capability set shared by all adapters.
///
/// Includes `Streaming`, `ToolUse`, and `Permissions`. Additional capabilities
/// can be appended via `extra`.
pub fn standard_capabilities(extra: &[AgentCapability]) -> HashSet<AgentCapability> {
    let mut caps = HashSet::new();
    caps.insert(AgentCapability::Streaming);
    caps.insert(AgentCapability::ToolUse);
    caps.insert(AgentCapability::Permissions);
    for cap in extra {
        caps.insert(*cap);
    }
    caps
}

/// Percent-encode a path segment for use in URLs.
///
/// Encodes all characters that are not unreserved (A-Z, a-z, 0-9, `-`, `.`, `_`, `~`)
/// or reserved sub-delims (`!`, `$`, `&`, `'`, `(`, `)`, `*`, `+`, `,`, `;`, `=`).
/// This is suitable for encoding individual path segments between `/` delimiters.
pub fn encode_path_segment(raw: &str) -> String {
    const UNRESERVED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    const SUB_DELIMS: &[u8] = b"!$&'()*+,;=";

    let mut encoded = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        if UNRESERVED.contains(&byte) || SUB_DELIMS.contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_str_panic() {
        let payload = Box::new("something broke") as Box<dyn std::any::Any + Send>;
        assert_eq!(extract_panic_message(&payload), "something broke");
    }

    #[test]
    fn extracts_string_panic() {
        let payload = Box::new("something broke".to_owned()) as Box<dyn std::any::Any + Send>;
        assert_eq!(extract_panic_message(&payload), "something broke");
    }

    #[test]
    fn extracts_unknown_panic() {
        let payload = Box::new(42usize) as Box<dyn std::any::Any + Send>;
        assert_eq!(extract_panic_message(&payload), "unknown panic");
    }

    #[test]
    fn panic_to_error_event_works() {
        let payload = Box::new("test panic") as Box<dyn std::any::Any + Send>;
        let event = panic_to_error_event("sess-1", "Agent crashed", payload);
        match event {
            UnifiedAgentEvent::Error {
                session_id,
                message,
                recoverable,
            } => {
                assert_eq!(session_id, "sess-1");
                assert!(message.contains("Agent crashed"));
                assert!(message.contains("test panic"));
                assert!(!recoverable);
            }
            _ => panic!("expected Error event"),
        }
    }

    #[test]
    fn standard_capabilities_has_base_set() {
        let caps = standard_capabilities(&[]);
        assert!(caps.contains(&AgentCapability::Streaming));
        assert!(caps.contains(&AgentCapability::ToolUse));
        assert!(caps.contains(&AgentCapability::Permissions));
        assert_eq!(caps.len(), 3);
    }

    #[test]
    fn standard_capabilities_adds_extras() {
        let caps = standard_capabilities(&[AgentCapability::McpSupport, AgentCapability::Subtasks]);
        assert_eq!(caps.len(), 5);
    }

    #[test]
    fn encode_path_segment_escapes_reserved_bytes() {
        assert_eq!(encode_path_segment("hello world"), "hello%20world");
        assert_eq!(encode_path_segment("foo/bar"), "foo%2Fbar");
        assert_eq!(encode_path_segment("a%b"), "a%25b");
        assert_eq!(encode_path_segment("simple-test_123"), "simple-test_123");
        assert_eq!(encode_path_segment(""), "");
        // Sub-delims should NOT be encoded.
        assert_eq!(encode_path_segment("$&'()+,;="), "$&'()+,;=");
    }
}
