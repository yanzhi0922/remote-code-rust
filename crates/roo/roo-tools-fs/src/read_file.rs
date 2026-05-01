//! read_file tool implementation.
//!
//! Supports two reading modes:
//! - **Slice mode** (default): reads lines by offset/limit with line numbers.
//! - **Indentation mode**: extracts semantic code blocks around an anchor line
//!   based on indentation structure.
//!
//! Also provides binary detection and long line truncation.

use crate::helpers::*;
use crate::types::*;
use roo_ignore::RooIgnoreController;
use roo_types::tool_params::{IndentationParams, ReadFileMode, ReadFileParams};

/// Validate read_file parameters.
pub fn validate_read_file_params(params: &ReadFileParams) -> Result<(), FsToolError> {
    if params.path.trim().is_empty() {
        return Err(FsToolError::Validation("path must not be empty".to_string()));
    }

    // Check for path traversal
    if params.path.contains("..") {
        return Err(FsToolError::InvalidPath(
            "path must not contain '..'".to_string(),
        ));
    }

    if let Some(offset) = params.offset {
        if offset == 0 {
            return Err(FsToolError::Validation(
                "offset must be >= 1 (1-based line number)".to_string(),
            ));
        }
    }

    if let Some(limit) = params.limit {
        if limit == 0 {
            return Err(FsToolError::Validation("limit must be >= 1".to_string()));
        }
    }

    // Validate indentation mode params
    if params.mode == Some(ReadFileMode::Indentation) {
        if let Some(ref indent) = params.indentation {
            if indent.anchor_line.is_none() {
                return Err(FsToolError::Validation(
                    "indentation mode requires anchor_line".to_string(),
                ));
            }
            if let Some(anchor) = indent.anchor_line {
                if anchor == 0 {
                    return Err(FsToolError::Validation(
                        "anchor_line must be >= 1 (1-based line number)".to_string(),
                    ));
                }
            }
        } else {
            return Err(FsToolError::Validation(
                "indentation mode requires indentation params".to_string(),
            ));
        }
    }

    Ok(())
}

/// Process a file read operation.
///
/// This function handles the core logic of reading a file:
/// 1. Reading raw bytes from disk
/// 2. Detecting binary content
/// 3. Decoding as UTF-8
/// 4. Applying mode-specific extraction (slice or indentation)
/// 5. Truncating long lines
/// 6. Adding line numbers
pub fn process_read_file(
    params: &ReadFileParams,
    cwd: &std::path::Path,
    ignore_controller: Option<&RooIgnoreController>,
) -> Result<ReadResult, FsToolError> {
    validate_read_file_params(params)?;

    // Check .rooignore before any file I/O
    check_roo_ignore(&params.path, ignore_controller)?;

    let file_path = resolve_path(&params.path, cwd)?;

    if !file_path.exists() {
        return Err(FsToolError::FileNotFound(params.path.clone()));
    }

    let metadata = std::fs::metadata(&file_path)?;
    if metadata.is_dir() {
        return Err(FsToolError::InvalidPath(format!(
            "{} is a directory, not a file",
            params.path
        )));
    }

    // Check file size
    if metadata.len() as usize > MAX_FILE_SIZE {
        return Err(FsToolError::ContentTooLarge(metadata.len() as usize, MAX_FILE_SIZE));
    }

    // Read raw bytes for binary detection
    let raw_data = std::fs::read(&file_path)?;

    if is_binary_content(&raw_data) {
        return Ok(ReadResult {
            content: format!(
                "(Binary file: {} bytes, not displaying)",
                raw_data.len()
            ),
            path: params.path.clone(),
            total_lines: 0,
            truncated: false,
            is_binary: true,
            start_line: 0,
            end_line: 0,
        });
    }

    // Decode as UTF-8
    let content = String::from_utf8_lossy(&raw_data).into_owned();

    let mode = params.mode.unwrap_or_default();
    match mode {
        ReadFileMode::Slice => {
            build_read_result(content, &params.path, params.offset, params.limit)
        }
        ReadFileMode::Indentation => {
            let indent_params = params.indentation.as_ref().unwrap();
            build_read_result_indentation(
                content,
                &params.path,
                indent_params,
            )
        }
    }
}

