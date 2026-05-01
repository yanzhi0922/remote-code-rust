use crate::similarity::{fuzzy_search, get_similarity};
use crate::text_utils::{add_line_numbers, every_line_has_line_numbers, strip_line_numbers};
use crate::types::{DiffResult, ToolProgressStatus, ToolUse};
use crate::validate::validate_marker_sequencing;

const BUFFER_LINES: usize = 40;

/// A parsed diff block containing search and replace content.
struct DiffBlock {
    start_line: usize,
    search_content: String,
    replace_content: String,
}

/// Parses diff content into a list of DiffBlocks using a line-by-line state machine.
/// This replaces the JavaScript regex which uses look-around assertions not supported
/// by the Rust `regex` crate.
fn parse_diff_blocks(diff_content: &str) -> Vec<DiffBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = diff_content.split('\n').collect();

    let mut i = 0;

    while i < lines.len() {
        // Find the next unescaped <<<<<<< SEARCH line
        let line = lines[i].trim_end();

        // Check if this line is an unescaped <<<<<<< SEARCH marker
        if is_search_marker(line) {
            let mut start_line: usize = 0;
            let mut search_lines: Vec<String> = Vec::new();
            let mut replace_lines: Vec<String> = Vec::new();
            let mut found_separator = false;
            let mut found_replace = false;

            i += 1; // Move past <<<<<<< SEARCH

            // Check for optional :start_line: marker
            if i < lines.len() {
                let trimmed = lines[i].trim();
                if trimmed.starts_with(":start_line:") {
                    // Parse the line number
                    let rest = trimmed[":start_line:".len()..].trim();
                    if let Ok(num) = rest.parse::<usize>() {
                        start_line = num;
                    }
                    i += 1;
                }
            }

            // Check for optional :end_line: marker
            if i < lines.len() {
                let trimmed = lines[i].trim();
                if trimmed.starts_with(":end_line:") {
                    // Skip end_line marker (we don't use it)
                    i += 1;
                }
            }

            // Check for optional ------- separator
            if i < lines.len() {
                let trimmed = lines[i].trim_end();
                if trimmed == "-------" && !lines[i].starts_with('\\') {
                    i += 1;
                }
            }

            // Read search content until ======= (not escaped)
            while i < lines.len() {
                let trimmed = lines[i].trim_end();
                if trimmed == "=======" && !lines[i].starts_with('\\') {
                    found_separator = true;
                    i += 1;
                    break;
                }
                search_lines.push(lines[i].to_string());
                i += 1;
            }

            if !found_separator {
                // Malformed block, skip
                break;
            }

            // Read replace content until >>>>>>> REPLACE (not escaped)
            while i < lines.len() {
                let trimmed = lines[i].trim_end();
                if trimmed == ">>>>>>> REPLACE" && !lines[i].starts_with('\\') {
                    found_replace = true;
                    i += 1;
                    break;
                }
                replace_lines.push(lines[i].to_string());
                i += 1;
            }

            if found_replace {
                // Trim trailing empty line from search content if present
                // (matching JS regex behavior which has (?:\n)? after search content)
                let search_content = trim_trailing_newline(&search_lines.join("\n"));
                let replace_content = trim_trailing_newline(&replace_lines.join("\n"));

                blocks.push(DiffBlock {
                    start_line,
                    search_content,
                    replace_content,
                });
            }
        } else {
            i += 1;
        }
    }

    blocks
}

/// Checks if a line is an unescaped <<<<<<< SEARCH marker.
/// Matches `<<<<<<< SEARCH`, `<<<<<<< SEARCH>`, `<<<<<<< SEARCH>>`, etc.
fn is_search_marker(line: &str) -> bool {
    if line.starts_with('\\') {
        return false;
    }
    let trimmed = line.trim_end();
    if !trimmed.starts_with("<<<<<<< SEARCH") {
        return false;
    }
    let rest = &trimmed["<<<<<<< SEARCH".len()..];
    // Allow optional '>' characters after SEARCH
    rest.chars().all(|c| c == '>')
}

