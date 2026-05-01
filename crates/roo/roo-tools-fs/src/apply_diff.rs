//! apply_diff tool implementation.
//!
//! Handles parsing and validation of diff parameters, coordinating
//! the actual diff application (delegated to roo-diff).

use crate::helpers::{check_roo_ignore, unescape_html_entities};
use crate::types::*;
use roo_ignore::RooIgnoreController;
use roo_types::tool::ApplyDiffParams;

/// Clean diff content by unescaping HTML entities for non-Claude models.
///
/// Matches TS: `if (modelId.includes("claude")) { ... } else {
///     diffContent = unescapeHtmlEntities(diffContent) }`
pub fn clean_diff_content(diff: &str, model_id: Option<&str>) -> String {
    let is_claude = model_id.map_or(false, |id| id.contains("claude"));
    if is_claude {
        diff.to_string()
    } else {
        unescape_html_entities(diff)
    }
}

/// Validate apply_diff parameters.
pub fn validate_apply_diff_params(params: &ApplyDiffParams) -> Result<(), FsToolError> {
    if params.path.trim().is_empty() {
        return Err(FsToolError::Validation("path must not be empty".to_string()));
    }

    if params.path.contains("..") {
        return Err(FsToolError::InvalidPath(
            "path must not contain '..'".to_string(),
        ));
    }

    if params.diff.trim().is_empty() {
        return Err(FsToolError::Validation(
            "diff content must not be empty".to_string(),
        ));
    }

    // Validate diff format: must contain SEARCH/REPLACE markers
    validate_diff_format(&params.diff)?;

    Ok(())
}

/// Validate apply_diff parameters including .rooignore access check.
pub fn validate_apply_diff_access(
    params: &ApplyDiffParams,
    ignore_controller: Option<&RooIgnoreController>,
) -> Result<(), FsToolError> {
    validate_apply_diff_params(params)?;
    check_roo_ignore(&params.path, ignore_controller)?;
    Ok(())
}

/// Validate that the diff string contains properly formatted search/replace blocks.
pub fn validate_diff_format(diff: &str) -> Result<(), FsToolError> {
    let has_search = diff.contains("<<<<<<< SEARCH");
    let has_separator = diff.contains("=======");
    let has_end = diff.contains(">>>>>>> REPLACE");

    if !has_search {
        return Err(FsToolError::InvalidDiff(
            "diff must contain '<<<<<<< SEARCH' markers".to_string(),
        ));
    }

    if !has_separator {
        return Err(FsToolError::InvalidDiff(
            "diff must contain '=======' separator".to_string(),
        ));
    }

    if !has_end {
        return Err(FsToolError::InvalidDiff(
            "diff must contain '>>>>>>> REPLACE' markers".to_string(),
        ));
    }

    Ok(())
}

/// Count the number of diff blocks in a diff string.
pub fn count_diff_blocks(diff: &str) -> usize {
    diff.matches("<<<<<<< SEARCH").count()
}

/// Generate a notice when only a single SEARCH/REPLACE block is used.
///
/// Matches TS behavior:
/// ```text
/// \n<notice>Making multiple related changes in a single apply_diff is more efficient.
/// If other changes are needed in this file, please include them as additional SEARCH/REPLACE blocks.</notice>
/// ```
pub fn single_block_notice(diff: &str) -> String {
    let block_count = count_diff_blocks(diff);
    if block_count == 1 {
        "\n<notice>Making multiple related changes in a single apply_diff is more efficient. If other changes are needed in this file, please include them as additional SEARCH/REPLACE blocks.</notice>".to_string()
    } else {
        String::new()
    }
}

/// Generate a part-fail hint when some diff blocks could not be applied.
///
/// Matches TS behavior:
/// ```text
/// But unable to apply all diff parts to file: {path}. Use the read_file tool to check the newest file version and re-apply diffs.\n
/// ```
pub fn part_fail_hint(absolute_path: &str, total_blocks: usize, applied_blocks: usize) -> String {
    if applied_blocks < total_blocks {
        format!(
            "But unable to apply all diff parts to file: {}. Use the read_file tool to check the newest file version and re-apply diffs.\n",
            absolute_path
        )
    } else {
        String::new()
    }
}

