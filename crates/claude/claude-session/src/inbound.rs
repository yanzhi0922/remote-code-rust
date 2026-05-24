//! Inbound message processing for bridge clients.
//!
//! Native Rust implementation of `claude-code-rev/src/bridge/inboundMessages.ts`.
//! Processes incoming user messages, normalizing image blocks and extracting
//! content fields.

use serde_json::Value;
use uuid::Uuid;

/// Result of processing an inbound message.
pub struct InboundMessage {
    /// The message content (text or content blocks).
    pub content: InboundContent,
    /// Optional UUID from the bridge client.
    pub uuid: Option<Uuid>,
}

/// Inbound message content.
pub enum InboundContent {
    /// Plain text message.
    Text(String),
    /// Structured content blocks (text, images, etc.).
    Blocks(Vec<Value>),
}

/// Process an inbound user message from the bridge.
///
/// Returns `None` if the message should be skipped (non-user, missing content).
pub fn extract_inbound_message_fields(
    msg_type: &str,
    content: Option<&Value>,
    msg_uuid: Option<&str>,
) -> Option<InboundMessage> {
    // Only process user messages
    if msg_type != "user" {
        return None;
    }

    let content_value = content?;

    let inbound_content = if let Some(arr) = content_value.as_array() {
        if arr.is_empty() {
            return None;
        }
        InboundContent::Blocks(normalize_image_blocks(arr))
    } else if let Some(text) = content_value.as_str() {
        if text.is_empty() {
            return None;
        }
        InboundContent::Text(text.to_owned())
    } else {
        return None;
    };

    let uuid = msg_uuid.and_then(|u| Uuid::parse_str(u).ok());

    Some(InboundMessage {
        content: inbound_content,
        uuid,
    })
}

/// Normalize image content blocks from bridge clients.
///
/// iOS/web clients may send `mediaType` (camelCase) instead of
/// `media_type` (snake_case). Without normalization, the bad block
/// poisons the session.
pub fn normalize_image_blocks(blocks: &[Value]) -> Vec<Value> {
    if !blocks.iter().any(is_malformed_base64_image) {
        return blocks.to_vec();
    }

    blocks
        .iter()
        .map(|block| {
            if !is_malformed_base64_image(block) {
                return block.clone();
            }
            let mut block = block.clone();
            if let Some(source) = block.get_mut("source") {
                if let Some(obj) = source.as_object_mut() {
                    // Fix camelCase mediaType -> snake_case media_type
                    if let Some(media_type) = obj.remove("mediaType") {
                        obj.insert("media_type".to_owned(), media_type);
                    }
                    // Ensure type is set
                    obj.entry("type")
                        .or_insert(Value::String("base64".to_owned()));
                }
            }
            block
        })
        .collect()
}

/// Check if a content block is a malformed base64 image (missing media_type).
fn is_malformed_base64_image(block: &Value) -> bool {
    let obj = match block.as_object() {
        Some(o) => o,
        None => return false,
    };
    if obj.get("type").and_then(|v| v.as_str()) != Some("image") {
        return false;
    }
    let source = match obj.get("source") {
        Some(Value::Object(s)) => s,
        _ => return false,
    };
    if source.get("type").and_then(|v| v.as_str()) != Some("base64") {
        return false;
    }
    // Malformed if media_type is missing (wrong casing or absent)
    if source.contains_key("media_type") {
        return false; // correctly formed
    }
    // Has mediaType (camelCase) — needs normalization, or no media type at all
    source.contains_key("mediaType") || !source.iter().any(|(k, _)| k.contains("media"))
}

/// Detect image format from base64 data prefix.
pub fn detect_image_format_from_base64(data: &str) -> Option<&'static str> {
    if data.starts_with("/9j/") {
        Some("image/jpeg")
    } else if data.starts_with("iVBOR") {
        Some("image/png")
    } else if data.starts_with("R0lG") {
        Some("image/gif")
    } else if data.starts_with("UklGR") {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_text_content() {
        let result = extract_inbound_message_fields(
            "user",
            Some(&json!("hello world")),
            Some("550e8400-e29b-41d4-a716-446655440000"),
        )
        .expect("should extract");

        match result.content {
            InboundContent::Text(t) => assert_eq!(t, "hello world"),
            _ => panic!("expected text"),
        }
        assert!(result.uuid.is_some());
    }

    #[test]
    fn skips_non_user_messages() {
        assert!(extract_inbound_message_fields("assistant", Some(&json!("hi")), None).is_none());
    }

    #[test]
    fn skips_empty_content() {
        assert!(extract_inbound_message_fields("user", Some(&json!("")), None).is_none());
    }

    #[test]
    fn normalizes_camel_case_media_type() {
        let blocks = vec![json!({
            "type": "image",
            "source": {
                "type": "base64",
                "mediaType": "image/png",
                "data": "iVBORw0KGgo="
            }
        })];
        let normalized = normalize_image_blocks(&blocks);
        let source = &normalized[0]["source"];
        assert!(source.get("media_type").is_some());
        assert!(source.get("mediaType").is_none());
    }

    #[test]
    fn detects_jpeg_from_base64_prefix() {
        assert_eq!(
            detect_image_format_from_base64("/9j/4AAQSkZJRg"),
            Some("image/jpeg")
        );
    }

    #[test]
    fn detects_png_from_base64_prefix() {
        assert_eq!(
            detect_image_format_from_base64("iVBORw0KGgoAAAANSUhEUg"),
            Some("image/png")
        );
    }

    #[test]
    fn returns_none_for_unknown_format() {
        assert!(detect_image_format_from_base64("AAAA").is_none());
    }
}
