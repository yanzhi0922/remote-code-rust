//! Unified diff strategy for Roo Code Rust.
//!
//! Parses and applies standard unified diff format:
//! - `--- a/file` and `+++ b/file` headers
//! - `@@ ... @@` hunk headers with line/count info
//! - `+` (add), `-` (remove), ` ` (context) lines within hunks
//!
//! Port of the unified-diff strategy from the Roo Code TypeScript source.

use crate::similarity::get_similarity;
use crate::types::DiffResult;

/// A parsed hunk from a unified diff.
#[derive(Debug, Clone)]
struct Hunk {
    /// 1-based start line in the original file.
    old_start: usize,
    /// Number of lines in the original (context + removed).
    #[allow(dead_code)]
    old_count: usize,
    /// The lines of this hunk, with their prefix character (`+`, `-`, or ` `).
    lines: Vec<HunkLine>,
}

/// A single line within a hunk.
#[derive(Debug, Clone)]
struct HunkLine {
    /// The kind: Add, Remove, or Context.
    kind: HunkLineKind,
    /// The text content (without the prefix character).
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum HunkLineKind {
    Add,
    Remove,
    Context,
}

/// Unified diff strategy implementation.
///
/// Applies a unified diff to original content and returns the result.
pub struct UnifiedDiffStrategy {
    /// Fuzzy matching threshold (0.0–1.0). Default 1.0 = exact.
    fuzzy_threshold: f64,
}

impl UnifiedDiffStrategy {
    /// Create a new `UnifiedDiffStrategy`.
    ///
    /// - `fuzzy_threshold`: Similarity threshold for fuzzy matching. Defaults to 1.0 (exact).
    pub fn new(fuzzy_threshold: Option<f64>) -> Self {
        Self {
            fuzzy_threshold: fuzzy_threshold.unwrap_or(1.0),
        }
    }

    /// Returns the name of this diff strategy.
    pub fn name(&self) -> &str {
        "UnifiedDiff"
    }

    /// Apply a unified diff to the given original content.
    ///
    /// The `diff` parameter should be a standard unified diff (possibly with
    /// `--- a/...` / `+++ b/...` headers and `@@ ... @@` hunks).
    pub fn apply_diff(&self, original_content: &str, diff: &str) -> DiffResult {
        let hunks = match parse_hunks(diff) {
            Ok(h) => h,
            Err(e) => return DiffResult::fail(e),
        };

        if hunks.is_empty() {
            return DiffResult::fail(
                "No hunks found in the unified diff".to_string(),
            );
        }

        // Detect line ending
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
        let mut applied_count: usize = 0;
        let mut fail_parts: Vec<DiffResult> = Vec::new();

        for hunk in &hunks {
            let adjusted_start = hunk.old_start as i64 + delta;
            if adjusted_start < 0 {
                fail_parts.push(DiffResult::fail(format!(
                    "Hunk start line {} is out of bounds",
                    hunk.old_start
                )));
                continue;
            }
            let start_idx = adjusted_start as usize;

            // Compute the old lines (context + removed) for this hunk
            let old_lines: Vec<String> = hunk
                .lines
                .iter()
                .filter(|l| l.kind == HunkLineKind::Remove || l.kind == HunkLineKind::Context)
                .map(|l| l.text.clone())
                .collect();

            // Compute the new lines (context + added) for this hunk
            let new_lines: Vec<String> = hunk
                .lines
                .iter()
                .filter(|l| l.kind == HunkLineKind::Add || l.kind == HunkLineKind::Context)
                .map(|l| l.text.clone())
                .collect();

            // Try to find where to apply the hunk.
            // First, try at the specified location.
            let match_idx = if start_idx > 0 && start_idx <= result_lines.len() {
                let start0 = start_idx.saturating_sub(1); // convert 1-based to 0-based
                let end0 = start0 + old_lines.len();
                if end0 <= result_lines.len() {
                    let original_chunk = &result_lines[start0..end0];
                    let similarity = get_similarity(
                        &original_chunk.join("\n"),
                        &old_lines.join("\n"),
                    );
                    if similarity >= self.fuzzy_threshold {
                        Some(start0)
                    } else {
                        // Fallback: fuzzy search the whole file
                        fuzzy_find_lines(&result_lines, &old_lines, self.fuzzy_threshold)
                    }
                } else {
                    // Out of bounds at the specified location; try fuzzy search
                    fuzzy_find_lines(&result_lines, &old_lines, self.fuzzy_threshold)
                }
            } else {
                // No line info; fuzzy search
                fuzzy_find_lines(&result_lines, &old_lines, self.fuzzy_threshold)
            };

            match match_idx {
                Some(idx) => {
                    // Splice the new lines into result_lines
                    let old_count = old_lines.len();
                    result_lines.splice(idx..idx + old_count, new_lines.clone());
                    delta -= old_count as i64;
                    delta += new_lines.len() as i64;
                    applied_count += 1;
                }
                None => {
                    fail_parts.push(DiffResult::fail(format!(
                        "Could not find a matching location for hunk at line {} ({} old lines)",
                        hunk.old_start,
                        old_lines.len()
                    )));
                }
            }
        }

        let final_content = result_lines.join(line_ending);
        if applied_count == 0 {
            if fail_parts.is_empty() {
                DiffResult::fail("No hunks were applied".to_string())
            } else {
                DiffResult::fail_with_parts(fail_parts)
            }
        } else {
            DiffResult::ok(final_content, fail_parts)
        }
    }
}

/// Search for the best match of `old_lines` within `result_lines` using fuzzy matching.
fn fuzzy_find_lines(
    result_lines: &[String],
    old_lines: &[String],
    threshold: f64,
) -> Option<usize> {
    if old_lines.is_empty() || result_lines.len() < old_lines.len() {
        return None;
    }

    let search_chunk = old_lines.join("\n");
    let mut best_idx: Option<usize> = None;
    let mut best_score: f64 = 0.0;

    let max_start = result_lines.len() - old_lines.len();
    for i in 0..=max_start {
        let original_chunk = result_lines[i..i + old_lines.len()].join("\n");
        let similarity = get_similarity(&original_chunk, &search_chunk);
        if similarity > best_score {
            best_score = similarity;
            best_idx = Some(i);
        }
    }

    if best_score >= threshold {
        best_idx
    } else {
        None
    }
}

/// Parse all hunks from a unified diff string.
fn parse_hunks(diff: &str) -> Result<Vec<Hunk>, String> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let lines: Vec<&str> = diff.lines().collect();
    let mut i = 0;

