//! AWS Bedrock Converse message format conversion.
//!
//! Derived from `src/api/transform/bedrock-converse-format.ts`.
//! Converts Anthropic-style [`ApiMessage`] into AWS Bedrock Converse format,
//! handling image bytes, tool use/result renaming, and video support.

use serde_json::{json, Value};

use roo_types::api::{ApiMessage, ContentBlock, ImageSource, MessageRole, ToolResultContent};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum length for a Bedrock tool use ID (matches OpenAI's 64-char limit).
const CALL_ID_MAX_LENGTH: usize = 64;

/// Valid image formats for Bedrock Converse.
const VALID_IMAGE_FORMATS: &[&str] = &["png", "jpeg", "gif", "webp"];

/// Base64 decoding table.
const B64_DECODE_TABLE: [i8; 128] = [
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62, -1, -1, -1, 63,
    52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1,
    -1,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14,
    15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1,
    -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
    41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Sanitises a tool call ID for use with Bedrock Converse (and OpenAI).
///
/// 1. Replaces any character that is not `[a-zA-Z0-9_-]` with `_`.
/// 2. If the result exceeds [`CALL_ID_MAX_LENGTH`] (64), truncates it and
///    appends an 8-hex-char hash suffix derived from the full original ID to
///    preserve uniqueness.
///
/// Source: `src/utils/tool-id.ts` — `sanitizeOpenAiCallId`
pub fn sanitize_openai_call_id(id: &str) -> String {
    // Step 1: sanitize characters
    let sanitized: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();

    // Step 2: truncate if needed
    if sanitized.len() <= CALL_ID_MAX_LENGTH {
        return sanitized;
    }

    // Compute a simple hash suffix from the full sanitized string
    let hash_suffix = fnv_hash_suffix(&sanitized);
    let prefix_max = CALL_ID_MAX_LENGTH - 9; // 8 hex + 1 separator
    let prefix = &sanitized[..prefix_max];
    format!("{prefix}_{hash_suffix}")
}

/// Converts Anthropic-style [`ApiMessage`]s into AWS Bedrock Converse format.
///
/// # Format mapping
/// - `text` → `{ "text": "…" }`
/// - `image` (base64) → `{ "image": { "format": "png", "source": { "bytes": […] } } }`
/// - `tool_use` → `{ "toolUse": { "toolUseId": "…", "name": "…", "input": {…} } }`
/// - `tool_result` → `{ "toolResult": { "toolUseId": "…", "content": […], "status": "success" } }`
///
/// Source: `src/api/transform/bedrock-converse-format.ts` — `convertToBedrockConverseMessages`
pub fn convert_to_bedrock_converse_messages(messages: &[ApiMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                MessageRole::Assistant => "assistant",
                MessageRole::User => "user",
            };

            let content: Vec<Value> = msg
                .content
                .iter()
                .filter_map(|block| convert_content_block(block))
                .collect();

            json!({
                "role": role,
                "content": content,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert a single [`ContentBlock`] to a Bedrock Converse content block JSON.
fn convert_content_block(block: &ContentBlock) -> Option<Value> {
    match block {
        ContentBlock::Text { text } => Some(json!({ "text": text })),

        ContentBlock::Image { source } => convert_image_block(source),

        ContentBlock::ToolUse { id, name, input } => Some(json!({
            "toolUse": {
                "toolUseId": sanitize_openai_call_id(id),
                "name": name,
                "input": input,
            }
        })),

        ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } => {
            let bedrock_content: Vec<Value> = content
                .iter()
                .map(|c| match c {
                    ToolResultContent::Text { text } => json!({ "text": text }),
                    ToolResultContent::Image { .. } => {
                        json!({ "text": "(see following message for image)" })
                    }
                })
                .collect();

            Some(json!({
                "toolResult": {
                    "toolUseId": sanitize_openai_call_id(tool_use_id),
                    "content": bedrock_content,
                    "status": "success",
                }
            }))
        }

        // Thinking / redacted_thinking blocks are not supported in Bedrock Converse
        ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => None,
    }
}

/// Convert an image source to Bedrock Converse image format.
fn convert_image_block(source: &ImageSource) -> Option<Value> {
    match source {
        ImageSource::Base64 { media_type, data } => {
            let format = media_type.split('/').nth(1).unwrap_or("png");
            if !VALID_IMAGE_FORMATS.contains(&format) {
                return None;
            }

            let bytes = base64_decode(data)?;

            // Represent bytes as a JSON array of numbers
            let bytes_json: Vec<Value> = bytes.into_iter().map(|b| json!(b)).collect();

            Some(json!({
                "image": {
                    "format": format,
                    "source": {
                        "bytes": bytes_json,
                    }
                }
            }))
        }
        ImageSource::Url { .. } => {
            // Bedrock Converse doesn't support URL-based images directly
            None
        }
    }
}

/// Simple base64 decoder (no external dependency required).
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let input: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let input = input.trim_end_matches('=');

    if input.is_empty() {
        return Some(Vec::new());
    }

    let mut result = Vec::with_capacity(input.len() * 3 / 4);
    let chars: Vec<u8> = input.bytes().collect();
    let mut buffer: u32 = 0;
    let mut bits = 0i32;

    for &byte in &chars {
        let val = if (byte as usize) < 128 {
            B64_DECODE_TABLE[byte as usize]
        } else {
            -1
        };
        if val < 0 {
            return None;
        }
        buffer = (buffer << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }

    Some(result)
}

/// Compute an 8-hex-char hash suffix using FNV-1a.
fn fnv_hash_suffix(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", hash & 0xFFFFFFFF)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ContentBlock, ImageSource, ToolResultContent};

    fn make_user_message(content: Vec<ContentBlock>) -> ApiMessage {
        ApiMessage {
            role: MessageRole::User,
            content,
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    fn make_assistant_message(content: Vec<ContentBlock>) -> ApiMessage {
        ApiMessage {
            role: MessageRole::Assistant,
            content,
            reasoning: None,
            ts: None,
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
    fn test_sanitize_simple_id() {
        assert_eq!(
            sanitize_openai_call_id("toolu_01AbC-xyz_789"),
            "toolu_01AbC-xyz_789"
        );
    }

    #[test]
    fn test_sanitize_strips_invalid_chars() {
        assert_eq!(sanitize_openai_call_id("tool.with.dots"), "tool_with_dots");
    }

    #[test]
    fn test_sanitize_truncates_long_id() {
        let long_id = "call_mcp--posthog--dashboard_create_12345678-1234-1234-1234-123456789012";
        let result = sanitize_openai_call_id(long_id);
        assert!(result.len() <= CALL_ID_MAX_LENGTH);
        assert!(result.contains('_'));
    }

    #[test]
    fn test_sanitize_mcp_tool_id() {
        let mcp_id = "toolu_mcp.server:tool/name_".to_string() + &"x".repeat(50);
        let result = sanitize_openai_call_id(&mcp_id);
        assert!(result.len() <= CALL_ID_MAX_LENGTH);
        for c in result.chars() {
            assert!(c.is_ascii_alphanumeric() || c == '_' || c == '-');
        }
    }

    #[test]
    fn test_convert_text_message() {
        let messages = vec![make_user_message(vec![ContentBlock::Text {
            text: "hello".to_string(),
        }])];

        let result = convert_to_bedrock_converse_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["text"], "hello");
    }

    #[test]
    fn test_convert_tool_use() {
        let messages = vec![make_assistant_message(vec![ContentBlock::ToolUse {
            id: "call_123".to_string(),
            name: "read_file".to_string(),
            input: json!({"path": "test.rs"}),
        }])];

        let result = convert_to_bedrock_converse_messages(&messages);
        assert_eq!(result[0]["role"], "assistant");
        let tool_use = &result[0]["content"][0]["toolUse"];
        assert_eq!(tool_use["name"], "read_file");
        assert_eq!(tool_use["toolUseId"], "call_123");
    }

    #[test]
    fn test_convert_tool_result() {
        let messages = vec![make_user_message(vec![ContentBlock::ToolResult {
            tool_use_id: "call_123".to_string(),
            content: vec![ToolResultContent::Text {
                text: "file contents".to_string(),
            }],
            is_error: None,
        }])];

        let result = convert_to_bedrock_converse_messages(&messages);
        let tool_result = &result[0]["content"][0]["toolResult"];
        assert_eq!(tool_result["toolUseId"], "call_123");
        assert_eq!(tool_result["status"], "success");
        let content = tool_result["content"].as_array().unwrap();
        assert_eq!(content[0]["text"], "file contents");
    }

    #[test]
    fn test_convert_image_base64() {
        // "hello" in base64 = "aGVsbG8="
        let messages = vec![make_user_message(vec![ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: "image/png".to_string(),
                data: "aGVsbG8=".to_string(),
            },
        }])];

        let result = convert_to_bedrock_converse_messages(&messages);
        let image_block = &result[0]["content"][0]["image"];
        assert_eq!(image_block["format"], "png");
        assert!(image_block["source"]["bytes"].is_array());
        // Verify decoded bytes: "hello" = [104, 101, 108, 108, 111]
        let bytes: Vec<u8> = image_block["source"]["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_u64().map(|b| b as u8))
            .collect();
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn test_skips_thinking_blocks() {
        let messages = vec![make_assistant_message(vec![
            ContentBlock::Thinking {
                thinking: "thoughts".to_string(),
                signature: "sig".to_string(),
            },
            ContentBlock::Text {
                text: "response".to_string(),
            },
        ])];

        let result = convert_to_bedrock_converse_messages(&messages);
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["text"], "response");
    }

    #[test]
    fn test_base64_decode() {
        // Empty
        assert_eq!(base64_decode(""), Some(Vec::new()));
        // "hello"
        assert_eq!(base64_decode("aGVsbG8="), Some(b"hello".to_vec()));
        // "Hello, World!"
        assert_eq!(
            base64_decode("SGVsbG8sIFdvcmxkIQ=="),
            Some(b"Hello, World!".to_vec())
        );
        // Invalid char
        assert!(base64_decode("aGVsbG8!").is_none());
    }
}
