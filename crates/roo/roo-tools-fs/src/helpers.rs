//! Helper functions for file system tools.

use crate::types::FsToolError;
use roo_ignore::RooIgnoreController;

/// Strip leading line numbers from content that may have been formatted as
/// `  1 | content` or `1→content` or similar patterns.
pub fn strip_line_numbers(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    for line in content.lines() {
        let stripped = strip_single_line_number(line);
        result.push_str(stripped);
        result.push('\n');
    }
    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') && !result.is_empty() {
        result.pop();
    }
    result
}

fn strip_single_line_number(line: &str) -> &str {
    let trimmed = line.trim_start();
    // Try to parse leading digits followed by optional separator
    let digits_end = trimmed
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(trimmed.len());

    if digits_end == 0 {
        return line;
    }

    let rest = &trimmed[digits_end..];

    // Check for common separators: " | ", "->", ":\t", ") "
    if rest.starts_with(" | ") {
        return &rest[3..];
    }
    if rest.starts_with("\u{2192}") {
        // → arrow
        return &rest[3..]; // UTF-8 3 bytes
    }
    if rest.starts_with(":\t") {
        return &rest[2..];
    }
    if rest.starts_with(") ") {
        return &rest[2..];
    }

    // No recognized separator, return original line
    line
}

/// Unescape common HTML entities in a string.
///
/// Handles: `&`, `<`, `>`, `"`, `'`, `'`,
/// and numeric entities like `&#60;` and `&#x3c;`.
pub fn unescape_html_entities(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '&' {
            let entity = consume_html_entity(&mut chars);
            match entity.as_str() {
                "amp" => result.push('&'),
                "lt" => result.push('<'),
                "gt" => result.push('>'),
                "quot" => result.push('"'),
                "apos" | "#39" => result.push('\''),
                s if s.starts_with("#x") || s.starts_with("#X") => {
                    if let Ok(code) = u32::from_str_radix(&s[2..], 16) {
                        if let Some(c) = char::from_u32(code) {
                            result.push(c);
                        } else {
                            result.push('&');
                            result.push_str(s);
                            result.push(';');
                        }
                    } else {
                        result.push('&');
                        result.push_str(s);
                        result.push(';');
                    }
                }
                s if s.starts_with('#') => {
                    if let Ok(code) = s[1..].parse::<u32>() {
                        if let Some(c) = char::from_u32(code) {
                            result.push(c);
                        } else {
                            result.push('&');
                            result.push_str(s);
                            result.push(';');
                        }
                    } else {
                        result.push('&');
                        result.push_str(s);
                        result.push(';');
                    }
                }
                s => {
                    result.push('&');
                    result.push_str(s);
                    result.push(';');
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn consume_html_entity(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut entity = String::new();
    while let Some(&ch) = chars.peek() {
        if ch == ';' {
            chars.next();
            break;
        }
        entity.push(ch);
        chars.next();
    }
    entity
}

/// Create all parent directories for a given file path.
pub fn create_directories_for_file(path: &std::path::Path) -> Result<(), FsToolError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(FsToolError::Io)?;
        }
    }
    Ok(())
}

/// Check if content appears to be binary (non-text).
///
/// Uses a heuristic: if the first 8KB contain null bytes or the data
/// is not valid UTF-8, it's considered binary.
pub fn is_binary_content(data: &[u8]) -> bool {
    // Check for null bytes
    if data.contains(&0) {
        return true;
    }

    // Check if valid UTF-8
    if std::str::from_utf8(data).is_err() {
        return true;
    }

    false
}

/// Strip markdown code fences from content.
///
/// If the content starts with ``` and ends with ```, removes them.
/// Also handles optional language specifier on the opening fence.
pub fn strip_markdown_fences(content: &str) -> String {
    let trimmed = content.trim();

    if !trimmed.starts_with("```") {
        return content.to_string();
    }

    // Find the end of the first line (opening fence)
    let first_newline = match trimmed.find('\n') {
        Some(pos) => pos,
        None => return content.to_string(),
    };

    // Check if it ends with ```
    let inner = trimmed[first_newline + 1..].trim_end();
    if !inner.ends_with("```") {
        return content.to_string();
    }

    inner[..inner.len() - 3].to_string()
}

/// Add line numbers to content in the format `  1 | line`.
pub fn add_line_numbers(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let max_line = lines.len();
    let width = format!("{max_line}").len();

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        result.push_str(&format!("{:>width$} | {line}\n", line_num, width = width));
    }
    result
}

