//! Tiktoken-based token counting with content block serialization.
//!
//! Derived from `src/utils/tiktoken.ts`.
//!
//! Provides accurate token counting for content blocks using the o200k_base
//! tokenizer encoding (GPT-4o family). Falls back to character-based estimation
//! when the tiktoken library fails to initialize.

use std::sync::LazyLock;

use roo_types::api::{ContentBlock, ImageSource, ToolResultContent};
use tiktoken_rs::CoreBPE;

/// Fudge factor to account for tiktoken not always being accurate.
/// Source: `src/utils/tiktoken.ts` -- `TOKEN_FUDGE_FACTOR`
const TOKEN_FUDGE_FACTOR: f64 = 1.5;

/// Conservative estimate for unknown images.
const DEFAULT_IMAGE_TOKENS: u64 = 300;

/// Lazily initialized BPE tokenizer using o200k_base encoding (GPT-4o family).
/// Falls back to `cl100k_base` if o200k_base fails, and ultimately to `None`
/// if neither can be loaded.
static BPE: LazyLock<Option<CoreBPE>> = LazyLock::new(|| {
    tiktoken_rs::o200k_base()
        .ok()
        .or_else(|| tiktoken_rs::cl100k_base().ok())
});

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Counts tokens for a slice of content blocks.
///
/// Source: `src/utils/tiktoken.ts` -- `tiktoken`
///
/// Uses the o200k_base encoding (GPT-4o family) with a fudge factor of 1.5x
/// to account for encoding differences between tiktoken and actual API tokenizers.
///
/// For images, uses `ceil(sqrt(base64_data_length))` for base64 images
/// or a conservative estimate of 300 tokens.
///
/// For tool_use and tool_result blocks, serializes them to text first
/// before counting tokens.
pub async fn count_tokens(content: &[ContentBlock]) -> u64 {
    if content.is_empty() {
        return 0;
    }

    let mut total_tokens: u64 = 0;

    for block in content {
        total_tokens += count_block_tokens(block);
    }

    // Apply fudge factor
    (total_tokens as f64 * TOKEN_FUDGE_FACTOR).ceil() as u64
}

/// Counts tokens for a single content block.
fn count_block_tokens(block: &ContentBlock) -> u64 {
    match block {
        ContentBlock::Text { text } => {
            if text.is_empty() {
                0
            } else {
                count_tokens_for_text(text)
            }
        }
        ContentBlock::Image { source } => count_image_tokens(source),
        ContentBlock::ToolUse { name, input, .. } => {
            let serialized = serialize_tool_use(name, input);
            if serialized.is_empty() {
                0
            } else {
                count_tokens_for_text(&serialized)
            }
        }
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } => {
            let serialized = serialize_tool_result(tool_use_id, content, *is_error);
            if serialized.is_empty() {
                0
            } else {
                count_tokens_for_text(&serialized)
            }
        }
        ContentBlock::Thinking { thinking, .. } => {
            if thinking.is_empty() {
                0
            } else {
                count_tokens_for_text(thinking)
            }
        }
        ContentBlock::RedactedThinking { data } => {
            // Redacted thinking is opaque, estimate based on data length
            (data.len() as f64 / 4.0).ceil() as u64
        }
    }
}

/// Counts tokens for a text string using real BPE encoding.
///
/// Uses the `o200k_base` encoding (same as the TS tiktoken implementation).
/// Falls back to chars/4 heuristic if BPE encoding fails.
fn count_tokens_for_text(text: &str) -> u64 {
    if let Some(bpe) = BPE.as_ref() {
        let tokens = bpe.encode_with_special_tokens(text);
        tokens.len() as u64
    } else {
        // Fallback: simple heuristic ~4 characters per token
        fallback_estimate(text)
    }
}

/// Fallback token estimation using character-based heuristic.
///
/// Uses ~4 chars/token approximation when BPE encoding is unavailable.
fn fallback_estimate(text: &str) -> u64 {
    (text.len() as u64).div_ceil(4)
}