/// Build a ReadResult from string content with optional offset/limit (slice mode).
///
/// Adds line numbers to each line in the format `N | content`.
pub fn build_read_result(
    content: String,
    path: &str,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<ReadResult, FsToolError> {
    let total_lines = content.lines().count();

    let (sliced_content, total) = if let Some(start) = offset {
        let max_lines = limit.unwrap_or(DEFAULT_READ_LIMIT as u64) as usize;
        slice_lines(&content, start as usize, max_lines)
    } else if let Some(max) = limit {
        slice_lines(&content, 1, max as usize)
    } else {
        (content.clone(), total_lines)
    };

    let truncated = total_lines > 0
        && (offset.is_some() || limit.is_some())
        && sliced_content.lines().count() < total_lines;

    // Truncate long lines
    let truncated_lines = truncate_long_lines(&sliced_content, DEFAULT_MAX_LINE_LENGTH);

    // Add line numbers
    let start_line = offset.unwrap_or(1) as usize;
    let numbered = add_line_numbers_from(&truncated_lines, start_line);

    let returned_lines = truncated_lines.lines().count();
    let end_line = start_line + returned_lines.saturating_sub(1);

    // Build final content with truncation message (matching TS format)
    let final_content = if truncated {
        let effective_limit = limit.unwrap_or(DEFAULT_READ_LIMIT as u64) as usize;
        let next_offset = end_line + 1;
        format!(
            "IMPORTANT: File content truncated.\n\
             \tStatus: Showing lines {}-{} of {} total lines.\n\
             \tTo read more: Use the read_file tool with offset={} and limit={}.\n\
             \t\n\
             \t{}",
            start_line, end_line, total, next_offset, effective_limit, numbered
        )
    } else if returned_lines == 0 {
        "Note: File is empty".to_string()
    } else {
        numbered
    };

    Ok(ReadResult {
        content: final_content,
        path: path.to_string(),
        total_lines: total,
        truncated,
        is_binary: false,
        start_line,
        end_line,
    })
}

/// Build a ReadResult using indentation-based extraction.
///
/// Extracts a semantic code block around the anchor line by tracking
/// indentation levels. This is useful for extracting complete functions,
/// methods, structs, etc. from source code.
pub fn build_read_result_indentation(
    content: String,
    path: &str,
    params: &IndentationParams,
) -> Result<ReadResult, FsToolError> {
    let total_lines = content.lines().count();
    let anchor = params.anchor_line.unwrap() as usize;

    if anchor == 0 || anchor > total_lines {
        return Err(FsToolError::Validation(format!(
            "anchor_line {} is out of range (1-{})",
            anchor, total_lines
        )));
    }

    let max_lines = params.max_lines.unwrap_or(DEFAULT_READ_LIMIT as u64) as usize;

    // Extract the indentation block
    let extracted = extract_indentation_block(
        &content,
        anchor,
        params.max_levels.unwrap_or(0),
        params.include_siblings.unwrap_or(false),
        params.include_header.unwrap_or(true),
        max_lines,
    );

    let extracted_lines = extracted.lines().count();
    let truncated = extracted_lines < total_lines;

    // Truncate long lines
    let truncated_content = truncate_long_lines(&extracted, DEFAULT_MAX_LINE_LENGTH);

    // Determine start line for numbering
    let start_line = find_extraction_start_line(&content, anchor, params);
    let numbered = add_line_numbers_from(&truncated_content, start_line);

    let returned_lines = truncated_content.lines().count();
    let end_line = start_line + returned_lines.saturating_sub(1);

    // Build final content with truncation message (matching TS format)
    let effective_limit = params.max_lines.unwrap_or(DEFAULT_READ_LIMIT as u64) as usize;
    let final_content = if truncated {
        let next_offset = end_line + 1;
        format!(
            "IMPORTANT: File content truncated.\n\
             \tStatus: Showing lines {}-{} of {} total lines.\n\
             \tTo read more: Use the read_file tool with offset={} and limit={}.\n\
             \t\n\
             \t{}",
            start_line, end_line, total_lines, next_offset, effective_limit, numbered
        )
    } else {
        numbered
    };

    Ok(ReadResult {
        content: final_content,
        path: path.to_string(),
        total_lines,
        truncated,
        is_binary: false,
        start_line,
        end_line,
    })
}

/// Extract a semantic code block based on indentation structure.
///
/// Given an anchor line, walks upward to find the enclosing block boundary
/// (a line at the same or lower indentation level), then walks downward to
/// capture the full block including nested content.
fn extract_indentation_block(
    content: &str,
    anchor_line: usize,
    max_levels: u64,
    include_siblings: bool,
    include_header: bool,
    max_lines: usize,
) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() || anchor_line == 0 || anchor_line > lines.len() {
        return String::new();
    }

    let anchor_idx = anchor_line - 1; // Convert to 0-based
    let anchor_indent = leading_indent(lines[anchor_idx]);

    // Find the start of the block (walk upward)
    // Walk upward to find the enclosing block boundary (a line with less indentation).
    let mut start_idx = anchor_idx;
    let mut levels_up = 0u64;

    for i in (0..anchor_idx).rev() {
        let line = lines[i];
        if line.trim().is_empty() {
            continue;
        }
        let indent = leading_indent(line);
        if indent < anchor_indent {
            // Found a less-indented line — this is a block boundary
            levels_up += 1;
            if max_levels > 0 && levels_up > max_levels {
                break;
            }
            start_idx = i;
            if indent == 0 {
                break;
            }
        }
        // For same or greater indentation, keep walking up to find the block start
    }

    // If including header (imports, module-level comments), find the header region
    let header_end = if include_header {
        find_header_end(&lines)
    } else {
        0
    };

    // Adjust start to include header if it's above the block
    let final_start = if header_end > 0 && header_end <= start_idx {
        // Include header lines from header_end to start of block
        header_end
    } else {
        start_idx
    };

    // Find the end of the block (walk downward from anchor)
    let anchor_or_block_indent = leading_indent(lines[start_idx]);
    let mut end_idx = anchor_idx;

    for i in (anchor_idx + 1)..lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            end_idx = i;
            continue;
        }
        let indent = leading_indent(line);
        if indent < anchor_or_block_indent {
            // Dedented past the block start — stop
            break;
        }
        end_idx = i;
    }

    // If including siblings, extend to cover same-level blocks after anchor
    if include_siblings {
        for i in (end_idx + 1)..lines.len() {
            let line = lines[i];
            if line.trim().is_empty() {
                end_idx = i;
                continue;
            }
            let indent = leading_indent(line);
            if indent < anchor_indent {
                break;
            }
            if indent == anchor_indent {
                // Same level sibling — include it and its body
                end_idx = i;
                // Also include nested content of this sibling
                for j in (i + 1)..lines.len() {
                    let nested_line = lines[j];
                    if nested_line.trim().is_empty() {
                        end_idx = j;
                        continue;
                    }
                    if leading_indent(nested_line) < anchor_indent {
                        break;
                    }
                    end_idx = j;
                }
            }
        }
    }

    // Collect lines, respecting max_lines
    let mut result_lines = Vec::new();
    let mut count = 0;

    // Add header if present
    if include_header && header_end > 0 && header_end <= final_start {
        for i in 0..header_end {
            if count >= max_lines {
                break;
            }
            result_lines.push(lines[i].to_string());
            count += 1;
        }
    }

    // Add the block lines
    for i in final_start..=end_idx {
        if count >= max_lines {
            break;
        }
        if i < lines.len() {
            result_lines.push(lines[i].to_string());
            count += 1;
        }
    }

    result_lines.join("\n")
}

