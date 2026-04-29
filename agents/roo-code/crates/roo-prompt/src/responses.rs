//! Response formatting utilities.
//!
//! Source: `src/core/prompts/responses.ts`

use serde_json::json;
use similar::TextDiff;

use crate::types::FileEntry;

/// Formats a tool denied response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.toolDenied`
pub fn tool_denied() -> String {
    json!({
        "status": "denied",
        "message": "The user denied this operation."
    })
    .to_string()
}

/// Formats a tool denied with feedback response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.toolDeniedWithFeedback`
pub fn tool_denied_with_feedback(feedback: Option<&str>) -> String {
    json!({
        "status": "denied",
        "feedback": feedback
    })
    .to_string()
}

/// Formats a tool approved with feedback response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.toolApprovedWithFeedback`
pub fn tool_approved_with_feedback(feedback: Option<&str>) -> String {
    json!({
        "status": "approved",
        "feedback": feedback
    })
    .to_string()
}

/// Formats a tool error response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.toolError`
pub fn tool_error(error: Option<&str>) -> String {
    json!({
        "status": "error",
        "message": "The tool execution failed",
        "error": error
    })
    .to_string()
}

/// Formats a .rooignore error response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.rooIgnoreError`
pub fn roo_ignore_error(path: &str) -> String {
    json!({
        "status": "error",
        "type": "access_denied",
        "message": "Access blocked by .rooignore",
        "path": path,
        "suggestion": "Try to continue without this file, or ask the user to update the .rooignore file"
    })
    .to_string()
}

/// The tool use instructions reminder for native tool calling.
///
/// Source: `src/core/prompts/responses.ts` — `toolUseInstructionsReminderNative`
const TOOL_USE_INSTRUCTIONS_REMINDER_NATIVE: &str = r#"# Reminder: Instructions for Tool Use

Tools are invoked using the platform's native tool calling mechanism. Each tool requires specific parameters as defined in the tool descriptions. Refer to the tool definitions provided in your system instructions for the correct parameter structure and usage examples.

Always ensure you provide all required parameters for the tool you wish to use."#;

/// Gets the tool use instructions reminder.
fn get_tool_instructions_reminder() -> &'static str {
    TOOL_USE_INSTRUCTIONS_REMINDER_NATIVE
}

/// Formats a "no tools used" error response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.noToolsUsed`
pub fn no_tools_used() -> String {
    let instructions = get_tool_instructions_reminder();

    format!(
        r#"[ERROR] You did not use a tool in your previous response! Please retry with a tool use.

{instructions}

# Next Steps

If you have completed the user's task, use the attempt_completion tool.
If you require additional information from the user, use the ask_followup_question tool.
Otherwise, if you have not completed the task and do not need additional information, then proceed with the next step of the task.
(This is an automated message, so do not respond to it conversationally.)"#
    )
}

/// Formats a "too many mistakes" guidance response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.tooManyMistakes`
pub fn too_many_mistakes(feedback: Option<&str>) -> String {
    json!({
        "status": "guidance",
        "feedback": feedback
    })
    .to_string()
}

/// Formats a missing tool parameter error response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.missingToolParameterError`
pub fn missing_tool_parameter_error(param_name: &str) -> String {
    let instructions = get_tool_instructions_reminder();
    format!(
        "Missing value for required parameter '{}'. Please retry with complete response.\n\n{}",
        param_name, instructions
    )
}

/// Formats an invalid MCP tool argument error response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.invalidMcpToolArgumentError`
pub fn invalid_mcp_tool_argument_error(server_name: &str, tool_name: &str) -> String {
    json!({
        "status": "error",
        "type": "invalid_argument",
        "message": "Invalid JSON argument",
        "server": server_name,
        "tool": tool_name,
        "suggestion": "Please retry with a properly formatted JSON argument"
    })
    .to_string()
}

/// Formats an unknown MCP tool error response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.unknownMcpToolError`
pub fn unknown_mcp_tool_error(
    server_name: &str,
    tool_name: &str,
    available_tools: &[String],
) -> String {
    json!({
        "status": "error",
        "type": "unknown_tool",
        "message": "Tool does not exist on server",
        "server": server_name,
        "tool": tool_name,
        "available_tools": available_tools,
        "suggestion": "Please use one of the available tools or check if the server is properly configured"
    })
    .to_string()
}