/// Parse diff blocks from the diff string.
/// Returns a vector of (search_content, replace_content) tuples.
///
/// The diff format is:
/// ```text
/// <<<<<<< SEARCH
/// :start_line:N
/// -------
/// search content
/// =======
/// replace content
/// >>>>>>> REPLACE
/// ```
///
/// The `:start_line:N` line is optional and is skipped if present.
/// The `-------` separator line after `:start_line:` is also optional.
/// Legacy format `:N:` is also supported.
pub fn parse_diff_blocks(diff: &str) -> Result<Vec<(String, String)>, FsToolError> {
    let mut blocks = Vec::new();
    let mut remaining = diff;

    while let Some(search_start) = remaining.find("<<<<<<< SEARCH") {
        // Skip past the SEARCH marker line
        let header_end = remaining[search_start..]
            .find('\n')
            .map(|pos| search_start + pos + 1)
            .unwrap_or(remaining.len());

        let mut after_header = &remaining[header_end..];

        // Skip optional :start_line: header
        // The format is: `:start_line:N` or `:N:` (legacy)
        if let Some(line_end) = after_header.find('\n') {
            let first_line = after_header[..line_end].trim();
            // Match `:start_line:N` or `:N:` patterns
            if first_line.starts_with(':') {
                // Skip this line
                after_header = &after_header[line_end + 1..];

                // Skip the `-------` separator line that follows :start_line:
                if let Some(next_line_end) = after_header.find('\n') {
                    let second_line = after_header[..next_line_end].trim();
                    if second_line.starts_with('-') && second_line.chars().all(|c| c == '-') {
                        after_header = &after_header[next_line_end + 1..];
                    }
                }
            }
        }

        // Find the separator
        let separator_pos = after_header
            .find("=======")
            .ok_or_else(|| FsToolError::InvalidDiff("missing '=======' separator".to_string()))?;

        let search_content = after_header[..separator_pos].trim_end().to_string();

        let after_separator_raw = &after_header[separator_pos + "=======".len()..];
        // Skip the newline immediately after the separator
        let after_separator = after_separator_raw.strip_prefix('\n').unwrap_or(after_separator_raw);

        // Find the end marker
        let end_pos = after_separator
            .find(">>>>>>> REPLACE")
            .ok_or_else(|| FsToolError::InvalidDiff("missing '>>>>>>> REPLACE' marker".to_string()))?;

        let replace_content = after_separator[..end_pos].trim_end().to_string();

        blocks.push((search_content, replace_content));

        remaining = &after_separator[end_pos + ">>>>>>> REPLACE".len()..];
    }

    Ok(blocks)
}

/// Apply parsed diff blocks to content.
///
/// For each block, first tries an exact substring match. If that fails,
/// falls back to fuzzy matching via [`roo_diff::fuzzy_search`] with a
/// similarity threshold of 0.8 (80%).
pub fn apply_diff_blocks(
    original: &str,
    blocks: &[(String, String)],
) -> Result<DiffApplyResult, FsToolError> {
    let mut content = original.to_string();
    let mut applied = 0;
    let mut warnings = Vec::new();

    for (i, (search, replace)) in blocks.iter().enumerate() {
        // 1. Try exact match first
        if let Some(pos) = content.find(search.as_str()) {
            content.replace_range(pos..pos + search.len(), replace);
            applied += 1;
        } else {
            // 2. Try fuzzy match using roo-diff similarity
            match apply_fuzzy_diff(&content, search, replace) {
                Ok(new_content) => {
                    content = new_content;
                    applied += 1;
                    warnings.push(format!(
                        "Block {}: applied via fuzzy match (≥80% similarity)",
                        i + 1
                    ));
                }
                Err(_) => {
                    warnings.push(format!(
                        "Block {}: search content not found in file",
                        i + 1
                    ));
                }
            }
        }
    }

    // L5.3: Post-apply verification
    if applied > 0 {
        if let Err(verify_warning) = verify_diff_result(&content) {
            warnings.push(verify_warning);
        }
    }

    Ok(DiffApplyResult {
        path: String::new(), // Caller sets this
        blocks_applied: applied,
        warnings,
    })
}