    // Skip any header lines until we find the first @@ hunk header
    while i < lines.len() {
        let line = lines[i];
        // Skip `---` and `+++` header lines
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            i += 1;
            continue;
        }
        // Look for @@ hunk headers
        if let Some(hunk) = parse_hunk_header(line) {
            i += 1;
            let (hunk_lines, new_i) = parse_hunk_body(&lines, i, hunk.old_start, hunk.old_count)?;
            hunks.push(Hunk {
                old_start: hunk.old_start,
                old_count: hunk.old_count,
                lines: hunk_lines,
            });
            i = new_i;
        } else {
            i += 1;
        }
    }

    Ok(hunks)
}

struct HunkHeader {
    old_start: usize,
    old_count: usize,
}

/// Parse a hunk header line like `@@ -10,5 +12,7 @@` or `@@ -10 +12 @@`.
fn parse_hunk_header(line: &str) -> Option<HunkHeader> {
    let trimmed = line.trim();

    if !trimmed.starts_with("@@") {
        return None;
    }

    // Find the second @@ (the closing one)
    let after_first = &trimmed[2..];
    let end_at = after_first.find("@@")?;
    let range_str = after_first[..end_at].trim();

    // Parse "-old_start,old_count"
    if !range_str.starts_with('-') {
        return None;
    }
    let rest = &range_str[1..];
    // Skip the +new_start,... part
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let old_part = parts[0];
    let (old_start, old_count) = if let Some(comma_pos) = old_part.find(',') {
        let start: usize = old_part[..comma_pos].parse().ok()?;
        let count: usize = old_part[comma_pos + 1..].parse().ok()?;
        (start, count)
    } else {
        let start: usize = old_part.parse().ok()?;
        (start, 1)
    };

    Some(HunkHeader { old_start, old_count })
}