/// Formats an unknown MCP server error response.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.unknownMcpServerError`
pub fn unknown_mcp_server_error(server_name: &str, available_servers: &[String]) -> String {
    json!({
        "status": "error",
        "type": "unknown_server",
        "message": "Server is not configured",
        "server": server_name,
        "available_servers": available_servers
    })
    .to_string()
}

/// Formats a tool result with optional images.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.toolResult`
pub fn tool_result(text: &str, images: Option<&[String]>) -> ToolResult {
    match images {
        Some(imgs) if !imgs.is_empty() => {
            let mut blocks: Vec<ContentBlock> = Vec::new();
            blocks.push(ContentBlock::Text {
                text: text.to_string(),
            });
            for data_url in imgs {
                if let Some(block) = parse_image_data_url(data_url) {
                    blocks.push(block);
                }
            }
            ToolResult::Blocks(blocks)
        }
        _ => ToolResult::Text(text.to_string()),
    }
}

/// Parsed image block from a data URL.
fn parse_image_data_url(data_url: &str) -> Option<ContentBlock> {
    // data:image/png;base64,base64string
    let (rest, base64) = data_url.split_once(',')?;
    let mime_part = rest.strip_prefix("data:")?;
    let mime_type = mime_part.split(';').next()?;
    Some(ContentBlock::Image {
        source: ImageSource {
            source_type: "base64".to_string(),
            media_type: mime_type.to_string(),
            data: base64.to_string(),
        },
    })
}

/// Convert an array of base64 data URLs into image content blocks.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.imageBlocks` + `formatImagesIntoBlocks`
pub fn image_blocks(images: &[String]) -> Vec<ContentBlock> {
    images
        .iter()
        .filter_map(|data_url| parse_image_data_url(data_url))
        .collect()
}

/// Result of a tool call.
#[derive(Debug, Clone)]
pub enum ToolResult {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// Content block for tool results.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text { text: String },
    Image { source: ImageSource },
}

/// Image source for content blocks.
#[derive(Debug, Clone)]
pub struct ImageSource {
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

/// Natural sort comparison that handles numeric parts.
///
/// Matches the TypeScript `localeCompare` with `{ numeric: true, sensitivity: "base" }`
/// behavior: numeric substrings are compared by value, non-numeric by case-insensitive
/// lexicographic order.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    loop {
        let a_ch = a_chars.next();
        let b_ch = b_chars.next();

        match (a_ch, b_ch) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ac), Some(bc)) => {
                let a_is_digit = ac.is_ascii_digit();
                let b_is_digit = bc.is_ascii_digit();

                if a_is_digit && b_is_digit {
                    // Both are digits — consume the full numeric run and compare by value
                    let mut a_num: u64 = ac.to_digit(10).unwrap() as u64;
                    let mut b_num: u64 = bc.to_digit(10).unwrap() as u64;

                    while let Some(&d) = a_chars.peek() {
                        if d.is_ascii_digit() {
                            a_num = a_num * 10 + d.to_digit(10).unwrap() as u64;
                            a_chars.next();
                        } else {
                            break;
                        }
                    }
                    while let Some(&d) = b_chars.peek() {
                        if d.is_ascii_digit() {
                            b_num = b_num * 10 + d.to_digit(10).unwrap() as u64;
                            b_chars.next();
                        } else {
                            break;
                        }
                    }

                    match a_num.cmp(&b_num) {
                        Ordering::Equal => continue,
                        ord => return ord,
                    }
                } else if !a_is_digit && !b_is_digit {
                    // Both non-digit — case-insensitive comparison (sensitivity: "base")
                    let a_lower = ac.to_lowercase().next().unwrap();
                    let b_lower = bc.to_lowercase().next().unwrap();
                    match a_lower.cmp(&b_lower) {
                        Ordering::Equal => continue,
                        ord => return ord,
                    }
                } else {
                    // One digit, one non-digit — digits sort before letters
                    return if a_is_digit {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                }
            }
        }
    }
}

