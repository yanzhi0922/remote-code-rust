//! Message fingerprint computation.
//!
//! Provides [`compute_message_fingerprint`] for generating a deterministic
//! hash of the message list sent to the API.  This is used for cache
//! invalidation detection and request deduplication.
//!
//! Based on upstream Claude Code's `computeFingerprintFromMessages` in
//! `utils/fingerprint.ts`.

use serde_json::Value;
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Fingerprint computation
// ---------------------------------------------------------------------------

/// Compute a SHA-256 fingerprint from a list of API messages.
///
/// The fingerprint is derived from the canonical JSON representation of the
/// messages array, sorted by key for determinism.  Only the `role` and
/// `content` fields are included to avoid spurious fingerprint changes from
/// non-semantic metadata.
///
/// # Arguments
///
/// * `messages` — The messages array that will be sent to the API.
///
/// # Returns
///
/// A hex-encoded SHA-256 digest string.
#[must_use]
pub fn compute_message_fingerprint(messages: &[Value]) -> String {
    let mut hasher = Sha256::new();

    for message in messages {
        // Extract role and content for a stable fingerprint.
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        let content = message.get("content").cloned().unwrap_or(Value::Null);

        // Create a canonical representation.
        let canonical = serde_json::json!({
            "content": content,
            "role": role,
        });

        // Sort keys for deterministic serialization.
        let mut canonical_obj = match canonical {
            Value::Object(map) => {
                let mut pairs: Vec<_> = map.into_iter().collect();
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
                pairs
                    .into_iter()
                    .collect::<serde_json::Map<String, Value>>()
            }
            other => {
                // Shouldn't happen, but handle gracefully.
                let mut map = serde_json::Map::new();
                map.insert("value".to_owned(), other);
                map
            }
        };

        // Sort nested content arrays for determinism.
        if let Some(content_val) = canonical_obj.get_mut("content") {
            sort_content_keys(content_val);
        }

        let bytes = serde_json::to_string(&canonical_obj).unwrap_or_default();
        hasher.update(bytes.as_bytes());
    }

    let result = hasher.finalize();
    format!("{result:x}")
}

/// Recursively sort keys in JSON content for deterministic serialization.
fn sort_content_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut pairs: Vec<_> = map.clone().into_iter().collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            *map = pairs.into_iter().collect();
            for (_, v) in map.iter_mut() {
                sort_content_keys(v);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                sort_content_keys(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fingerprint_is_deterministic() {
        let messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "assistant", "content": "hi there"}),
        ];
        let fp1 = compute_message_fingerprint(&messages);
        let fp2 = compute_message_fingerprint(&messages);
        assert_eq!(fp1, fp2);
        // Should be a 64-char hex string (SHA-256).
        assert_eq!(fp1.len(), 64);
    }

    #[test]
    fn fingerprint_changes_with_content() {
        let messages_a = vec![json!({"role": "user", "content": "hello"})];
        let messages_b = vec![json!({"role": "user", "content": "world"})];
        let fp_a = compute_message_fingerprint(&messages_a);
        let fp_b = compute_message_fingerprint(&messages_b);
        assert_ne!(fp_a, fp_b);
    }

    #[test]
    fn fingerprint_ignores_non_semantic_fields() {
        let messages_a = vec![json!({"role": "user", "content": "hello", "metadata": "a"})];
        let messages_b = vec![json!({"role": "user", "content": "hello", "metadata": "b"})];
        let fp_a = compute_message_fingerprint(&messages_a);
        let fp_b = compute_message_fingerprint(&messages_b);
        // Should be the same since metadata is ignored.
        assert_eq!(fp_a, fp_b);
    }

    #[test]
    fn fingerprint_handles_empty_messages() {
        let messages: Vec<Value> = vec![];
        let fp = compute_message_fingerprint(&messages);
        assert_eq!(fp.len(), 64);
    }
}