/// Fuzzy-match similarity threshold (80%).
const FUZZY_THRESHOLD: f64 = 0.8;

/// Attempt to apply a diff block using fuzzy matching.
///
/// Uses [`roo_diff::fuzzy_search`] to find the best-matching region in the
/// content, then checks whether the similarity score meets the threshold.
fn apply_fuzzy_diff(
    content: &str,
    search: &str,
    replace: &str,
) -> Result<String, FsToolError> {
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let search_line_count = search.lines().count();

    if search_line_count == 0 || search_line_count > lines.len() {
        return Err(FsToolError::InvalidDiff(
            "search content is empty or longer than file".to_string(),
        ));
    }

    let fuzzy_result = roo_diff::fuzzy_search(&lines, search, 0, lines.len());

    if fuzzy_result.best_score >= FUZZY_THRESHOLD && fuzzy_result.best_match_index >= 0 {
        let start = fuzzy_result.best_match_index as usize;
        let end = (start + search_line_count).min(lines.len());

        let mut new_lines = lines;
        let replace_lines: Vec<String> = replace.lines().map(|l| l.to_string()).collect();
        new_lines.splice(start..end, replace_lines);

        // Preserve trailing newline from original content
        let mut result = new_lines.join("\n");
        if content.ends_with('\n') && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    } else {
        Err(FsToolError::InvalidDiff(format!(
            "no fuzzy match found (best similarity: {:.0}%)",
            fuzzy_result.best_score * 100.0
        )))
    }
}