/// Formats a list of files for display.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.formatFilesList`
pub fn format_files_list(entries: &[FileEntry], did_hit_limit: bool) -> String {
    // Sort entries by directory structure
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        let a_parts: Vec<&str> = a.relative_path.split('/').collect();
        let b_parts: Vec<&str> = b.relative_path.split('/').collect();

        for i in 0..std::cmp::min(a_parts.len(), b_parts.len()) {
            if a_parts[i] != b_parts[i] {
                // If one is a directory and the other isn't at this level, sort the directory first
                if i + 1 == a_parts.len() && i + 1 < b_parts.len() {
                    return std::cmp::Ordering::Less;
                }
                if i + 1 == b_parts.len() && i + 1 < a_parts.len() {
                    return std::cmp::Ordering::Greater;
                }
                // Otherwise, sort with natural/numeric comparison (matches TS localeCompare numeric:true)
                return natural_cmp(a_parts[i], b_parts[i]);
            }
        }
        a_parts.len().cmp(&b_parts.len())
    });

    let formatted: Vec<String> = sorted
        .iter()
        .map(|entry| {
            if entry.is_ignored {
                format!("🔒 {}", entry.relative_path)
            } else if entry.is_protected {
                format!("🛡️ {}", entry.relative_path)
            } else {
                entry.relative_path.clone()
            }
        })
        .collect();

    if did_hit_limit {
        format!(
            "{}\n\n(File list truncated. Use list_files on specific subdirectories if you need to explore further.)",
            formatted.join("\n")
        )
    } else if formatted.is_empty() || (formatted.len() == 1 && formatted[0].is_empty()) {
        // Matches TS: rooIgnoreParsed.length === 0 || (rooIgnoreParsed.length === 1 && rooIgnoreParsed[0] === "")
        "No files found.".to_string()
    } else {
        formatted.join("\n")
    }
}

