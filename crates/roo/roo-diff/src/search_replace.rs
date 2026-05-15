//! Single search-replace diff strategy for Roo Code Rust.
//!
//! Parses a single `<<< SEARCH` / `>>> REPLACE` block (without the multi-block
//! `<<<<<<< SEARCH` / `=======` / `>>>>>>> REPLACE` markers). This is the
//! simpler variant used by some LLM providers.
//!
//! Port of the single search-replace strategy from the Roo Code TypeScript source.

use crate::similarity::fuzzy_search;
use crate::text_utils::add_line_numbers;
use crate::types::DiffResult;

const BUFFER_LINES: usize = 40;

/// Parsed search-replace block.
struct SearchReplaceBlock {
    search_content: String,
    replace_content: String,
}

/// Parse a single `<<< SEARCH` / `>>> REPLACE` block.
///
/// The expected format is:
/// ```text
/// <<< SEARCH
/// search content here
/// >>> REPLACE
/// replacement content here
/// ```
///
/// Variants with `---` separator are also accepted:
/// ```text
/// <<< SEARCH
/// search content
/// ---
/// >>> REPLACE
/// replacement content
/// ```
fn parse_single_block(diff: &str) -> Option<SearchReplaceBlock> {
    let lines: Vec<&str> = diff.lines().collect();
    let mut i = 0;

    // Find <<< SEARCH
    while i < lines.len() {
        if lines[i].trim().starts_with("<<<") && lines[i].trim().contains("SEARCH") {
            i += 1;
            break;
        }
        i += 1;
    }

    if i >= lines.len() {
        return None;
    }

    let mut search_lines: Vec<String> = Vec::new();
    let mut found_end_search = false;

    // Read search content until >>> REPLACE or --- separator
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.contains(">>>") && trimmed.contains("REPLACE") {
            found_end_search = true;
            i += 1;
            break;
        }
        if trimmed == "---" {
            // Continue reading search content after separator
            i += 1;
            continue;
        }
        search_lines.push(lines[i].to_string());
        i += 1;
    }

    if !found_end_search {
        return None;
    }

    // Read replace content (everything after >>> REPLACE until end or next block)
    let mut replace_lines: Vec<String> = Vec::new();
    while i < lines.len() {
        let trimmed = lines[i].trim();
        // Stop at another search block
        if trimmed.starts_with("<<<") && trimmed.contains("SEARCH") {
            break;
        }
        replace_lines.push(lines[i].to_string());
        i += 1;
    }

    let search_content = trim_trailing_newline(&search_lines.join("\n"));
    let replace_content = trim_trailing_newline(&replace_lines.join("\n"));

    Some(SearchReplaceBlock {
        search_content,
        replace_content,
    })
}

/// Parse blocks using `<<< SEARCH` / `>>> REPLACE` markers where search content
/// comes BEFORE the `>>> REPLACE` marker and replace content comes AFTER.
fn parse_blocks(diff: &str) -> Vec<SearchReplaceBlock> {
    // First try the single-block format
    if let Some(block) = parse_single_block(diff) {
        return vec![block];
    }

    // Fallback: try the format where SEARCH and REPLACE sections are separated by >>> REPLACE
    let lines: Vec<&str> = diff.lines().collect();
    let mut blocks: Vec<SearchReplaceBlock> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Look for <<< SEARCH marker
        if trimmed.starts_with("<<<") && trimmed.contains("SEARCH") {
            let mut search_lines: Vec<String> = Vec::new();
            i += 1;

            // Read search content until >>> REPLACE
            while i < lines.len() {
                let t = lines[i].trim();
                if t.contains(">>>") && t.contains("REPLACE") {
                    i += 1;
                    break;
                }
                search_lines.push(lines[i].to_string());
                i += 1;
            }

            // Read replace content until next <<< SEARCH or end
            let mut replace_lines: Vec<String> = Vec::new();
            while i < lines.len() {
                let t = lines[i].trim();
                if t.starts_with("<<<") && t.contains("SEARCH") {
                    break;
                }
                replace_lines.push(lines[i].to_string());
                i += 1;
            }

            let search_content = trim_trailing_newline(&search_lines.join("\n"));
            let replace_content = trim_trailing_newline(&replace_lines.join("\n"));

            blocks.push(SearchReplaceBlock {
                search_content,
                replace_content,
            });
        } else {
            i += 1;
        }
    }

    blocks
}