/// Get the number of leading whitespace characters (spaces or tabs) in a line.
fn leading_indent(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// Find the end of the file header region (imports, module-level comments).
///
/// The header ends when we encounter a non-comment, non-import, non-empty line
/// that starts a code block (typically a fn, struct, impl, pub, etc.).
fn find_header_end(lines: &[&str]) -> usize {
    let mut header_end = 0;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            header_end = i + 1;
            continue;
        }
        // Comment lines are part of header
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("///") {
            header_end = i + 1;
            continue;
        }
        // Import/use statements are part of header
        if trimmed.starts_with("use ") || trimmed.starts_with("import ")
            || trimmed.starts_with("mod ") || trimmed.starts_with("extern ")
            || trimmed.starts_with("#[") || trimmed.starts_with("pub mod ")
            || trimmed.starts_with("package ") || trimmed.starts_with("include!")
            || trimmed.starts_with("macro_rules!")
        {
            header_end = i + 1;
            continue;
        }
        // Attributes
        if trimmed.starts_with('#') {
            header_end = i + 1;
            continue;
        }
        // First non-header line
        break;
    }
    header_end
}

/// Determine the starting line number for the extracted block.
fn find_extraction_start_line(
    content: &str,
    anchor_line: usize,
    params: &IndentationParams,
) -> usize {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() || anchor_line == 0 || anchor_line > lines.len() {
        return 1;
    }

    let anchor_idx = anchor_line - 1;
    let anchor_indent = leading_indent(lines[anchor_idx]);

    // Walk upward to find block start
    let mut start_idx = anchor_idx;
    let max_levels = params.max_levels.unwrap_or(0);
    let mut levels_up = 0u64;

    for i in (0..anchor_idx).rev() {
        let line = lines[i];
        if line.trim().is_empty() {
            continue;
        }
        let indent = leading_indent(line);
        if indent < anchor_indent {
            levels_up += 1;
            if max_levels > 0 && levels_up > max_levels {
                break;
            }
            start_idx = i;
            if indent == 0 {
                break;
            }
        } else if indent == anchor_indent && !params.include_siblings.unwrap_or(false) {
            break;
        }
    }

    // If including header, adjust start
    if params.include_header.unwrap_or(true) {
        let header_end = find_header_end(&lines);
        if header_end > 0 && header_end <= start_idx {
            return header_end + 1; // 1-based
        }
    }

    start_idx + 1 // Convert to 1-based
}