/// Counts tokens for an image source.
fn count_image_tokens(source: &ImageSource) -> u64 {
    match source {
        ImageSource::Base64 { data, .. } => {
            let data_len = data.len() as f64;
            (data_len.sqrt().ceil()) as u64
        }
        ImageSource::Url { .. } => DEFAULT_IMAGE_TOKENS,
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Serializes a tool_use block to text for token counting.
///
/// Source: `src/utils/tiktoken.ts` -- `serializeToolUse`
fn serialize_tool_use(name: &str, input: &serde_json::Value) -> String {
    let mut parts = vec![format!("Tool: {name}")];
    if !input.is_null() {
        match serde_json::to_string(input) {
            Ok(s) => parts.push(format!("Arguments: {s}")),
            Err(_) => parts.push("Arguments: [serialization error]".to_string()),
        }
    }
    parts.join("\n")
}

/// Serializes a tool_result block to text for token counting.
///
/// Source: `src/utils/tiktoken.ts` -- `serializeToolResult`
fn serialize_tool_result(
    tool_use_id: &str,
    content: &[ToolResultContent],
    is_error: Option<bool>,
) -> String {
    let mut parts = vec![format!("Tool Result ({tool_use_id})")];

    if is_error.unwrap_or(false) {
        parts.push("[Error]".to_string());
    }

    for item in content {
        match item {
            ToolResultContent::Text { text } => parts.push(text.clone()),
            ToolResultContent::Image { .. } => parts.push("[Image content]".to_string()),
        }
    }

    parts.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_count_tokens_empty() {
        let content: Vec<ContentBlock> = vec![];
        assert_eq!(count_tokens(&content).await, 0);
    }

    #[tokio::test]
    async fn test_count_tokens_text() {
        let content = vec![ContentBlock::Text {
            text: "Hello, world!".to_string(),
        }];
        let tokens = count_tokens(&content).await;
        // With real BPE, "Hello, world!" is ~4 tokens, * 1.5 = 6
        assert!(tokens > 0);
        assert!(tokens >= 3); // BPE should produce at least 3 tokens
    }

    #[tokio::test]
    async fn test_count_tokens_long_text() {
        let long_text = "The quick brown fox jumps over the lazy dog. ".repeat(10);
        let content = vec![ContentBlock::Text {
            text: long_text.clone(),
        }];
        let tokens = count_tokens(&content).await;
        // BPE should give a more accurate count than chars/4
        // For 440 chars, BPE produces roughly 80-90 tokens
        assert!(tokens > 0);
        // Verify it's less than chars/4 * fudge (which would be ~165)
        let chars_per_4 = (long_text.len() as f64 / 4.0 * TOKEN_FUDGE_FACTOR).ceil() as u64;
        assert!(tokens <= chars_per_4 * 2); // generous bound
    }

    #[tokio::test]
    async fn test_count_tokens_image_base64() {
        let content = vec![ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: "image/png".to_string(),
                data: "aGVsbG8gd29ybGQ=".to_string(), // 16 chars
            },
        }];
        let tokens = count_tokens(&content).await;
        assert!(tokens > 0);
    }

    #[tokio::test]
    async fn test_count_tokens_image_url() {
        let content = vec![ContentBlock::Image {
            source: ImageSource::Url {
                url: "https://example.com/image.png".to_string(),
            },
        }];
        let tokens = count_tokens(&content).await;
        // URL image should use DEFAULT_IMAGE_TOKENS * fudge factor
        assert!(tokens >= 300);
    }

    #[tokio::test]
    async fn test_count_tokens_tool_use() {
        let content = vec![ContentBlock::ToolUse {
            id: "tool_1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "/test.txt"}),
        }];
        let tokens = count_tokens(&content).await;
        assert!(tokens > 0);
    }

    #[tokio::test]
    async fn test_count_tokens_tool_result() {
        let content = vec![ContentBlock::ToolResult {
            tool_use_id: "tool_1".to_string(),
            content: vec![ToolResultContent::Text {
                text: "File contents here".to_string(),
            }],
            is_error: None,
        }];
        let tokens = count_tokens(&content).await;
        assert!(tokens > 0);
    }

    #[tokio::test]
    async fn test_count_tokens_thinking() {
        let content = vec![ContentBlock::Thinking {
            thinking: "Let me analyze this...".to_string(),
            signature: "sig123".to_string(),
        }];
        let tokens = count_tokens(&content).await;
        assert!(tokens > 0);
    }

    #[test]
    fn test_serialize_tool_use() {
        let result = serialize_tool_use("read_file", &serde_json::json!({"path": "/test.txt"}));
        assert!(result.contains("Tool: read_file"));
        assert!(result.contains("Arguments:"));
    }

    #[test]
    fn test_serialize_tool_result() {
        let result = serialize_tool_result(
            "tool_1",
            &[ToolResultContent::Text {
                text: "content".to_string(),
            }],
            None,
        );
        assert!(result.contains("Tool Result (tool_1)"));
        assert!(result.contains("content"));
    }

    #[test]
    fn test_serialize_tool_result_error() {
        let result = serialize_tool_result(
            "tool_1",
            &[ToolResultContent::Text {
                text: "error msg".to_string(),
            }],
            Some(true),
        );
        assert!(result.contains("[Error]"));
    }

    #[test]
    fn test_count_tokens_for_text_bpe() {
        // "Hello" should be 1 token in BPE
        let tokens = count_tokens_for_text("Hello");
        assert!(tokens >= 1);

        // "Hello, world!" should be ~4 tokens in o200k_base
        let tokens = count_tokens_for_text("Hello, world!");
        assert!(tokens >= 2);
    }

    #[test]
    fn test_count_tokens_for_text_empty() {
        assert_eq!(count_tokens_for_text(""), 0);
    }

    #[test]
    fn test_fallback_estimate() {
        // "Hello" = 5 chars, 5/4 = 1.25, ceil = 2
        assert_eq!(fallback_estimate("Hello"), 2);
        // "Hello, world!" = 13 chars, 13/4 = 3.25, ceil = 4
        assert_eq!(fallback_estimate("Hello, world!"), 4);
        // Empty string
        assert_eq!(fallback_estimate(""), 0);
    }

    #[test]
    fn test_bpe_initializer_succeeds() {
        // Verify that the BPE tokenizer initialized successfully
        assert!(BPE.is_some(), "BPE tokenizer should initialize successfully");
    }
}
