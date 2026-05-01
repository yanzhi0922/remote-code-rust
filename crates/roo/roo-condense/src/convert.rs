//! Tool block conversion utilities.
//!
//! Converts `tool_use` and `tool_result` content blocks to text representations,
//! allowing conversations to be summarized without requiring the `tools` parameter.
//!
//! Source: `src/core/condense/index.ts` — `toolUseToText`, `toolResultToText`,
//! `convertToolBlocksToText`, `extractCommandBlocks`

use regex::Regex;
use roo_types::api::{ContentBlock, ToolResultContent};

/// Converts a `tool_use` block to a text representation.
///
/// Source: `src/core/condense/index.ts` — `toolUseToText`
pub fn tool_use_to_text(name: &str, input: &serde_json::Value) -> String {
    let input_text = if input.is_object() {
        // Format each key-value pair
        input
            .as_object()
            .map(|obj| {
                obj.iter()
                    .map(|(key, value)| {
                        let formatted_value = if value.is_object() || value.is_array() {
                            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
                        } else {
                            match value {
                                serde_json::Value::String(s) => s.clone(),
                                _ => value.to_string(),
                            }
                        };
                        format!("{key}: {formatted_value}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| input.to_string())
    } else {
        input.to_string()
    };

    format!("[Tool Use: {name}]\n{input_text}")
}

/// Converts a `tool_result` block to a text representation.
///
/// Source: `src/core/condense/index.ts` — `toolResultToText`
pub fn tool_result_to_text(content: &[ToolResultContent], is_error: Option<bool>) -> String {
    let error_suffix = if is_error.unwrap_or(false) {
        " (Error)"
    } else {
        ""
    };

    if content.is_empty() {
        return format!("[Tool Result{error_suffix}]");
    }

    let content_text = content
        .iter()
        .map(|block| match block {
            ToolResultContent::Text { text } => text.clone(),
            ToolResultContent::Image { .. } => "[Image]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("[Tool Result{error_suffix}]\n{content_text}")
}

/// Converts all `tool_use` and `tool_result` blocks in content to text representations.
///
/// This is necessary for providers like Bedrock that require the `tools` parameter when
/// tool blocks are present. By converting to text, we can send the conversation for
/// summarization without the `tools` parameter.
///
/// Source: `src/core/condense/index.ts` — `convertToolBlocksToText`
pub fn convert_tool_blocks_to_text(content: &[ContentBlock]) -> Vec<ContentBlock> {
    content
        .iter()
        .map(|block| match block {
            ContentBlock::ToolUse { name, input, .. } => ContentBlock::Text {
                text: tool_use_to_text(name, input),
            },
            ContentBlock::ToolResult {
                content,
                is_error,
                ..
            } => ContentBlock::Text {
                text: tool_result_to_text(content, *is_error),
            },
            other => other.clone(),
        })
        .collect()
}

/// Extracts `<command>` blocks from a message's content.
///
/// These blocks represent active workflows that must be preserved across condensings.
///
/// Source: `src/core/condense/index.ts` — `extractCommandBlocks`
pub fn extract_command_blocks(content: &[ContentBlock]) -> String {
    // Concatenate all text blocks
    let text: String = content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        return String::new();
    }

    // Match all <command> blocks including their content
    let Ok(re) = Regex::new(r"<command[^>]*>[\s\S]*?</command>") else {
        return String::new();
    };

    let matches: Vec<&str> = re.find_iter(&text).map(|m| m.as_str()).collect();

    if matches.is_empty() {
        String::new()
    } else {
        matches.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::ToolResultContent;

    #[test]
    fn test_tool_use_to_text_simple() {
        let input = serde_json::json!({"path": "/foo/bar.txt"});
        let result = tool_use_to_text("read_file", &input);
        assert!(result.contains("[Tool Use: read_file]"));
        assert!(result.contains("path: /foo/bar.txt"));
    }

    #[test]
    fn test_tool_use_to_text_nested() {
        let input = serde_json::json!({"options": {"recursive": true}});
        let result = tool_use_to_text("list_files", &input);
        assert!(result.contains("[Tool Use: list_files]"));
        assert!(result.contains("options:"));
    }

    #[test]
    fn test_tool_result_to_text_string_content() {
        let content = vec![ToolResultContent::Text {
            text: "file contents here".to_string(),
        }];
        let result = tool_result_to_text(&content, None);
        assert_eq!(result, "[Tool Result]\nfile contents here");
    }

    #[test]
    fn test_tool_result_to_text_error() {
        let content = vec![ToolResultContent::Text {
            text: "file not found".to_string(),
        }];
        let result = tool_result_to_text(&content, Some(true));
        assert_eq!(result, "[Tool Result (Error)]\nfile not found");
    }

    #[test]
    fn test_tool_result_to_text_empty() {
        let result = tool_result_to_text(&[], None);
        assert_eq!(result, "[Tool Result]");
    }

    #[test]
    fn test_convert_tool_blocks_to_text() {
        let blocks = vec![
            ContentBlock::Text {
                text: "hello".to_string(),
            },
            ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "/test.txt"}),
            },
            ContentBlock::ToolResult {
                tool_use_id: "tool_1".to_string(),
                content: vec![ToolResultContent::Text {
                    text: "contents".to_string(),
                }],
                is_error: None,
            },
        ];
        let result = convert_tool_blocks_to_text(&blocks);
        assert_eq!(result.len(), 3);
        // First block unchanged
        assert!(matches!(&result[0], ContentBlock::Text { text } if text == "hello"));
        // Tool use converted to text
        assert!(matches!(&result[1], ContentBlock::Text { text } if text.contains("[Tool Use: read_file]")));
        // Tool result converted to text
        assert!(matches!(&result[2], ContentBlock::Text { text } if text.contains("[Tool Result]")));
    }

    #[test]
    fn test_extract_command_blocks_with_commands() {
        let content = vec![ContentBlock::Text {
            text: "Some text\n<command name=\"test\">echo hello</command>\nmore text".to_string(),
        }];
        let result = extract_command_blocks(&content);
        assert!(result.contains("<command name=\"test\">echo hello</command>"));
    }

    #[test]
    fn test_extract_command_blocks_no_commands() {
        let content = vec![ContentBlock::Text {
            text: "Just regular text".to_string(),
        }];
        let result = extract_command_blocks(&content);
        assert!(result.is_empty());
    }
}
