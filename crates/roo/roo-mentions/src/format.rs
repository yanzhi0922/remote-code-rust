//! Formatting utilities for file read results.
//!
//! Maps to TypeScript source: `src/core/mentions/index.ts` (formatFileReadResult)

/// Default line limit when reading files via mentions.
pub const DEFAULT_LINE_LIMIT: usize = 500;

/// Result of extracting text from a file with metadata.
#[derive(Debug, Clone)]
pub struct ExtractTextResult {
    /// The extracted text content.
    pub content: String,
    /// Total number of lines in the file.
    pub total_lines: usize,
    /// Number of lines returned.
    pub returned_lines: usize,
    /// Whether the content was truncated.
    pub was_truncated: bool,
    /// The range of lines shown `[start, end]` (1-based, inclusive).
    pub lines_shown: Option<(usize, usize)>,
}

/// Formats file content to look like a read_file tool result.
///
/// Includes truncation warning when content is truncated, matching the
/// format used by the TypeScript implementation.
pub fn format_file_read_result(file_path: &str, result: &ExtractTextResult) -> String {
    let header = format!("[read_file for '{}']", file_path);

    if result.was_truncated && result.lines_shown.is_some() {
        let (start, end) = result.lines_shown.unwrap();
        let next_offset = end + 1;
        format!(
            "{header}\n\
             IMPORTANT: File content truncated.\n\
             Status: Showing lines {start}-{end} of {total_lines} total lines.\n\
             To read more: Use the read_file tool with offset={next_offset} and limit={DEFAULT_LINE_LIMIT}.\n\
             \n\
             File: {file_path}\n\
             {content}",
            header = header,
            start = start,
            end = end,
            total_lines = result.total_lines,
            next_offset = next_offset,
            file_path = file_path,
            content = result.content,
        )
    } else {
        format!(
            "{header}\n\
             File: {file_path}\n\
             {content}",
            header = header,
            file_path = file_path,
            content = result.content,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_file_read_result_no_truncation() {
        let result = ExtractTextResult {
            content: "line 1\nline 2\nline 3".to_string(),
            total_lines: 3,
            returned_lines: 3,
            was_truncated: false,
            lines_shown: None,
        };
        let formatted = format_file_read_result("src/main.rs", &result);
        assert!(formatted.starts_with("[read_file for 'src/main.rs']"));
        assert!(formatted.contains("File: src/main.rs"));
        assert!(formatted.contains("line 1\nline 2\nline 3"));
        assert!(!formatted.contains("IMPORTANT: File content truncated"));
    }

    #[test]
    fn test_format_file_read_result_with_truncation() {
        let result = ExtractTextResult {
            content: "line 1\nline 2".to_string(),
            total_lines: 100,
            returned_lines: 2,
            was_truncated: true,
            lines_shown: Some((1, 2)),
        };
        let formatted = format_file_read_result("src/main.rs", &result);
        assert!(formatted.starts_with("[read_file for 'src/main.rs']"));
        assert!(formatted.contains("IMPORTANT: File content truncated"));
        assert!(formatted.contains("Showing lines 1-2 of 100 total lines"));
        assert!(formatted.contains("offset=3"));
        assert!(formatted.contains(&format!("limit={}", DEFAULT_LINE_LIMIT)));
        assert!(formatted.contains("line 1\nline 2"));
    }

    #[test]
    fn test_format_file_read_result_truncated_no_lines_shown() {
        let result = ExtractTextResult {
            content: "content".to_string(),
            total_lines: 50,
            returned_lines: 10,
            was_truncated: true,
            lines_shown: None,
        };
        let formatted = format_file_read_result("test.rs", &result);
        // When was_truncated is true but lines_shown is None, no truncation message
        assert!(!formatted.contains("IMPORTANT: File content truncated"));
        assert!(formatted.contains("[read_file for 'test.rs']"));
    }

    #[test]
    fn test_format_file_read_result_empty_content() {
        let result = ExtractTextResult {
            content: String::new(),
            total_lines: 0,
            returned_lines: 0,
            was_truncated: false,
            lines_shown: None,
        };
        let formatted = format_file_read_result("empty.rs", &result);
        assert!(formatted.contains("[read_file for 'empty.rs']"));
        assert!(formatted.contains("File: empty.rs"));
    }

    #[test]
    fn test_format_file_read_result_path_with_spaces() {
        let result = ExtractTextResult {
            content: "hello".to_string(),
            total_lines: 1,
            returned_lines: 1,
            was_truncated: false,
            lines_shown: None,
        };
        let formatted = format_file_read_result("path/with spaces/file.rs", &result);
        assert!(formatted.contains("[read_file for 'path/with spaces/file.rs']"));
    }

    #[test]
    fn test_default_line_limit() {
        assert_eq!(DEFAULT_LINE_LIMIT, 500);
    }
}