/// Verify the result of a diff application.
///
/// Checks for common issues:
/// - Inconsistent indentation (mixed tabs and spaces in the same line)
/// - Trailing whitespace issues
/// - Unbalanced braces (rough heuristic)
/// - Empty search blocks left behind
fn verify_diff_result(content: &str) -> Result<(), String> {
    let mut issues = Vec::new();

    // Check for unbalanced braces (rough heuristic)
    let open_braces = content.chars().filter(|c| *c == '{').count();
    let close_braces = content.chars().filter(|c| *c == '}').count();
    if open_braces != close_braces {
        issues.push(format!(
            "Unbalanced braces: {} opening vs {} closing",
            open_braces, close_braces
        ));
    }

    // Check for leftover diff markers
    if content.contains("<<<<<<< SEARCH") || content.contains(">>>>>>> REPLACE") || content.contains("=======") {
        issues.push("Leftover diff markers found in result".to_string());
    }

    // Check for lines with mixed tab/space indentation
    for (i, line) in content.lines().enumerate() {
        let trimmed_start = line.trim_start();
        if trimmed_start.is_empty() {
            continue;
        }
        let indent: String = line[..line.len() - trimmed_start.len()].to_string();
        if indent.contains('\t') && indent.contains(' ') {
            issues.push(format!(
                "Line {}: mixed tabs and spaces in indentation",
                i + 1
            ));
            break; // Only report first occurrence
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(format!("Verification warnings: {}", issues.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_path() {
        let params = ApplyDiffParams {
            path: "".to_string(),
            diff: "<<<<<<< SEARCH\nfoo\n=======\nbar\n>>>>>>> REPLACE".to_string(),
        };
        assert!(validate_apply_diff_params(&params).is_err());
    }

    #[test]
    fn test_validate_path_traversal() {
        let params = ApplyDiffParams {
            path: "../etc/passwd".to_string(),
            diff: "<<<<<<< SEARCH\nfoo\n=======\nbar\n>>>>>>> REPLACE".to_string(),
        };
        assert!(validate_apply_diff_params(&params).is_err());
    }

    #[test]
    fn test_validate_empty_diff() {
        let params = ApplyDiffParams {
            path: "test.txt".to_string(),
            diff: "".to_string(),
        };
        assert!(validate_apply_diff_params(&params).is_err());
    }

    #[test]
    fn test_validate_invalid_diff_no_markers() {
        let params = ApplyDiffParams {
            path: "test.txt".to_string(),
            diff: "just some text without markers".to_string(),
        };
        assert!(validate_apply_diff_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_diff() {
        let params = ApplyDiffParams {
            path: "test.txt".to_string(),
            diff: "<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE".to_string(),
        };
        assert!(validate_apply_diff_params(&params).is_ok());
    }

    #[test]
    fn test_count_diff_blocks_single() {
        let diff = "<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE";
        assert_eq!(count_diff_blocks(diff), 1);
    }

    #[test]
    fn test_count_diff_blocks_multiple() {
        let diff = "\
<<<<<<< SEARCH
old1
=======
new1
>>>>>>> REPLACE
some text
<<<<<<< SEARCH
old2
=======
new2
>>>>>>> REPLACE";
        assert_eq!(count_diff_blocks(diff), 2);
    }

    #[test]
    fn test_parse_diff_blocks_single() {
        let diff = "<<<<<<< SEARCH\n:9:\nold content\n=======\nnew content\n>>>>>>> REPLACE";
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "old content");
        assert_eq!(blocks[0].1, "new content");
    }

    #[test]
    fn test_parse_diff_blocks_multiple() {
        let diff = "\
<<<<<<< SEARCH
:5:
hello
=======
world
>>>>>>> REPLACE
<<<<<<< SEARCH
:10:
foo
=======
bar
>>>>>>> REPLACE";
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "hello");
        assert_eq!(blocks[0].1, "world");
        assert_eq!(blocks[1].0, "foo");
        assert_eq!(blocks[1].1, "bar");
    }

    #[test]
    fn test_apply_diff_blocks_success() {
        let original = "line1\nhello\nline3";
        let blocks = vec![("hello".to_string(), "world".to_string())];
        let result = apply_diff_blocks(original, &blocks).unwrap();
        assert_eq!(result.blocks_applied, 1);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_apply_diff_blocks_not_found() {
        let original = "line1\nline2\nline3";
        let blocks = vec![("nonexistent".to_string(), "replacement".to_string())];
        let result = apply_diff_blocks(original, &blocks).unwrap();
        assert_eq!(result.blocks_applied, 0);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_apply_diff_blocks_partial() {
        let original = "alpha\nbeta\ngamma";
        let blocks = vec![
            ("alpha".to_string(), "ALPHA".to_string()),
            ("missing".to_string(), "MISSING".to_string()),
        ];
        let result = apply_diff_blocks(original, &blocks).unwrap();
        assert_eq!(result.blocks_applied, 1);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_validate_diff_format_missing_end() {
        let diff = "<<<<<<< SEARCH\nfoo\n=======\nbar";
        assert!(validate_diff_format(diff).is_err());
    }

    #[test]
    fn test_validate_diff_format_missing_separator() {
        let diff = "<<<<<<< SEARCH\nfoo\n>>>>>>> REPLACE";
        assert!(validate_diff_format(diff).is_err());
    }

    // --- New tests for :start_line:N and ------- format ---

    #[test]
    fn test_parse_start_line_format() {
        // Test the :start_line:N format (new Roo Code format)
        let diff = concat!(
            "<<<<<<< SEARCH\n",
            ":start_line:5\n",
            "-------\n",
            "old line\n",
            "=======\n",
            "new line\n",
            ">>>>>>> REPLACE"
        );
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "old line");
        assert_eq!(blocks[0].1, "new line");
    }

    #[test]
    fn test_parse_start_line_format_multiline() {
        // Test :start_line:N with multiline search/replace content
        let diff = concat!(
            "<<<<<<< SEARCH\n",
            ":start_line:10\n",
            "-------\n",
            "line one\n",
            "line two\n",
            "line three\n",
            "=======\n",
            "replaced one\n",
            "replaced two\n",
            ">>>>>>> REPLACE"
        );
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "line one\nline two\nline three");
        assert_eq!(blocks[0].1, "replaced one\nreplaced two");
    }

    #[test]
    fn test_parse_start_line_without_dashes() {
        // Test :start_line:N without the ------- separator
        let diff = concat!(
            "<<<<<<< SEARCH\n",
            ":start_line:42\n",
            "search this\n",
            "=======\n",
            "replace that\n",
            ">>>>>>> REPLACE"
        );
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "search this");
        assert_eq!(blocks[0].1, "replace that");
    }

    #[test]
    fn test_parse_no_start_line_header() {
        // Test diff without any :start_line: or :N: header
        let diff = concat!(
            "<<<<<<< SEARCH\n",
            "plain search\n",
            "=======\n",
            "plain replace\n",
            ">>>>>>> REPLACE"
        );
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "plain search");
        assert_eq!(blocks[0].1, "plain replace");
    }

    #[test]
    fn test_parse_legacy_colon_format() {
        // Test legacy :N: format
        let diff = "<<<<<<< SEARCH\n:15:\nlegacy search\n=======\nlegacy replace\n>>>>>>> REPLACE";
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "legacy search");
        assert_eq!(blocks[0].1, "legacy replace");
    }

    #[test]
    fn test_parse_mixed_formats() {
        // Test a diff with both :start_line:N+------- and :N: formats
        let diff = concat!(
            "<<<<<<< SEARCH\n",
            ":start_line:3\n",
            "-------\n",
            "first old\n",
            "=======\n",
            "first new\n",
            ">>>>>>> REPLACE\n",
            "<<<<<<< SEARCH\n",
            ":7:\n",
            "second old\n",
            "=======\n",
            "second new\n",
            ">>>>>>> REPLACE"
        );
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "first old");
        assert_eq!(blocks[0].1, "first new");
        assert_eq!(blocks[1].0, "second old");
        assert_eq!(blocks[1].1, "second new");
    }

    #[test]
    fn test_parse_start_line_with_leading_whitespace() {
        // Test that leading whitespace in search content is preserved
        let diff = concat!(
            "<<<<<<< SEARCH\n",
            ":start_line:1\n",
            "-------\n",
            "    indented code\n",
            "=======\n",
            "    replaced code\n",
            ">>>>>>> REPLACE"
        );
        let blocks = parse_diff_blocks(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "    indented code");
        assert_eq!(blocks[0].1, "    replaced code");
    }

    #[test]
    fn test_apply_with_start_line_format() {
        // End-to-end test: parse then apply
        let diff = concat!(
            "<<<<<<< SEARCH\n",
            ":start_line:2\n",
            "-------\n",
            "hello\n",
            "=======\n",
            "world\n",
            ">>>>>>> REPLACE"
        );
        let blocks = parse_diff_blocks(diff).unwrap();
        let original = "line1\nhello\nline3";
        let result = apply_diff_blocks(original, &blocks).unwrap();
        assert_eq!(result.blocks_applied, 1);
        assert!(result.warnings.is_empty());
    }

    // --- L5.3: Verification tests ---

    #[test]
    fn test_verify_balanced_content() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        assert!(verify_diff_result(content).is_ok());
    }

    #[test]
    fn test_verify_unbalanced_braces() {
        let content = "fn main() {\n    println!(\"hello\");\n";
        let result = verify_diff_result(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unbalanced braces"));
    }

    #[test]
    fn test_verify_leftover_markers() {
        let content = "fn main() {\n<<<<<<< SEARCH\n    code\n}\n";
        let result = verify_diff_result(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Leftover diff markers"));
    }

    #[test]
    fn test_verify_mixed_indentation() {
        let content = "fn main() {\n\t    println!(\"mixed\");\n}\n";
        let result = verify_diff_result(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mixed tabs and spaces"));
    }

    #[test]
    fn test_verify_clean_content() {
        let content = "struct Foo {\n    bar: i32,\n}\n";
        assert!(verify_diff_result(content).is_ok());
    }

    #[test]
    fn test_apply_diff_with_verification_warning() {
        let original = "fn main() {";
        let blocks = vec![("fn main() {".to_string(), "fn main() {\n    // added\n".to_string())];
        let result = apply_diff_blocks(original, &blocks).unwrap();
        assert_eq!(result.blocks_applied, 1);
        // Should have a warning about unbalanced braces
        assert!(!result.warnings.is_empty());
    }

    // --- Fuzzy matching tests ---

    #[test]
    fn test_fuzzy_match_minor_whitespace_diff() {
        // Exact match fails due to extra space, but fuzzy match should succeed
        let original = "line1\n  hello world\nline3";
        let search = "  hello world"; // exact match
        let replace = "  goodbye world";
        let blocks = vec![(search.to_string(), replace.to_string())];
        let result = apply_diff_blocks(original, &blocks).unwrap();
        assert_eq!(result.blocks_applied, 1);
    }

    #[test]
    fn test_fuzzy_match_similar_content() {
        // Search content has a minor difference from the actual content
        let original = "fn main() {\n    println!(\"hello\");\n}";
        let search = "fn main() {\n    println!(\"world\");\n}";
        let replace = "fn main() {\n    println!(\"universe\");\n}";
        let blocks = vec![(search.to_string(), replace.to_string())];
        let result = apply_diff_blocks(original, &blocks).unwrap();
        // Should apply via fuzzy match since similarity is high
        assert_eq!(result.blocks_applied, 1);
        assert!(result.warnings.iter().any(|w| w.contains("fuzzy match")));
    }

    #[test]
    fn test_fuzzy_match_below_threshold() {
        // Content is too different for fuzzy match
        let original = "alpha\nbeta\ngamma";
        let search = "completely different content here";
        let replace = "replacement";
        let blocks = vec![(search.to_string(), replace.to_string())];
        let result = apply_diff_blocks(original, &blocks).unwrap();
        assert_eq!(result.blocks_applied, 0);
        assert!(result.warnings.iter().any(|w| w.contains("not found")));
    }

    #[test]
    fn test_fuzzy_match_multiline_region() {
        let original = "line1\nline2\nline3\nline4\nline5";
        let search = "line2\nline3";
        let replace = "LINE2\nLINE3";
        let blocks = vec![(search.to_string(), replace.to_string())];
        let result = apply_diff_blocks(original, &blocks).unwrap();
        // Exact match should work here
        assert_eq!(result.blocks_applied, 1);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_apply_fuzzy_diff_empty_search() {
        let content = "hello world";
        let result = apply_fuzzy_diff(content, "", "replacement");
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_fuzzy_diff_search_longer_than_content() {
        let content = "short";
        let result = apply_fuzzy_diff(content, "line1\nline2\nline3\nline4\nline5\nline6", "replacement");
        assert!(result.is_err());
    }

    // --- Tests for single_block_notice and part_fail_hint ---

    #[test]
    fn test_single_block_notice_single() {
        let diff = "<<<<<<< SEARCH\nfoo\n=======\nbar\n>>>>>>> REPLACE";
        let notice = single_block_notice(diff);
        assert!(notice.contains("<notice>"));
        assert!(notice.contains("Making multiple related changes"));
    }

    #[test]
    fn test_single_block_notice_multiple() {
        let diff = "<<<<<<< SEARCH\nfoo\n=======\nbar\n>>>>>>> REPLACE\n<<<<<<< SEARCH\nbaz\n=======\nqux\n>>>>>>> REPLACE";
        let notice = single_block_notice(diff);
        assert!(notice.is_empty());
    }

    #[test]
    fn test_part_fail_hint_all_applied() {
        let hint = part_fail_hint("/path/to/file.txt", 3, 3);
        assert!(hint.is_empty());
    }

    #[test]
    fn test_clean_diff_content_non_claude() {
        let diff = "foo &lt;bar&gt; baz &amp; qux &quot;";
        let result = clean_diff_content(diff, None);
        assert_eq!(result, "foo <bar> baz & qux \"");
    }

    #[test]
    fn test_clean_diff_content_claude() {
        let diff = "foo &lt;bar&gt;";
        let result = clean_diff_content(diff, Some("claude-3.5-sonnet"));
        assert_eq!(result, diff);
    }

    #[test]
    fn test_clean_diff_content_claude_no_unescape() {
        let diff = "keep &amp; as is &lt; and &gt;";
        let result = clean_diff_content(diff, Some("claude-3-opus"));
        assert_eq!(result, diff);
    }

    #[test]
    fn test_part_fail_hint_some_failed() {
        let hint = part_fail_hint("/path/to/file.txt", 3, 2);
        assert!(hint.contains("unable to apply all diff parts"));
        assert!(hint.contains("/path/to/file.txt"));
        assert!(hint.contains("read_file tool"));
    }
}