/// Trims a single trailing newline from the content.
fn trim_trailing_newline(content: &str) -> String {
    if content.ends_with('\n') {
        content[..content.len() - 1].to_string()
    } else {
        content.to_string()
    }
}

/// Single search-replace diff strategy implementation.
///
/// Handles a simpler format than MultiSearchReplace:
/// ```text
/// <<< SEARCH
/// content to find
/// >>> REPLACE
/// replacement content
/// ```
pub struct SingleSearchReplaceDiffStrategy {
    fuzzy_threshold: f64,
    #[allow(dead_code)]
    buffer_lines: usize,
}

impl SingleSearchReplaceDiffStrategy {
    /// Create a new `SingleSearchReplaceDiffStrategy`.
    ///
    /// - `fuzzy_threshold`: Similarity threshold for fuzzy matching. Default 1.0 = exact.
    /// - `buffer_lines`: Context lines around the match for error reporting. Default 40.
    pub fn new(fuzzy_threshold: Option<f64>, buffer_lines: Option<usize>) -> Self {
        Self {
            fuzzy_threshold: fuzzy_threshold.unwrap_or(1.0),
            buffer_lines: buffer_lines.unwrap_or(BUFFER_LINES),
        }
    }

    /// Returns the name of this diff strategy.
    pub fn name(&self) -> &str {
        "SingleSearchReplace"
    }

    /// Apply the diff content to the original content.
    pub fn apply_diff(&self, original_content: &str, diff_content: &str) -> DiffResult {
        let blocks = parse_blocks(diff_content);

        if blocks.is_empty() {
            return DiffResult::fail(
                "Invalid diff format: no <<< SEARCH / >>> REPLACE blocks found\n\n\
                 Debug Info:\n\
                 - Expected Format: <<< SEARCH\\n[search content]\\n>>> REPLACE\\n[replace content]\n\
                 - Tip: Make sure to include both SEARCH and REPLACE markers"
                    .to_string(),
            );
        }

        // Detect line ending from original content
        let line_ending = if original_content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };

        let mut result_lines: Vec<String> = original_content
            .split("\r\n")
            .flat_map(|s| s.split('\n'))
            .map(String::from)
            .collect();

        let mut applied_count: usize = 0;
        let mut fail_parts: Vec<DiffResult> = Vec::new();