/// Parse the body of a hunk (the lines following the @@ header).
fn parse_hunk_body(
    lines: &[&str],
    start: usize,
    _expected_old_start: usize,
    _expected_old_count: usize,
) -> Result<(Vec<HunkLine>, usize), String> {
    let mut hunk_lines: Vec<HunkLine> = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];

        // End of hunk if we hit another @@ header or diff header
        if line.starts_with("@@") || line.starts_with("--- ") || line.starts_with("+++ ") {
            break;
        }

        // Lines starting with '\ No newline at end of file' are metadata; skip
        if line.starts_with("\\ ") {
            i += 1;
            continue;
        }

        // Empty line in the diff is treated as a context line (space prefix was stripped)
        if line.is_empty() {
            hunk_lines.push(HunkLine {
                kind: HunkLineKind::Context,
                text: String::new(),
            });
            i += 1;
            continue;
        }

        let first_char = line.chars().next();
        match first_char {
            Some('+') => {
                hunk_lines.push(HunkLine {
                    kind: HunkLineKind::Add,
                    text: line[1..].to_string(),
                });
            }
            Some('-') => {
                hunk_lines.push(HunkLine {
                    kind: HunkLineKind::Remove,
                    text: line[1..].to_string(),
                });
            }
            Some(' ') => {
                hunk_lines.push(HunkLine {
                    kind: HunkLineKind::Context,
                    text: line[1..].to_string(),
                });
            }
            _ => {
                // Unknown prefix; treat as context (some diffs omit the space)
                hunk_lines.push(HunkLine {
                    kind: HunkLineKind::Context,
                    text: line.to_string(),
                });
            }
        }
        i += 1;
    }

    Ok((hunk_lines, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_strategy() -> UnifiedDiffStrategy {
        UnifiedDiffStrategy::new(None)
    }

    fn make_fuzzy_strategy() -> UnifiedDiffStrategy {
        UnifiedDiffStrategy::new(Some(0.8))
    }

    #[test]
    fn test_name() {
        let strategy = make_strategy();
        assert_eq!(strategy.name(), "UnifiedDiff");
    }

    #[test]
    fn test_apply_simple_replacement() {
        let strategy = make_strategy();
        let original = "line1\nline2\nline3\n";
        let diff = "\
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,3 @@
 line1
-line2
+LINE2
 line3";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(result.content.unwrap(), "line1\nLINE2\nline3\n");
    }

    #[test]
    fn test_apply_addition() {
        let strategy = make_strategy();
        let original = "line1\nline3\n";
        let diff = "\
--- a/test.txt
+++ b/test.txt
@@ -1,2 +1,3 @@
 line1
+line2
 line3";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(result.content.unwrap(), "line1\nline2\nline3\n");
    }

    #[test]
    fn test_apply_deletion() {
        let strategy = make_strategy();
        let original = "line1\nline2\nline3\n";
        let diff = "\
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,2 @@
 line1
-line2
 line3";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(result.content.unwrap(), "line1\nline3\n");
    }

    #[test]
    fn test_apply_multiple_hunks() {
        let strategy = make_strategy();
        let original = "line1\nline2\nline3\nline4\nline5\n";
        let diff = "\
--- a/test.txt
+++ b/test.txt
@@ -1,2 +1,2 @@
-line1
+LINE1
 line2
@@ -4,2 +4,2 @@
 line4
-line5
+LINE5";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.content.unwrap(),
            "LINE1\nline2\nline3\nline4\nLINE5\n"
        );
    }

    #[test]
    fn test_apply_no_hunks() {
        let strategy = make_strategy();
        let original = "line1\nline2\n";
        let diff = "just some random text without any hunk";
        let result = strategy.apply_diff(original, diff);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_fuzzy_matching() {
        let strategy = make_fuzzy_strategy();
        let original = "function hello() {\n    console.log('hi');\n}\n";
        let diff = "\
--- a/test.js
+++ b/test.js
@@ -1,3 +1,3 @@
 function hello() {
-    console.log(\"hi\");
+    console.log(\"hello\");
 }";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
    }

    #[test]
    fn test_parse_hunk_header_basic() {
        let h = parse_hunk_header("@@ -10,5 +12,7 @@").unwrap();
        assert_eq!(h.old_start, 10);
        assert_eq!(h.old_count, 5);
    }

    #[test]
    fn test_parse_hunk_header_no_count() {
        let h = parse_hunk_header("@@ -10 +12 @@").unwrap();
        assert_eq!(h.old_start, 10);
        assert_eq!(h.old_count, 1);
    }

    #[test]
    fn test_parse_hunk_header_with_context() {
        let h = parse_hunk_header("@@ -1,3 +1,3 @@ function main()").unwrap();
        assert_eq!(h.old_start, 1);
        assert_eq!(h.old_count, 3);
    }

    #[test]
    fn test_parse_hunk_header_invalid() {
        assert!(parse_hunk_header("not a header").is_none());
        assert!(parse_hunk_header("++ some line").is_none());
    }

    #[test]
    fn test_windows_line_endings() {
        let strategy = make_strategy();
        let original = "line1\r\nline2\r\nline3\r\n";
        let diff = "\
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,3 @@
 line1
-line2
+LINE2
 line3";
        let result = strategy.apply_diff(original, diff);
        assert!(result.success, "Expected success, got error: {:?}", result.error);
        assert_eq!(result.content.unwrap(), "line1\r\nLINE2\r\nline3\r\n");
    }
}