/// Trims a single trailing newline from the content, matching the JS regex `(?:\n)?` behavior.
fn trim_trailing_newline(content: &str) -> String {
    if content.ends_with('\n') {
        content[..content.len() - 1].to_string()
    } else {
        content.to_string()
    }
}

/// MultiSearchReplace diff strategy implementation.
///
/// Port of `MultiSearchReplaceDiffStrategy` from `multi-search-replace.ts`.
pub struct MultiSearchReplaceDiffStrategy {
    fuzzy_threshold: f64,
    buffer_lines: usize,
}

impl MultiSearchReplaceDiffStrategy {
    /// Creates a new MultiSearchReplaceDiffStrategy.
    ///
    /// - `fuzzy_threshold`: Similarity threshold for fuzzy matching (default 1.0 = exact).
    /// - `buffer_lines`: Number of extra context lines to show before and after matches (default 40).
    pub fn new(fuzzy_threshold: Option<f64>, buffer_lines: Option<usize>) -> Self {
        Self {
            fuzzy_threshold: fuzzy_threshold.unwrap_or(1.0),
            buffer_lines: buffer_lines.unwrap_or(BUFFER_LINES),
        }
    }

    /// Returns the name of this diff strategy.
    pub fn name(&self) -> &str {
        "MultiSearchReplace"
    }

    /// Unescapes special markers in diff content.
    ///
    /// Handles escaped markers like `\<<<<<<<`, `\=======`, `\>>>>>>>`, etc.
    fn unescape_markers(content: &str) -> String {
        let mut result = String::new();
        for line in content.split('\n') {
            let processed = if line.starts_with("\\<<<<<<<") {
                &line[1..]
            } else if line.starts_with("\\=======") {
                &line[1..]
            } else if line.starts_with("\\>>>>>>>") {
                &line[1..]
            } else if line.starts_with("\\-------") {
                &line[1..]
            } else if line.starts_with("\\:end_line:") {
                &line[1..]
            } else if line.starts_with("\\:start_line:") {
                &line[1..]
            } else {
                line
            };
            if result.is_empty() {
                result = processed.to_string();
            } else {
                result.push('\n');
                result.push_str(processed);
            }
        }
        result
    }