        for block in &blocks {
            let search_content = &block.search_content;
            let replace_content = &block.replace_content;

            // Validate that search and replace are not identical
            if search_content == replace_content {
                fail_parts.push(DiffResult::fail(
                    "Search and replace content are identical - no changes would be made"
                        .to_string(),
                ));
                continue;
            }

            let search_lines: Vec<String> = if search_content.is_empty() {
                Vec::new()
            } else {
                search_content
                    .split("\r\n")
                    .flat_map(|s| s.split('\n'))
                    .map(String::from)
                    .collect()
            };

            let replace_lines: Vec<String> = if replace_content.is_empty() {
                Vec::new()
            } else {
                replace_content
                    .split("\r\n")
                    .flat_map(|s| s.split('\n'))
                    .map(String::from)
                    .collect()
            };

            if search_lines.is_empty() {
                fail_parts.push(DiffResult::fail(
                    "Empty search content is not allowed".to_string(),
                ));
                continue;
            }

            let search_chunk = search_lines.join("\n");

            // Try fuzzy search over the entire content
            let fuzzy_result = fuzzy_search(&result_lines, &search_chunk, 0, result_lines.len());

            if fuzzy_result.best_match_index >= 0 && fuzzy_result.best_score >= self.fuzzy_threshold
            {
                let match_idx = fuzzy_result.best_match_index as usize;

                // Get indentation from the matched lines
                let original_indents: Vec<String> = result_lines
                    [match_idx..(match_idx + search_lines.len())]
                    .iter()
                    .map(|line| {
                        let trimmed = line.trim_start();
                        line[..line.len() - trimmed.len()].to_string()
                    })
                    .collect();

                let search_indents: Vec<String> = search_lines
                    .iter()
                    .map(|line| {
                        let trimmed = line.trim_start();
                        line[..line.len() - trimmed.len()].to_string()
                    })
                    .collect();

                // Apply indentation preservation
                let indented_replace_lines: Vec<String> = replace_lines
                    .iter()
                    .map(|line| {
                        let matched_indent =
                            original_indents.first().map(|s| s.as_str()).unwrap_or("");
                        let trimmed = line.trim_start();
                        let current_indent = &line[..line.len() - trimmed.len()];
                        let search_base_indent =
                            search_indents.first().map(|s| s.as_str()).unwrap_or("");
                        let relative_level =
                            current_indent.len() as isize - search_base_indent.len() as isize;

                        let final_indent = if relative_level < 0 {
                            let keep =
                                (matched_indent.len() as isize + relative_level).max(0) as usize;
                            matched_indent[..keep].to_string()
                        } else {
                            format!(
                                "{}{}",
                                matched_indent,
                                &current_indent[search_base_indent.len()..]
                            )
                        };
                        format!("{}{}", final_indent, trimmed)
                    })
                    .collect();

                // Splice the replacement
                let before: Vec<String> = result_lines[..match_idx].to_vec();
                let after: Vec<String> = result_lines[match_idx + search_lines.len()..].to_vec();
                result_lines = before;
                result_lines.extend(indented_replace_lines);
                result_lines.extend(after);

                applied_count += 1;
            } else {
                // Build error message
                let original_section = format!(
                    "\n\nOriginal Content:\n{}",
                    add_line_numbers(&result_lines.join("\n"), 1)
                );
                let best_match_section = if fuzzy_result.best_match_content.is_empty() {
                    "\n\nBest Match Found:\n(no match)".to_string()
                } else {
                    format!(
                        "\n\nBest Match Found:\n{}",
                        add_line_numbers(
                            &fuzzy_result.best_match_content,
                            (fuzzy_result.best_match_index + 1) as usize
                        )
                    )
                };

                fail_parts.push(DiffResult::fail(format!(
                    "No sufficiently similar match found ({}% similar, needs {}%)\n\n\
                     Debug Info:\n\
                     - Similarity Score: {}%\n\
                     - Required Threshold: {}%\n\
                     - Tip: Use the read_file tool to get the latest content of the file\n\n\
                     Search Content:\n{}{}{}",
                    (fuzzy_result.best_score * 100.0).floor() as usize,
                    (self.fuzzy_threshold * 100.0).floor() as usize,
                    (fuzzy_result.best_score * 100.0).floor() as usize,
                    (self.fuzzy_threshold * 100.0).floor() as usize,
                    search_chunk,
                    best_match_section,
                    original_section
                )));
            }
        }