/// Truncate lines that exceed the maximum line length.
/// The ellipsis character is appended to truncated lines.
pub fn truncate_long_lines(content: &str, max_length: usize) -> String {
    let mut result = String::with_capacity(content.len());
    for line in content.lines() {
        if line.len() > max_length {
            // Truncate to max_length chars and add ellipsis
            result.push_str(&line[..max_length]);
            result.push_str("...");
            result.push('\n');
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    if !content.ends_with('\n') && !result.is_empty() {
        result.pop();
    }
    result
}

/// Extract a slice of lines from content (1-based offset, inclusive).
pub fn slice_lines(content: &str, start_line: usize, max_lines: usize) -> (String, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    if start_line > total || total == 0 {
        return (String::new(), total);
    }

    let start_idx = start_line.saturating_sub(1);
    let end_idx = std::cmp::min(start_idx + max_lines, total);

    let sliced: Vec<&str> = lines[start_idx..end_idx].to_vec();
    let result = sliced.join("\n");

    (result, total)
}

/// Check if every non-empty line in the content has a line number prefix
/// in the format `  1 | content`.
pub fn every_line_has_line_numbers(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return false;
    }
    lines
        .iter()
        .all(|line| line.trim().is_empty() || has_line_number_prefix(line))
}

/// Check if a single line has a line number prefix like `  1 | `.
fn has_line_number_prefix(line: &str) -> bool {
    let trimmed = line.trim_start();
    let digits_end = trimmed
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(trimmed.len());
    if digits_end == 0 {
        return false;
    }
    let rest = &trimmed[digits_end..];
    rest.starts_with(" | ") || rest.starts_with("\u{2192}") || rest.starts_with(":\t")
}

/// Check if a file path is blocked by .rooignore rules.
///
/// Returns `Ok(())` if the path is accessible (or no controller is provided).
/// Returns `Err(FsToolError::AccessDenied)` if the path is ignored.
///
/// This function is called by file operation tools (read_file, write_to_file,
/// edit_file, apply_diff) before performing any file I/O, matching the TS
/// behavior where `.rooignore` validation is enforced at the tool level.
pub fn check_roo_ignore(
    path: &str,
    controller: Option<&RooIgnoreController>,
) -> Result<(), FsToolError> {
    if let Some(ctrl) = controller {
        if !ctrl.validate_access(path) {
            return Err(FsToolError::AccessDenied(format!(
                "Access to '{}' is blocked by .rooignore rules. \
                 This file or directory has been excluded from access. \
                 If you believe this is an error, check the .rooignore file \
                 at the project root.",
                path
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- strip_line_numbers tests ----

    #[test]
    fn test_strip_line_numbers_with_pipe() {
        let input = "  1 | hello\n  2 | world\n";
        let expected = "hello\nworld\n";
        assert_eq!(strip_line_numbers(input), expected);
    }

    #[test]
    fn test_strip_line_numbers_without_numbers() {
        let input = "hello\nworld\n";
        assert_eq!(strip_line_numbers(input), input);
    }

    #[test]
    fn test_strip_line_numbers_mixed() {
        let input = "  1 | first\nsecond line\n  3 | third\n";
        let expected = "first\nsecond line\nthird\n";
        assert_eq!(strip_line_numbers(input), expected);
    }

    #[test]
    fn test_strip_line_numbers_arrow() {
        // Using the actual UTF-8 arrow character
        let input = "1\u{2192}hello\n2\u{2192}world\n";
        let expected = "hello\nworld\n";
        assert_eq!(strip_line_numbers(input), expected);
    }

    #[test]
    fn test_strip_line_numbers_paren() {
        let input = "1) hello\n2) world\n";
        let expected = "hello\nworld\n";
        assert_eq!(strip_line_numbers(input), expected);
    }

    #[test]
    fn test_strip_line_numbers_empty() {
        assert_eq!(strip_line_numbers(""), "");
    }

    // ---- unescape_html_entities tests ----

    #[test]
    fn test_unescape_amp() {
        assert_eq!(unescape_html_entities("&amp;"), "&");
    }

    #[test]
    fn test_unescape_lt_gt() {
        assert_eq!(unescape_html_entities("<script>"), "<script>");
    }

    #[test]
    fn test_unescape_quot() {
        let input = format!("{}quot;hello{}quot;", '&', '&');
        let expected = "\"hello\"";
        assert_eq!(unescape_html_entities(&input), expected);
    }

    #[test]
    fn test_unescape_numeric() {
        assert_eq!(unescape_html_entities("&#60;"), "<");
    }

    #[test]
    fn test_unescape_hex() {
        assert_eq!(unescape_html_entities("&#x3c;"), "<");
    }

    #[test]
    fn test_unescape_no_entity() {
        assert_eq!(unescape_html_entities("hello world"), "hello world");
    }

    #[test]
    fn test_unescape_apos() {
        assert_eq!(unescape_html_entities("'"), "'");
    }

    #[test]
    fn test_unescape_unknown_entity() {
        assert_eq!(unescape_html_entities("&unknown;"), "&unknown;");
    }

    // ---- is_binary_content tests ----

    #[test]
    fn test_binary_null_bytes() {
        assert!(is_binary_content(b"hello\x00world"));
    }

    #[test]
    fn test_binary_invalid_utf8() {
        assert!(is_binary_content(&[0xFF, 0xFE, 0xFD]));
    }

    #[test]
    fn test_text_content() {
        assert!(!is_binary_content(b"hello world\nline 2\n"));
    }

    #[test]
    fn test_empty_content() {
        assert!(!is_binary_content(b""));
    }

    // ---- strip_markdown_fences tests ----

    #[test]
    fn test_strip_fences_rust() {
        let input = "```rust\nfn main() {}\n```";
        assert_eq!(strip_markdown_fences(input), "fn main() {}\n");
    }

    #[test]
    fn test_strip_fences_no_fences() {
        let input = "just plain text";
        assert_eq!(strip_markdown_fences(input), "just plain text");
    }

    #[test]
    fn test_strip_fences_no_language() {
        let input = "```\nsome code\n```";
        assert_eq!(strip_markdown_fences(input), "some code\n");
    }

    #[test]
    fn test_strip_fences_incomplete() {
        let input = "```\nsome code";
        assert_eq!(strip_markdown_fences(input), "```\nsome code");
    }

    // ---- add_line_numbers tests ----

    #[test]
    fn test_add_line_numbers_basic() {
        let input = "hello\nworld";
        let result = add_line_numbers(input);
        assert!(result.contains("1 | hello"));
        assert!(result.contains("2 | world"));
    }

    #[test]
    fn test_add_line_numbers_empty() {
        assert_eq!(add_line_numbers(""), "");
    }

    #[test]
    fn test_add_line_numbers_alignment() {
        let input = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let result = add_line_numbers(input);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 10);
        assert!(lines[0].starts_with(" 1 |"));
        assert!(lines[9].starts_with("10 |"));
    }

    // ---- truncate_long_lines tests ----

    #[test]
    fn test_truncate_long_lines_short() {
        let input = "hello\nworld";
        assert_eq!(truncate_long_lines(input, 100), "hello\nworld");
    }

    #[test]
    fn test_truncate_long_lines_long() {
        let long_line = "a".repeat(200);
        let input = format!("{long_line}\nshort");
        let result = truncate_long_lines(&input, 100);
        let lines: Vec<&str> = result.lines().collect();
        // Truncated line should be max_length + "..." = 103 chars
        assert!(lines[0].ends_with("..."));
        assert!(lines[0].len() <= 103);
        assert_eq!(lines[1], "short");
    }

    // ---- slice_lines tests ----

    #[test]
    fn test_slice_lines_basic() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let (result, total) = slice_lines(content, 2, 2);
        assert_eq!(total, 5);
        assert_eq!(result, "line2\nline3");
    }

    #[test]
    fn test_slice_lines_from_start() {
        let content = "a\nb\nc";
        let (result, total) = slice_lines(content, 1, 10);
        assert_eq!(total, 3);
        assert_eq!(result, "a\nb\nc");
    }

    #[test]
    fn test_slice_lines_out_of_range() {
        let content = "a\nb\nc";
        let (result, total) = slice_lines(content, 10, 5);
        assert_eq!(total, 3);
        assert_eq!(result, "");
    }

    #[test]
    fn test_slice_lines_empty() {
        let (result, total) = slice_lines("", 1, 10);
        assert_eq!(total, 0);
        assert_eq!(result, "");
    }

    // ---- create_directories_for_file tests ----

    #[test]
    fn test_create_directories_for_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("a").join("b").join("c").join("test.txt");
        create_directories_for_file(&file_path).unwrap();
        assert!(file_path.parent().unwrap().exists());
    }

    #[test]
    fn test_create_directories_existing() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        create_directories_for_file(&file_path).unwrap();
    }

    #[test]
    fn test_create_directories_no_parent() {
        let file_path = std::path::Path::new("test.txt");
        create_directories_for_file(file_path).unwrap();
    }

    // ---- EditType tests ----

    #[test]
    fn test_edit_type_serde() {
        let create = crate::types::EditType::Create;
        let json = serde_json::to_string(&create).unwrap();
        assert_eq!(json, "\"create\"");

        let modify = crate::types::EditType::Modify;
        let json = serde_json::to_string(&modify).unwrap();
        assert_eq!(json, "\"modify\"");
    }

    // ---- ReadResult tests ----

    #[test]
    fn test_read_result_serde() {
        let result = crate::types::ReadResult {
            content: "hello".to_string(),
            path: "test.txt".to_string(),
            total_lines: 1,
            truncated: false,
            is_binary: false,
            start_line: 1,
            end_line: 1,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: crate::types::ReadResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, "hello");
        assert_eq!(parsed.path, "test.txt");
    }

    // ---- WriteResult tests ----

    #[test]
    fn test_write_result_serde() {
        let result = crate::types::WriteResult {
            path: "test.txt".to_string(),
            is_new_file: true,
            lines_written: 10,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: crate::types::WriteResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_new_file);
        assert_eq!(parsed.lines_written, 10);
    }

    // ---- DiffApplyResult tests ----

    #[test]
    fn test_diff_apply_result_serde() {
        let result = crate::types::DiffApplyResult {
            path: "test.txt".to_string(),
            blocks_applied: 3,
            warnings: vec!["minor issue".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: crate::types::DiffApplyResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.blocks_applied, 3);
        assert_eq!(parsed.warnings.len(), 1);
    }

    // ---- FsToolError tests ----

    #[test]
    fn test_fs_tool_error_display() {
        let err = crate::types::FsToolError::InvalidPath("../etc/passwd".to_string());
        assert_eq!(format!("{err}"), "Invalid path: ../etc/passwd");

        let err = crate::types::FsToolError::BinaryFile("image.png".to_string());
        assert_eq!(format!("{err}"), "Binary file detected: image.png");
    }

    // ---- check_roo_ignore tests ----

    #[test]
    fn test_check_roo_ignore_no_controller() {
        // No controller → always allow
        assert!(check_roo_ignore("secret.txt", None).is_ok());
        assert!(check_roo_ignore("any/path.rs", None).is_ok());
    }

    #[test]
    fn test_check_roo_ignore_allows_accessible_file() {
        let mut ctrl = roo_ignore::RooIgnoreController::new("/tmp");
        ctrl.load_patterns("node_modules/\n*.log");
        assert!(check_roo_ignore("src/main.rs", Some(&ctrl)).is_ok());
        assert!(check_roo_ignore("README.md", Some(&ctrl)).is_ok());
    }

    #[test]
    fn test_check_roo_ignore_blocks_ignored_file() {
        let mut ctrl = roo_ignore::RooIgnoreController::new("/tmp");
        ctrl.load_patterns("node_modules/\n*.log\nsecret.txt");
        assert!(check_roo_ignore("secret.txt", Some(&ctrl)).is_err());
        assert!(check_roo_ignore("debug.log", Some(&ctrl)).is_err());
        assert!(check_roo_ignore("node_modules/react/index.js", Some(&ctrl)).is_err());
    }

    #[test]
    fn test_check_roo_ignore_error_message_contains_path() {
        let mut ctrl = roo_ignore::RooIgnoreController::new("/tmp");
        ctrl.load_patterns("secret.txt");
        let err = check_roo_ignore("secret.txt", Some(&ctrl)).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("secret.txt"));
        assert!(msg.contains(".rooignore"));
    }

    #[test]
    fn test_check_roo_ignore_empty_patterns_allows_all() {
        let ctrl = roo_ignore::RooIgnoreController::new("/tmp");
        // No patterns loaded → all files accessible
        assert!(check_roo_ignore("anything.txt", Some(&ctrl)).is_ok());
    }
}