/// Add line numbers to content starting from a specific line number.
///
/// Format: `N | content` where N is right-aligned.
fn add_line_numbers_from(content: &str, start_line: usize) -> String {
    let total_lines = content.lines().count();
    let width = total_lines.to_string().len().max(1);

    content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let line_num = start_line + i;
            format!("{:>width$} | {}", line_num, line, width = width)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Resolve a relative path against the current working directory.
fn resolve_path(path: &str, cwd: &std::path::Path) -> Result<std::path::PathBuf, FsToolError> {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        Ok(p.to_path_buf())
    } else {
        Ok(cwd.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create simple ReadFileParams (slice mode).
    fn slice_params(path: &str, offset: Option<u64>, limit: Option<u64>) -> ReadFileParams {
        ReadFileParams {
            path: path.to_string(),
            mode: None,
            offset,
            limit,
            indentation: None,
        }
    }

    /// Helper to create indentation-mode ReadFileParams.
    fn indent_params(
        path: &str,
        anchor_line: u64,
        max_levels: Option<u64>,
        include_siblings: bool,
    ) -> ReadFileParams {
        ReadFileParams {
            path: path.to_string(),
            mode: Some(ReadFileMode::Indentation),
            offset: None,
            limit: None,
            indentation: Some(IndentationParams {
                anchor_line: Some(anchor_line),
                max_levels,
                include_siblings: Some(include_siblings),
                include_header: Some(false),
                max_lines: None,
            }),
        }
    }

    #[test]
    fn test_validate_empty_path() {
        let params = slice_params("", None, None);
        assert!(validate_read_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_path_traversal() {
        let params = slice_params("../etc/passwd", None, None);
        assert!(validate_read_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_zero_offset() {
        let params = slice_params("test.txt", Some(0), None);
        assert!(validate_read_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_zero_limit() {
        let params = slice_params("test.txt", None, Some(0));
        assert!(validate_read_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_params() {
        let params = slice_params("test.txt", Some(1), Some(100));
        assert!(validate_read_file_params(&params).is_ok());
    }

    #[test]
    fn test_validate_valid_no_optional() {
        let params = slice_params("test.txt", None, None);
        assert!(validate_read_file_params(&params).is_ok());
    }

    #[test]
    fn test_validate_indentation_mode_no_anchor() {
        let params = ReadFileParams {
            path: "test.txt".to_string(),
            mode: Some(ReadFileMode::Indentation),
            offset: None,
            limit: None,
            indentation: Some(IndentationParams {
                anchor_line: None,
                max_levels: None,
                include_siblings: None,
                include_header: None,
                max_lines: None,
            }),
        };
        assert!(validate_read_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_indentation_mode_no_params() {
        let params = ReadFileParams {
            path: "test.txt".to_string(),
            mode: Some(ReadFileMode::Indentation),
            offset: None,
            limit: None,
            indentation: None,
        };
        assert!(validate_read_file_params(&params).is_err());
    }

    #[test]
    fn test_build_read_result_full() {
        let content = "line1\nline2\nline3".to_string();
        let result = build_read_result(content, "test.txt", None, None).unwrap();
        assert_eq!(result.total_lines, 3);
        assert!(!result.truncated);
        assert!(!result.is_binary);
        // Should have line numbers
        assert!(result.content.contains("1 | line1"));
        assert!(result.content.contains("2 | line2"));
        assert!(result.content.contains("3 | line3"));
    }

    #[test]
    fn test_build_read_result_with_offset() {
        let content = "line1\nline2\nline3\nline4\nline5".to_string();
        let result = build_read_result(content, "test.txt", Some(2), Some(2)).unwrap();
        assert_eq!(result.total_lines, 5);
        assert!(result.truncated);
        assert!(result.content.contains("2 | line2"));
        assert!(result.content.contains("3 | line3"));
    }

    #[test]
    fn test_build_read_result_with_limit() {
        let content = "line1\nline2\nline3".to_string();
        let result = build_read_result(content, "test.txt", None, Some(2)).unwrap();
        assert_eq!(result.total_lines, 3);
        assert!(result.truncated);
    }

    #[test]
    fn test_process_read_file_not_found() {
        let params = slice_params("nonexistent.txt", None, None);
        let cwd = std::env::current_dir().unwrap();
        let result = process_read_file(&params, &cwd, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_read_file_directory() {
        let dir = tempfile::tempdir().unwrap();
        let params = slice_params(dir.path().to_str().unwrap(), None, None);
        let result = process_read_file(&params, std::path::Path::new("."), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_read_file_actual() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello\nworld\nfoo\nbar\n").unwrap();

        let params = slice_params(file_path.to_str().unwrap(), None, None);
        let result = process_read_file(&params, std::path::Path::new("."), None).unwrap();
        assert_eq!(result.total_lines, 4);
        assert!(!result.is_binary);
        assert!(result.content.contains("hello"));
    }

    #[test]
    fn test_process_read_file_binary() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("binary.bin");
        std::fs::write(&file_path, b"hello\x00world").unwrap();

        let params = slice_params(file_path.to_str().unwrap(), None, None);
        let result = process_read_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.is_binary);
    }

    #[test]
    fn test_process_read_file_with_offset() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5\n").unwrap();

        let params = slice_params(file_path.to_str().unwrap(), Some(3), Some(2));
        let result = process_read_file(&params, std::path::Path::new("."), None).unwrap();
        assert_eq!(result.total_lines, 5);
        assert!(result.truncated);
        assert!(result.content.contains("3 | line3"));
        assert!(result.content.contains("4 | line4"));
    }

    // --- L5.1: Indentation mode tests ---

    #[test]
    fn test_indentation_extract_function() {
        let code = r#"use std::io;

fn helper() -> i32 {
    42
}

fn main() {
    let x = 1;
    if x > 0 {
        println!("positive");
    }
}

struct Foo {
    bar: i32,
}
"#;
        // Anchor on "println!" (line 11, 1-based)
        let result = build_read_result_indentation(
            code.to_string(),
            "test.rs",
            &IndentationParams {
                anchor_line: Some(11),
                max_levels: Some(0),
                include_siblings: Some(false),
                include_header: Some(false),
                max_lines: None,
            },
        )
        .unwrap();

        // Should include the if block and its parent context
        assert!(result.content.contains("println!"));
        assert!(!result.is_binary);
    }

    #[test]
    fn test_indentation_extract_with_siblings() {
        let code = "fn foo() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n}\n";

        // Anchor on "let b = 2;" (line 3)
        let result = build_read_result_indentation(
            code.to_string(),
            "test.rs",
            &IndentationParams {
                anchor_line: Some(3),
                max_levels: Some(0),
                include_siblings: Some(true),
                include_header: Some(false),
                max_lines: None,
            },
        )
        .unwrap();

        // With siblings, should include all let statements
        assert!(result.content.contains("let a = 1"));
        assert!(result.content.contains("let b = 2"));
        assert!(result.content.contains("let c = 3"));
    }

    #[test]
    fn test_indentation_anchor_out_of_range() {
        let code = "line1\nline2\n";
        let result = build_read_result_indentation(
            code.to_string(),
            "test.rs",
            &IndentationParams {
                anchor_line: Some(99),
                max_levels: None,
                include_siblings: None,
                include_header: None,
                max_lines: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_leading_indent() {
        assert_eq!(leading_indent("hello"), 0);
        assert_eq!(leading_indent("    hello"), 4);
        assert_eq!(leading_indent("\thello"), 1);
        assert_eq!(leading_indent("        hello"), 8);
        assert_eq!(leading_indent(""), 0);
    }

    #[test]
    fn test_add_line_numbers_from() {
        let content = "foo\nbar\nbaz";
        let result = add_line_numbers_from(content, 1);
        assert_eq!(result, "1 | foo\n2 | bar\n3 | baz");
    }

    #[test]
    fn test_add_line_numbers_from_offset() {
        let content = "foo\nbar";
        let result = add_line_numbers_from(content, 10);
        // Width is based on total lines (2), so single-digit width
        assert_eq!(result, "10 | foo\n11 | bar");
    }

    #[test]
    fn test_find_header_end() {
        let lines: Vec<&str> = vec![
            "use std::io;",
            "// comment",
            "",
            "fn main() {",
            "    println!();",
            "}",
        ];
        let header_end = find_header_end(&lines);
        assert_eq!(header_end, 3); // Lines 0-2 are header (use, comment, empty)
    }

    #[test]
    fn test_find_header_end_no_header() {
        let lines: Vec<&str> = vec![
            "fn main() {",
            "    println!();",
            "}",
        ];
        let header_end = find_header_end(&lines);
        assert_eq!(header_end, 0); // No header lines
    }

    #[test]
    fn test_process_read_file_indentation_mode() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            "use std::io;\n\nfn main() {\n    let x = 1;\n    println!(\"{}\", x);\n}\n",
        )
        .unwrap();

        let params = indent_params(file_path.to_str().unwrap(), 5, Some(0), false);
        let result = process_read_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(!result.is_binary);
        assert!(result.content.contains("println!"));
    }
}