    /// Applies the diff content to the original content.
    ///
    /// This is the core diff application algorithm. It parses diff blocks,
    /// performs fuzzy/exact matching, handles indentation preservation,
    /// and applies the replacements.
    #[allow(unused_assignments)]
    pub fn apply_diff(&self, original_content: &str, diff_content: &str) -> DiffResult {
        let valid_seq = validate_marker_sequencing(diff_content);
        if !valid_seq.success {
            return DiffResult::fail(valid_seq.error.unwrap());
        }

        // Parse diff blocks using custom parser (replaces JS regex with look-around)
        let parsed_blocks = parse_diff_blocks(diff_content);

        if parsed_blocks.is_empty() {
            return DiffResult::fail(format!(
                "Invalid diff format - missing required sections\n\nDebug Info:\n- Expected Format: <<<<<<< SEARCH\\n:start_line: start line\\n-------\\n[search content]\\n=======\\n[replace content]\\n>>>>>>> REPLACE\n- Tip: Make sure to include start_line/SEARCH/=======/REPLACE sections with correct markers on new lines"
            ));
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
        let mut delta: i64 = 0;
        let mut diff_results: Vec<DiffResult> = Vec::new();
        let mut applied_count: usize = 0;

        // Sort replacements by startLine
        let mut replacements = parsed_blocks;
        replacements.sort_by_key(|r| r.start_line);

        for replacement in replacements {
            let (mut search_content, mut replace_content) =
                (replacement.search_content.clone(), replacement.replace_content.clone());
            let mut start_line =
                replacement.start_line as i64 + if replacement.start_line == 0 { 0 } else { delta };

            // First unescape any escaped markers in the content
            search_content = Self::unescape_markers(&search_content);
            replace_content = Self::unescape_markers(&replace_content);

            // Strip line numbers from search and replace content if every line starts with a line number
            let has_all_line_numbers = (every_line_has_line_numbers(&search_content)
                && every_line_has_line_numbers(&replace_content))
                || (every_line_has_line_numbers(&search_content) && replace_content.trim().is_empty());

            if has_all_line_numbers && start_line == 0 {
                if let Some(first_line) = search_content.split('\n').next() {
                    if let Some(num_part) = first_line.split('|').next() {
                        if let Ok(num) = num_part.trim().parse::<i64>() {
                            start_line = num;
                        }
                    }
                }
            }

            if has_all_line_numbers {
                search_content = strip_line_numbers(&search_content, false);
                replace_content = strip_line_numbers(&replace_content, false);
            }

            // Validate that search and replace content are not identical
            if search_content == replace_content {
                diff_results.push(DiffResult::fail(format!(
                    "Search and replace content are identical - no changes would be made\n\n\
                     Debug Info:\n\
                     - Search and replace must be different to make changes\n\
                     - Use read_file to verify the content you want to change"
                )));
                continue;
            }

            // Split content into lines, handling both \n and \r\n
            let mut search_lines: Vec<String> = if search_content.is_empty() {
                Vec::new()
            } else {
                search_content
                    .split("\r\n")
                    .flat_map(|s| s.split('\n'))
                    .map(String::from)
                    .collect()
            };
            let mut replace_lines: Vec<String> = if replace_content.is_empty() {
                Vec::new()
            } else {
                replace_content
                    .split("\r\n")
                    .flat_map(|s| s.split('\n'))
                    .map(String::from)
                    .collect()
            };

            // Validate that search content is not empty
            if search_lines.is_empty() {
                diff_results.push(DiffResult::fail(format!(
                    "Empty search content is not allowed\n\nDebug Info:\n- Search content cannot be empty\n- For insertions, provide a specific line using :start_line: and include content to search for\n- For example, match a single line to insert before/after it"
                )));
                continue;
            }

            let end_line = replacement.start_line as i64 + search_lines.len() as i64 - 1;

            // Initialize search variables
            let mut match_index: i64 = -1;
            let mut best_match_score: f64 = 0.0;
            let mut best_match_content = String::new();
            let search_chunk = search_lines.join("\n");

            // Determine search bounds
            let mut search_start_index: usize = 0;
            let mut search_end_index: usize = result_lines.len();

            // Validate and handle line range if provided
            if start_line > 0 {
                // Convert to 0-based index
                let exact_start_index = (start_line - 1) as usize;
                let search_len = search_lines.len();
                let exact_end_index = exact_start_index + search_len - 1;

                if exact_end_index < result_lines.len() {
                    // Try exact match first
                    let original_chunk =
                        result_lines[exact_start_index..=exact_end_index].join("\n");
                    let similarity = get_similarity(&original_chunk, &search_chunk);
                    if similarity >= self.fuzzy_threshold {
                        match_index = exact_start_index as i64;
                        best_match_score = similarity;
                        best_match_content = original_chunk;
                    } else {
                        // Set bounds for buffered search
                        search_start_index =
                            (start_line as usize).saturating_sub(self.buffer_lines + 1);
                        search_end_index = ((start_line as usize) + search_len + self.buffer_lines)
                            .min(result_lines.len());
                    }
                } else {
                    // Line range is out of bounds, use buffered search
                    search_start_index =
                        (start_line as usize).saturating_sub(self.buffer_lines + 1);
                    search_end_index = ((start_line as usize) + search_len + self.buffer_lines)
                        .min(result_lines.len());
                }
            }

            // If no match found yet, try middle-out search within bounds
            if match_index == -1 {
                let fuzzy_result = fuzzy_search(
                    &result_lines,
                    &search_chunk,
                    search_start_index,
                    search_end_index,
                );
                match_index = fuzzy_result.best_match_index;
                best_match_score = fuzzy_result.best_score;
                best_match_content = fuzzy_result.best_match_content;
            }

            // Try aggressive line number stripping as a fallback if regular matching fails
            if match_index == -1 || best_match_score < self.fuzzy_threshold {
                let aggressive_search_content = strip_line_numbers(&search_content, true);
                let aggressive_replace_content = strip_line_numbers(&replace_content, true);

                let aggressive_search_lines: Vec<String> = if aggressive_search_content.is_empty()
                {
                    Vec::new()
                } else {
                    aggressive_search_content
                        .split("\r\n")
                        .flat_map(|s| s.split('\n'))
                        .map(String::from)
                        .collect()
                };
                let aggressive_search_chunk = aggressive_search_lines.join("\n");

                // Try middle-out search again with aggressive stripped content
                let fuzzy_result = fuzzy_search(
                    &result_lines,
                    &aggressive_search_chunk,
                    search_start_index,
                    search_end_index,
                );
                if fuzzy_result.best_match_index != -1
                    && fuzzy_result.best_score >= self.fuzzy_threshold
                {
                    match_index = fuzzy_result.best_match_index;
                    best_match_score = fuzzy_result.best_score;
                    best_match_content = fuzzy_result.best_match_content;
                    // Replace the original search/replace with their stripped versions
                    search_content = aggressive_search_content;
                    replace_content = aggressive_replace_content;
                    search_lines = aggressive_search_lines;
                    replace_lines = if replace_content.is_empty() {
                        Vec::new()
                    } else {
                        replace_content
                            .split("\r\n")
                            .flat_map(|s| s.split('\n'))
                            .map(String::from)
                            .collect()
                    };
                } else {
                    // No match found with either method
                    let original_content_section = if start_line > 0 && end_line > 0 {
                        let slice_start = ((start_line as usize).saturating_sub(self.buffer_lines + 1))
                            .max(0)
                            .min(result_lines.len());
                        let slice_end = ((end_line as usize + self.buffer_lines)
                            .min(result_lines.len()))
                        .max(slice_start);
                        let section = result_lines[slice_start..slice_end].join("\n");
                        format!(
                            "\n\nOriginal Content:\n{}",
                            add_line_numbers(
                                &section,
                                ((start_line as usize).saturating_sub(self.buffer_lines)).max(1)
                            )
                        )
                    } else {
                        format!(
                            "\n\nOriginal Content:\n{}",
                            add_line_numbers(&result_lines.join("\n"), 1)
                        )
                    };

                    let best_match_section = if best_match_content.is_empty() {
                        "\n\nBest Match Found:\n(no match)".to_string()
                    } else {
                        format!(
                            "\n\nBest Match Found:\n{}",
                            add_line_numbers(&best_match_content, (match_index + 1) as usize)
                        )
                    };

                    let line_range = if start_line > 0 {
                        format!(" at line: {}", start_line)
                    } else {
                        String::new()
                    };

                    diff_results.push(DiffResult::fail(format!(
                        "No sufficiently similar match found{} ({}% similar, needs {}%)\n\nDebug Info:\n- Similarity Score: {}%\n- Required Threshold: {}%\n- Search Range: {}\n- Tried both standard and aggressive line number stripping\n- Tip: Use the read_file tool to get the latest content of the file before attempting to use the apply_diff tool again, as the file content may have changed\n\nSearch Content:\n{}{}{}",
                        line_range,
                        (best_match_score * 100.0).floor() as usize,
                        (self.fuzzy_threshold * 100.0).floor() as usize,
                        (best_match_score * 100.0).floor() as usize,
                        (self.fuzzy_threshold * 100.0).floor() as usize,
                        if start_line > 0 { format!("starting at line {}", start_line) } else { "start to end".to_string() },
                        search_chunk,
                        best_match_section,
                        original_content_section
                    )));
                    continue;
                }
            }

            // Get the matched lines from the original content
            let match_idx = match_index as usize;
            let matched_line_count = search_lines.len();

            // Get the exact indentation (preserving tabs/spaces) of each matched line
            let original_indents: Vec<String> = result_lines
                [match_idx..(match_idx + matched_line_count)]
                .iter()
                .map(|line| {
                    let trimmed = line.trim_start();
                    line[..line.len() - trimmed.len()].to_string()
                })
                .collect();

            // Get the exact indentation of each line in the search block
            let search_indents: Vec<String> = search_lines
                .iter()
                .map(|line| {
                    let trimmed = line.trim_start();
                    line[..line.len() - trimmed.len()].to_string()
                })
                .collect();

            // Apply the replacement while preserving exact indentation
            let indented_replace_lines: Vec<String> = replace_lines
                .iter()
                .map(|line| {
                    // Get the matched line's exact indentation
                    let matched_indent =
                        original_indents.first().map(|s| s.as_str()).unwrap_or("");

                    // Get the current line's indentation relative to the search content
                    let current_indent: &str = {
                        let trimmed = line.trim_start();
                        &line[..line.len() - trimmed.len()]
                    };
                    let search_base_indent =
                        search_indents.first().map(|s| s.as_str()).unwrap_or("");

                    // Calculate the relative indentation level
                    let search_base_level = search_base_indent.len();
                    let current_level = current_indent.len();
                    let relative_level = current_level as isize - search_base_level as isize;

                    // If relative level is negative, remove indentation from matched indent
                    // If positive, add to matched indent
                    let final_indent = if relative_level < 0 {
                        let keep =
                            (matched_indent.len() as isize + relative_level).max(0) as usize;
                        matched_indent[..keep].to_string()
                    } else {
                        format!("{}{}", matched_indent, &current_indent[search_base_level..])
                    };

                    format!("{}{}", final_indent, line.trim_start())
                })
                .collect();

            // Construct the final content
            let before_match: Vec<String> = result_lines[..match_idx].to_vec();
            let after_match: Vec<String> = result_lines[match_idx + matched_line_count..].to_vec();
            result_lines = before_match;
            result_lines.extend(indented_replace_lines);
            result_lines.extend(after_match);

            delta -= matched_line_count as i64;
            delta += replace_lines.len() as i64;
            applied_count += 1;
        }

        let final_content = result_lines.join(line_ending);
        if applied_count == 0 {
            DiffResult::fail_with_parts(diff_results)
        } else {
            DiffResult::ok(final_content, diff_results)
        }
    }

    /// Gets the progress status for a tool use.
    pub fn get_progress_status(
        &self,
        tool_use: &ToolUse,
        result: Option<&DiffResult>,
    ) -> ToolProgressStatus {
        if let Some(diff_content) = &tool_use.params.diff {
            let icon = "diff-multiple".to_string();
            if tool_use.partial {
                if (diff_content.len() / 10) % 10 == 0 {
                    let search_block_count = diff_content.matches("SEARCH").count();
                    return ToolProgressStatus {
                        icon: Some(icon),
                        text: Some(format!("{}", search_block_count)),
                    };
                }
            } else if let Some(res) = result {
                let search_block_count = diff_content.matches("SEARCH").count();
                if !res.fail_parts.is_empty() {
                    return ToolProgressStatus {
                        icon: Some(icon),
                        text: Some(format!(
                            "{}/{}",
                            search_block_count - res.fail_parts.len(),
                            search_block_count
                        )),
                    };
                } else {
                    return ToolProgressStatus {
                        icon: Some(icon),
                        text: Some(format!("{}", search_block_count)),
                    };
                }
            }
        }
        ToolProgressStatus {
            icon: None,
            text: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolUseParams;

    fn make_strategy() -> MultiSearchReplaceDiffStrategy {
        MultiSearchReplaceDiffStrategy::new(None, None)
    }

    fn make_fuzzy_strategy() -> MultiSearchReplaceDiffStrategy {
        MultiSearchReplaceDiffStrategy::new(Some(0.9), None)
    }

    #[test]
    fn test_name() {
        let strategy = make_strategy();
        assert_eq!(strategy.name(), "MultiSearchReplace");
    }

    #[test]
    fn test_apply_diff_basic_replacement() {
        let strategy = make_strategy();
        let original = "function hello() {\n    console.log(\"hello\")\n}\n";
        let diff = "\
<<<<<<< SEARCH
function hello() {
    console.log(\"hello\")
}
=======
function hello() {
    console.log(\"goodbye\")
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "function hello() {\n    console.log(\"goodbye\")\n}\n"
        );
    }

    #[test]
    fn test_apply_diff_multiple_blocks() {
        let strategy = make_strategy();
        let original = "function hello() {\n    console.log(\"hello\")\n}\n";
        let diff = "\
<<<<<<< SEARCH
function hello() {
=======
function goodbye() {
>>>>>>> REPLACE

<<<<<<< SEARCH
    console.log(\"hello\")
=======
    console.log(\"goodbye\")
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "function goodbye() {\n    console.log(\"goodbye\")\n}\n"
        );
    }

    #[test]
    fn test_apply_diff_with_line_numbers() {
        let strategy = make_strategy();
        let original = "function hello() {\n    console.log(\"hello\")\n}\n";
        let diff = "\
<<<<<<< SEARCH
1 | function hello() {
2 |     console.log(\"hello\")
3 | }
=======
function hello() {
    console.log(\"goodbye\")
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "function hello() {\n    console.log(\"goodbye\")\n}\n"
        );
    }

    #[test]
    fn test_apply_diff_with_start_line() {
        let strategy = make_strategy();
        let original = "function hello() {\n    console.log(\"hello\")\n}\n";
        let diff = "\
<<<<<<< SEARCH
:start_line:1
function hello() {
    console.log(\"hello\")
}
=======
function hello() {
    console.log(\"goodbye\")
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
    }

    #[test]
    fn test_apply_diff_indentation_preservation() {
        let strategy = make_strategy();
        let original = "    function test() {\n        return true;\n    }\n";
        let diff = "\
<<<<<<< SEARCH
function test() {
    return true;
}
=======
function test() {
    return false;
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "    function test() {\n        return false;\n    }\n"
        );
    }

    #[test]
    fn test_apply_diff_tab_indentation() {
        let strategy = make_strategy();
        let original = "function test() {\n\treturn true;\n}\n";
        let diff = "\
<<<<<<< SEARCH
function test() {
\treturn true;
}
=======
function test() {
\treturn false;
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "function test() {\n\treturn false;\n}\n"
        );
    }

    #[test]
    fn test_apply_diff_windows_line_endings() {
        let strategy = make_strategy();
        let original = "function test() {\r\n    return true;\r\n}\r\n";
        let diff = "\
<<<<<<< SEARCH
function test() {
    return true;
}
=======
function test() {
    return false;
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "function test() {\r\n    return false;\r\n}\r\n"
        );
    }

    #[test]
    fn test_apply_diff_no_match() {
        let strategy = make_strategy();
        let original = "function hello() {\n    console.log(\"hello\")\n}\n";
        let diff = "\
<<<<<<< SEARCH
function goodbye() {
    console.log(\"goodbye\")
}
=======
function hello() {
    console.log(\"hello\")
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_diff_invalid_format() {
        let strategy = make_strategy();
        let original = "function hello() {\n    console.log(\"hello\")\n}\n";
        let diff = "invalid diff format";
        let result = strategy.apply_diff(original, diff);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_diff_identical_search_replace() {
        let strategy = make_strategy();
        let original = "line 1\nline 2\nline 3\n";
        let diff = "\
<<<<<<< SEARCH
line 2
=======
line 2
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(!result.success);
        // Error is in fail_parts since no blocks were applied
        let error_msg = result.error.as_deref()
            .or_else(|| result.fail_parts.first().and_then(|p| p.error.as_deref()))
            .unwrap();
        assert!(error_msg.contains("identical"));
    }

    #[test]
    fn test_apply_diff_deletion() {
        let strategy = make_strategy();
        let original = "function test() {\n    console.log(\"hello\");\n    return true;\n}\n";
        let diff = "\
<<<<<<< SEARCH
    console.log(\"hello\");
    return true;
=======
    return false;
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "function test() {\n    return false;\n}\n"
        );
    }

    #[test]
    fn test_apply_diff_fuzzy_matching() {
        let strategy = make_fuzzy_strategy();
        let original =
            "function processData(data) {\n    return data.map(item => item.name);\n}\n";
        let diff = "\
<<<<<<< SEARCH
function processData(data) {
    return data.map(item => item.name);
}
=======
function processData(data) {
    return data.filter(item => item.active).map(item => item.name);
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
    }

    #[test]
    fn test_unescape_markers() {
        let content = "\\<<<<<<< SEARCH\n\\=======\n\\>>>>>>>\n\\-------\n\\:start_line:\n\\:end_line:";
        let result = MultiSearchReplaceDiffStrategy::unescape_markers(content);
        assert!(result.contains("<<<<<<< SEARCH"));
        assert!(result.contains("======="));
        assert!(result.contains(">>>>>>>"));
        assert!(result.contains("-------"));
        assert!(result.contains(":start_line:"));
        assert!(result.contains(":end_line:"));
    }

    #[test]
    fn test_get_progress_status_partial() {
        let strategy = make_strategy();
        // Need a string whose length / 10 % 10 == 0
        // Construct a diff string of exactly 100 chars
        let base = "<<<<<<< SEARCH\naaaa\n=======\nnew\n>>>>>>> REPLACE";
        let padding = 100 - base.len();
        let diff_content = format!(
            "<<<<<<< SEARCH\n{}\n=======\nnew\n>>>>>>> REPLACE",
            "a".repeat(padding + 4) // +4 because we replaced "aaaa" with the padded version
        );
        assert_eq!(diff_content.len() / 10 % 10, 0);
        let tool_use = ToolUse {
            params: ToolUseParams {
                diff: Some(diff_content),
            },
            partial: true,
        };
        let status = strategy.get_progress_status(&tool_use, None);
        assert_eq!(status.icon, Some("diff-multiple".to_string()));
    }

    #[test]
    fn test_get_progress_status_complete() {
        let strategy = make_strategy();
        let tool_use = ToolUse {
            params: ToolUseParams {
                diff: Some(
                    "<<<<<<< SEARCH\ncontent\n=======\nnew content\n>>>>>>> REPLACE".to_string(),
                ),
            },
            partial: false,
        };
        let result = DiffResult::ok("new content".to_string(), Vec::new());
        let status = strategy.get_progress_status(&tool_use, Some(&result));
        assert_eq!(status.icon, Some("diff-multiple".to_string()));
        assert_eq!(status.text, Some("1".to_string()));
    }

    #[test]
    fn test_get_progress_status_with_failures() {
        let strategy = make_strategy();
        let tool_use = ToolUse {
            params: ToolUseParams {
                diff: Some("<<<<<<< SEARCH\ncontent\n=======\nnew content\n>>>>>>> REPLACE\n<<<<<<< SEARCH\nother\n=======\nnew other\n>>>>>>> REPLACE".to_string()),
            },
            partial: false,
        };
        let result =
            DiffResult::ok("new content".to_string(), vec![DiffResult::fail("error".to_string())]);
        let status = strategy.get_progress_status(&tool_use, Some(&result));
        assert_eq!(status.icon, Some("diff-multiple".to_string()));
        assert_eq!(status.text, Some("1/2".to_string()));
    }

    #[test]
    fn test_get_progress_status_no_diff() {
        let strategy = make_strategy();
        let tool_use = ToolUse {
            params: ToolUseParams { diff: None },
            partial: false,
        };
        let status = strategy.get_progress_status(&tool_use, None);
        assert_eq!(status.icon, None);
        assert_eq!(status.text, None);
    }

    #[test]
    fn test_apply_diff_with_extra_arrow_in_search() {
        let strategy = make_strategy();
        let original = "some content\nnew content\n";
        let diff = "\
<<<<<<< SEARCH>
some content
=======
updated content
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(result.content.unwrap(), "updated content\nnew content\n");
    }

    #[test]
    fn test_apply_diff_whitespace_handling() {
        let strategy = make_strategy();
        let original = "\nfunction example() {\n    return 42;\n}\n\n";
        let diff = "\
<<<<<<< SEARCH
function example() {
    return 42;
}
=======
function example() {
    return 43;
}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "\nfunction example() {\n    return 43;\n}\n\n"
        );
    }

    #[test]
    fn test_apply_diff_preserves_indentation_when_adding_lines() {
        let strategy = make_strategy();
        let original = "\tfunction test() {\n\t\treturn true;\n\t}";
        let diff = "\
<<<<<<< SEARCH
\tfunction test() {
\t\treturn true;
\t}
=======
\tfunction test() {
\t\t// First comment
\t\t// Second comment
\t\treturn true;
\t}
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "\tfunction test() {\n\t\t// First comment\n\t\t// Second comment\n\t\treturn true;\n\t}"
        );
    }

    #[test]
    fn test_apply_diff_negative_indentation() {
        let strategy = make_strategy();
        let original =
            "class Example {\n        if (true) {\n            this.init();\n            this.setup();\n        }\n}";
        let diff = "\
<<<<<<< SEARCH
            this.init();
            this.setup();
=======
        this.init();
        this.setup();
>>>>>>> REPLACE";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "class Example {\n        if (true) {\n        this.init();\n        this.setup();\n        }\n}"
        );
    }

    #[test]
    fn test_parse_diff_blocks_basic() {
        let diff = "\
<<<<<<< SEARCH
hello
=======
world
>>>>>>> REPLACE";
        let blocks = parse_diff_blocks(diff);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search_content, "hello");
        assert_eq!(blocks[0].replace_content, "world");
        assert_eq!(blocks[0].start_line, 0);
    }

    #[test]
    fn test_parse_diff_blocks_with_start_line() {
        let diff = "\
<<<<<<< SEARCH
:start_line:5
hello
=======
world
>>>>>>> REPLACE";
        let blocks = parse_diff_blocks(diff);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].start_line, 5);
    }

    #[test]
    fn test_parse_diff_blocks_with_dash_separator() {
        let diff = "\
<<<<<<< SEARCH
:start_line:3
-------
hello
=======
world
>>>>>>> REPLACE";
        let blocks = parse_diff_blocks(diff);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search_content, "hello");
        assert_eq!(blocks[0].start_line, 3);
    }

    #[test]
    fn test_parse_diff_blocks_multiple() {
        let diff = "\
<<<<<<< SEARCH
aaa
=======
bbb
>>>>>>> REPLACE
<<<<<<< SEARCH
ccc
=======
ddd
>>>>>>> REPLACE";
        let blocks = parse_diff_blocks(diff);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].search_content, "aaa");
        assert_eq!(blocks[1].search_content, "ccc");
    }

    #[test]
    fn test_is_search_marker() {
        assert!(is_search_marker("<<<<<<< SEARCH"));
        assert!(is_search_marker("<<<<<<< SEARCH>"));
        assert!(is_search_marker("<<<<<<< SEARCH>>"));
        assert!(!is_search_marker("\\<<<<<<< SEARCH"));
        assert!(!is_search_marker("not a marker"));
    }
}
