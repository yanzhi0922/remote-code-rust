//! Export Markdown
//!
//! Exports task conversations to Markdown format.
//! Mirrors `export-markdown.ts`.

use std::path::Path;

use chrono::{Datelike, Timelike};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Extended content block types for markdown export.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "tool_use")]
    ToolUse {
        name: String,
        input: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
    #[serde(rename = "reasoning")]
    Reasoning { text: String },
    #[serde(rename = "thoughtSignature")]
    ThoughtSignature,
}

/// A conversation message for export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: Value,
}

// ---------------------------------------------------------------------------
// Export functions
// ---------------------------------------------------------------------------

/// Generate a task file name from a timestamp.
///
/// Source: `export-markdown.ts` — `getTaskFileName`
pub fn get_task_file_name(date_ts: i64) -> String {
    use chrono::{TimeZone, Utc};

    let dt = Utc.timestamp_opt(date_ts, 0).single().unwrap_or_else(|| Utc::now());

    let month = dt.format("%b").to_string().to_lowercase();
    let day = dt.day();
    let year = dt.year();
    let mut hours = dt.hour();
    let minutes = dt.format("%M");
    let seconds = dt.format("%S");
    let ampm = if hours >= 12 { "pm" } else { "am" };
    hours = hours % 12;
    if hours == 0 {
        hours = 12;
    }

    format!(
        "roo_task_{}-{}-{}_{}-{}-{}-{}.md",
        month, day, year, hours, minutes, seconds, ampm
    )
}

/// Format a content block to markdown string.
///
/// Source: `export-markdown.ts` — `formatContentBlockToMarkdown`
pub fn format_content_block_to_markdown(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text { text } => text.clone(),
        ContentBlock::Image => "[Image]".to_string(),
        ContentBlock::ToolUse { name, input, .. } => {
            let input_str = if input.is_object() {
                input
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| {
                        let formatted_key = capitalize_first(key);
                        let formatted_value = if value.is_object() || value.is_array() {
                            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
                        } else {
                            value.to_string()
                        };
                        format!("{}: {}", formatted_key, formatted_value)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                input.to_string()
            };
            format!("[Tool Use: {}]\n{}", name, input_str)
        }
        ContentBlock::ToolResult {
            content,
            is_error,
            ..
        } => {
            let error_suffix = if is_error.unwrap_or(false) {
                " (Error)"
            } else {
                ""
            };
            match content {
                Some(Value::String(s)) => format!("[Tool{}]\n{}", error_suffix, s),
                Some(Value::Array(arr)) => {
                    let parts: Vec<String> = arr
                        .iter()
                        .map(|v| format_value_to_markdown(v))
                        .collect();
                    format!("[Tool{}]\n{}", error_suffix, parts.join("\n"))
                }
                _ => format!("[Tool{}]", error_suffix),
            }
        }
        ContentBlock::Reasoning { text } => format!("[Reasoning]\n{}", text),
        ContentBlock::ThoughtSignature => String::new(),
    }
}

/// Convert conversation history to markdown.
///
/// Source: `export-markdown.ts` — `downloadTask`
pub fn conversation_to_markdown(messages: &[ConversationMessage]) -> String {
    messages
        .iter()
        .map(|message| {
            let role = if message.role == "user" {
                "**User:**"
            } else {
                "**Assistant:**"
            };

            let content = if message.content.is_array() {
                message
                    .content
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|block| format_value_to_markdown(block))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                message.content.to_string()
            };

            format!("{}\n\n{}\n\n", role, content)
        })
        .collect::<Vec<_>>()
        .join("---\n\n")
}

/// Write markdown content to a file.
pub async fn write_markdown_to_file(
    file_path: &Path,
    messages: &[ConversationMessage],
) -> std::io::Result<()> {
    let markdown = conversation_to_markdown(messages);
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(file_path, &markdown).await
}

/// Find the tool name for a given tool call ID.
///
/// Source: `export-markdown.ts` — `findToolName`
pub fn find_tool_name(tool_call_id: &str, messages: &[ConversationMessage]) -> String {
    for message in messages {
        if message.content.is_array() {
            for block in message.content.as_array().unwrap() {
                if let Some(block_type) = block.get("type") {
                    if block_type == "tool_use" {
                        if block.get("id").and_then(|v| v.as_str()) == Some(tool_call_id) {
                            return block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown Tool")
                                .to_string();
                        }
                    }
                }
            }
        }
    }
    "Unknown Tool".to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn format_value_to_markdown(value: &Value) -> String {
    if let Some(obj) = value.as_object() {
        if let Some(block_type) = obj.get("type").and_then(|v| v.as_str()) {
            match block_type {
                "text" => obj
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                "image" => "[Image]".to_string(),
                "tool_use" => {
                    let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                    let input = obj.get("input").cloned().unwrap_or(Value::Null);
                    format!("[Tool Use: {}]\n{}", name, input)
                }
                "reasoning" => obj
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(|t| format!("[Reasoning]\n{}", t))
                    .unwrap_or_default(),
                _ => format!("[Unexpected content type: {}]", block_type),
            }
        } else {
            value.to_string()
        }
    } else {
        value.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_task_file_name() {
        // 2024-01-15 14:30:45 UTC
        let name = get_task_file_name(1705327845);
        assert!(name.starts_with("roo_task_"));
        assert!(name.ends_with(".md"));
        assert!(name.contains("jan"));
    }

    #[test]
    fn test_format_text_block() {
        let block = ContentBlock::Text {
            text: "Hello world".to_string(),
        };
        assert_eq!(format_content_block_to_markdown(&block), "Hello world");
    }

    #[test]
    fn test_format_image_block() {
        let block = ContentBlock::Image;
        assert_eq!(format_content_block_to_markdown(&block), "[Image]");
    }

    #[test]
    fn test_format_tool_use_block() {
        let block = ContentBlock::ToolUse {
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "/test.txt"}),
            id: None,
        };
        let result = format_content_block_to_markdown(&block);
        assert!(result.contains("[Tool Use: read_file]"));
        assert!(result.contains("Path: \"/test.txt\""));
    }

    #[test]
    fn test_format_reasoning_block() {
        let block = ContentBlock::Reasoning {
            text: "Thinking...".to_string(),
        };
        let result = format_content_block_to_markdown(&block);
        assert!(result.contains("[Reasoning]"));
        assert!(result.contains("Thinking..."));
    }

    #[test]
    fn test_conversation_to_markdown() {
        let messages = vec![
            ConversationMessage {
                role: "user".to_string(),
                content: Value::String("Hello".to_string()),
            },
            ConversationMessage {
                role: "assistant".to_string(),
                content: Value::String("Hi there".to_string()),
            },
        ];
        let md = conversation_to_markdown(&messages);
        assert!(md.contains("**User:**"));
        assert!(md.contains("**Assistant:**"));
        assert!(md.contains("Hello"));
        assert!(md.contains("Hi there"));
    }

    #[test]
    fn test_capitalize_first() {
        assert_eq!(capitalize_first("hello"), "Hello");
        assert_eq!(capitalize_first(""), "");
    }

    #[test]
    fn test_find_tool_name() {
        let messages = vec![ConversationMessage {
            role: "assistant".to_string(),
            content: serde_json::json!([
                {"type": "tool_use", "id": "123", "name": "read_file"}
            ]),
        }];
        assert_eq!(find_tool_name("123", &messages), "read_file");
        assert_eq!(find_tool_name("456", &messages), "Unknown Tool");
    }
}