/// Creates a pretty patch (unified diff) between two strings.
///
/// Source: `src/core/prompts/responses.ts` — `formatResponse.createPrettyPatch`
pub fn create_pretty_patch(filename: &str, old_str: Option<&str>, new_str: Option<&str>) -> String {
    let filename_posix = filename.replace('\\', "/");
    let old = old_str.unwrap_or("");
    let new = new_str.unwrap_or("");

    let diff = TextDiff::from_lines(old, new);

    // Use similar's unified_diff with context_radius(3) to match TS behavior
    // (TS uses diff.createPatch with context: 3)
    let output = format!(
        "{}",
        diff.unified_diff()
            .context_radius(3)
            .header(&filename_posix, &filename_posix)
    );

    // TS strips 4 header lines from diff.createPatch output (Index, ===, ---, +++).
    // similar's unified_diff only has 2 header lines (---, +++).
    // Strip them to match the TypeScript behavior of keeping only hunks and content.
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() > 2 {
        lines[2..].join("\n")
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_tool_denied() {
        let result: Value = serde_json::from_str(&tool_denied()).unwrap();
        assert_eq!(result["status"], "denied");
    }

    #[test]
    fn test_tool_error() {
        let result: Value = serde_json::from_str(&tool_error(Some("something went wrong"))).unwrap();
        assert_eq!(result["status"], "error");
        assert_eq!(result["error"], "something went wrong");
    }

    #[test]
    fn test_roo_ignore_error() {
        let result: Value = serde_json::from_str(&roo_ignore_error("/secret/file.txt")).unwrap();
        assert_eq!(result["status"], "error");
        assert_eq!(result["type"], "access_denied");
        assert_eq!(result["path"], "/secret/file.txt");
    }

    #[test]
    fn test_no_tools_used() {
        let result = no_tools_used();
        assert!(result.contains("[ERROR]"));
        assert!(result.contains("attempt_completion"));
    }

    #[test]
    fn test_missing_tool_parameter_error() {
        let result = missing_tool_parameter_error("path");
        assert!(result.contains("path"));
        assert!(result.contains("Missing value"));
    }

    #[test]
    fn test_format_files_list() {
        let entries = vec![
            FileEntry {
                relative_path: "src/main.rs".to_string(),
                is_ignored: false,
                is_protected: false,
            },
            FileEntry {
                relative_path: "src/lib.rs".to_string(),
                is_ignored: false,
                is_protected: true,
            },
            FileEntry {
                relative_path: "secret.txt".to_string(),
                is_ignored: true,
                is_protected: false,
            },
        ];
        let result = format_files_list(&entries, false);
        assert!(result.contains("src/main.rs"));
        assert!(result.contains("🛡️ src/lib.rs"));
        assert!(result.contains("🔒 secret.txt"));
    }

    #[test]
    fn test_format_files_list_truncated() {
        let entries = vec![FileEntry {
            relative_path: "file.txt".to_string(),
            is_ignored: false,
            is_protected: false,
        }];
        let result = format_files_list(&entries, true);
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_format_files_list_empty() {
        let result = format_files_list(&[], false);
        assert_eq!(result, "No files found.");
    }

    #[test]
    fn test_create_pretty_patch() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline2_modified\nline3\n";
        let patch = create_pretty_patch("test.txt", Some(old), Some(new));
        assert!(patch.contains("line2"));
    }

    #[test]
    fn test_natural_cmp_numeric() {
        // Numeric: 2 < 10 (not lexicographic "2" > "10")
        assert_eq!(natural_cmp("file2", "file10"), std::cmp::Ordering::Less);
        assert_eq!(natural_cmp("file10", "file2"), std::cmp::Ordering::Greater);
        assert_eq!(natural_cmp("file2", "file2"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_natural_cmp_case_insensitive() {
        // Case-insensitive (sensitivity: "base")
        assert_eq!(natural_cmp("File", "file"), std::cmp::Ordering::Equal);
        assert_eq!(natural_cmp("ABC", "abc"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_natural_cmp_mixed() {
        // Mixed numeric and non-numeric
        assert_eq!(natural_cmp("a1b", "a2b"), std::cmp::Ordering::Less);
        assert_eq!(natural_cmp("a10b", "a2b"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_format_files_list_numeric_sort() {
        let entries = vec![
            FileEntry { relative_path: "file10.txt".to_string(), is_ignored: false, is_protected: false },
            FileEntry { relative_path: "file2.txt".to_string(), is_ignored: false, is_protected: false },
            FileEntry { relative_path: "file1.txt".to_string(), is_ignored: false, is_protected: false },
        ];
        let result = format_files_list(&entries, false);
        // Should be sorted: file1, file2, file10 (numeric order)
        let pos1 = result.find("file1.txt").unwrap();
        let pos2 = result.find("file2.txt").unwrap();
        let pos10 = result.find("file10.txt").unwrap();
        assert!(pos1 < pos2);
        assert!(pos2 < pos10);
    }

    #[test]
    fn test_format_files_list_single_empty() {
        let entries = vec![FileEntry {
            relative_path: "".to_string(),
            is_ignored: false,
            is_protected: false,
        }];
        let result = format_files_list(&entries, false);
        assert_eq!(result, "No files found.");
    }

    #[test]
    fn test_tool_denied_with_feedback() {
        let result: Value = serde_json::from_str(&tool_denied_with_feedback(Some("try again"))).unwrap();
        assert_eq!(result["status"], "denied");
        assert_eq!(result["feedback"], "try again");
    }

    #[test]
    fn test_tool_approved_with_feedback() {
        let result: Value = serde_json::from_str(&tool_approved_with_feedback(Some("looks good"))).unwrap();
        assert_eq!(result["status"], "approved");
        assert_eq!(result["feedback"], "looks good");
    }

    #[test]
    fn test_invalid_mcp_tool_argument_error() {
        let result: Value = serde_json::from_str(&invalid_mcp_tool_argument_error("server1", "tool1")).unwrap();
        assert_eq!(result["status"], "error");
        assert_eq!(result["type"], "invalid_argument");
        assert_eq!(result["server"], "server1");
        assert_eq!(result["tool"], "tool1");
    }

    #[test]
    fn test_unknown_mcp_tool_error() {
        let result: Value = serde_json::from_str(
            &unknown_mcp_tool_error("server1", "tool1", &[String::from("tool2")])
        ).unwrap();
        assert_eq!(result["status"], "error");
        assert_eq!(result["type"], "unknown_tool");
        assert_eq!(result["server"], "server1");
        assert_eq!(result["tool"], "tool1");
    }

    #[test]
    fn test_unknown_mcp_server_error() {
        let result: Value = serde_json::from_str(
            &unknown_mcp_server_error("server1", &[String::from("server2")])
        ).unwrap();
        assert_eq!(result["status"], "error");
        assert_eq!(result["type"], "unknown_server");
        assert_eq!(result["server"], "server1");
    }

    #[test]
    fn test_too_many_mistakes() {
        let result: Value = serde_json::from_str(&too_many_mistakes(Some("slow down"))).unwrap();
        assert_eq!(result["status"], "guidance");
        assert_eq!(result["feedback"], "slow down");
    }
}