        let final_content = result_lines.join(line_ending);
        if applied_count == 0 {
            if fail_parts.is_empty() {
                DiffResult::fail("No blocks were applied".to_string())
            } else {
                DiffResult::fail_with_parts(fail_parts)
            }
        } else {
            DiffResult::ok(final_content, fail_parts)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_strategy() -> SingleSearchReplaceDiffStrategy {
        SingleSearchReplaceDiffStrategy::new(None, None)
    }

    fn make_fuzzy_strategy() -> SingleSearchReplaceDiffStrategy {
        SingleSearchReplaceDiffStrategy::new(Some(0.9), None)
    }

    #[test]
    fn test_name() {
        let strategy = make_strategy();
        assert_eq!(strategy.name(), "SingleSearchReplace");
    }

    #[test]
    fn test_apply_basic_replacement() {
        let strategy = make_strategy();
        let original = "function hello() {\n    console.log(\"hello\")\n}\n";
        let diff = "\
<<< SEARCH
function hello() {
    console.log(\"hello\")
}
>>> REPLACE
function hello() {
    console.log(\"goodbye\")
}";
        let result = strategy.apply_diff(original, diff);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(
            result.content.unwrap(),
            "function hello() {\n    console.log(\"goodbye\")\n}\n"
        );
    }

    #[test]
    fn test_apply_single_line_change() {
        let strategy = make_strategy();
        let original = "line 1\nline 2\nline 3\n";
        let diff = "\
<<< SEARCH
line 2
>>> REPLACE
LINE 2";
        let result = strategy.apply_diff(original, diff);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(result.content.unwrap(), "line 1\nLINE 2\nline 3\n");
    }

    #[test]
    fn test_apply_no_match() {
        let strategy = make_strategy();
        let original = "line 1\nline 2\nline 3\n";
        let diff = "\
<<< SEARCH
not in file
>>> REPLACE
replacement";
        let result = strategy.apply_diff(original, diff);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_identical_search_replace() {
        let strategy = make_strategy();
        let original = "line 1\nline 2\nline 3\n";
        let diff = "\
<<< SEARCH
line 2
>>> REPLACE
line 2";
        let result = strategy.apply_diff(original, diff);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_empty_search() {
        let strategy = make_strategy();
        let original = "line 1\nline 2\n";
        let diff = "\
<<< SEARCH
>>> REPLACE
something";
        let result = strategy.apply_diff(original, diff);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_invalid_format() {
        let strategy = make_strategy();
        let original = "line 1\nline 2\n";
        let diff = "no markers here";
        let result = strategy.apply_diff(original, diff);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_fuzzy_matching() {
        let strategy = make_fuzzy_strategy();
        let original = "function processData(data) {\n    return data.map(item => item.name);\n}\n";
        let diff = "\
<<< SEARCH
function processData(data) {
    return data.map(item => item.name);
}
>>> REPLACE
function processData(data) {
    return data.filter(item => item.active).map(item => item.name);
}";
        let result = strategy.apply_diff(original, diff);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
    }

    #[test]
    fn test_apply_indentation_preservation() {
        let strategy = make_strategy();
        let original = "    function test() {\n        return true;\n    }\n";
        let diff = "\
<<< SEARCH
function test() {
    return true;
}
>>> REPLACE
function test() {
    return false;
}";
        let result = strategy.apply_diff(original, diff);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(
            result.content.unwrap(),
            "    function test() {\n        return false;\n    }\n"
        );
    }

    #[test]
    fn test_apply_windows_line_endings() {
        let strategy = make_strategy();
        let original = "function test() {\r\n    return true;\r\n}\r\n";
        let diff = "\
<<< SEARCH
function test() {
    return true;
}
>>> REPLACE
function test() {
    return false;
}";
        let result = strategy.apply_diff(original, diff);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(
            result.content.unwrap(),
            "function test() {\r\n    return false;\r\n}\r\n"
        );
    }

    #[test]
    fn test_parse_blocks_basic() {
        let diff = "\
<<< SEARCH
hello
>>> REPLACE
world";
        let blocks = parse_blocks(diff);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search_content, "hello");
        assert_eq!(blocks[0].replace_content, "world");
    }

    #[test]
    fn test_parse_blocks_multiline() {
        let diff = "\
<<< SEARCH
line 1
line 2
>>> REPLACE
LINE 1
LINE 2";
        let blocks = parse_blocks(diff);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search_content, "line 1\nline 2");
        assert_eq!(blocks[0].replace_content, "LINE 1\nLINE 2");
    }

    #[test]
    fn test_parse_blocks_no_markers() {
        let diff = "no markers here";
        let blocks = parse_blocks(diff);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_apply_deletion() {
        let strategy = make_strategy();
        let original = "function test() {\n    console.log(\"hello\");\n    return true;\n}\n";
        let diff = "\
<<< SEARCH
    console.log(\"hello\");
    return true;
>>> REPLACE
    return false;";
        let result = strategy.apply_diff(original, diff);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(
            result.content.unwrap(),
            "function test() {\n    return false;\n}\n"
        );
    }
}
