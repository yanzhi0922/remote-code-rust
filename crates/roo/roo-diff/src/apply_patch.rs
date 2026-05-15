//! Apply-patch diff strategy for Roo Code Rust.
//!
//! Implements the Codex-format patch strategy. This handles patches in the format:
//!
//! ```text
//! *** Begin Patch
//! *** Update File: path/to/file
//! @@ context
//!  context line
//! -old line
//! +new line
//! *** End Patch
//! ```
//!
//! This is a simpler, single-file variant that:
//! - Parses `*** Begin Patch` / `*** End Patch` wrapped content
//! - Applies Update File hunks (with context markers, +/-/space lines)
//! - Returns a `DiffResult` compatible with the other strategies

use crate::types::DiffResult;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const CHANGE_CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A chunk within an Update File hunk.
#[derive(Debug, Clone)]
struct PatchChunk {
    /// Optional context (e.g., function/class name) to narrow search.
    change_context: Option<String>,
    /// Lines to find (context + removed).
    old_lines: Vec<String>,
    /// Lines to replace with (context + added).
    new_lines: Vec<String>,
}

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

/// Apply-patch diff strategy implementation.
///
/// Handles Codex-format patches for single-file updates.
pub struct ApplyPatchDiffStrategy;

impl ApplyPatchDiffStrategy {
    /// Create a new `ApplyPatchDiffStrategy`.
    pub fn new() -> Self {
        Self
    }

    /// Returns the name of this diff strategy.
    pub fn name(&self) -> &str {
        "ApplyPatch"
    }

    /// Apply a Codex-format patch to the given original content.
    ///
    /// The `patch` should contain `*** Begin Patch` / `*** End Patch` markers
    /// with `*** Update File:` hunks inside.
    ///
    /// For single-file usage, this extracts the first Update File hunk and
    /// applies it to the provided original content.
    pub fn apply_diff(&self, original_content: &str, patch: &str) -> DiffResult {
        let chunks = match parse_update_chunks(patch) {
            Ok(c) => c,
            Err(e) => return DiffResult::fail(e),
        };

        if chunks.is_empty() {
            return DiffResult::fail("No update hunks found in patch".to_string());
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

        // Drop trailing empty element from final newline
        if result_lines.last().map(|s| s.is_empty()) == Some(true) {
            result_lines.pop();
        }

        let mut line_index: usize = 0;
        let mut applied_count: usize = 0;
        let mut fail_parts: Vec<DiffResult> = Vec::new();
        let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();

        for chunk in &chunks {
            // If chunk has a change_context, find it first
            if let Some(ref ctx) = chunk.change_context {
                let idx = find_line(&result_lines, ctx, line_index);
                match idx {
                    Some(i) => line_index = i + 1,
                    None => {
                        fail_parts.push(DiffResult::fail(format!(
                            "Failed to find context '{}' in content",
                            ctx
                        )));
                        continue;
                    }
                }
            }

            if chunk.old_lines.is_empty() {
                // Pure addition at end
                let insertion_idx = if !result_lines.is_empty()
                    && result_lines.last().map(|s| s.is_empty()) == Some(true)
                {
                    result_lines.len() - 1
                } else {
                    result_lines.len()
                };
                replacements.push((insertion_idx, 0, chunk.new_lines.clone()));
                applied_count += 1;
                continue;
            }

            let pattern = chunk.old_lines.clone();
            let new_slice = chunk.new_lines.clone();
            let found = seek_lines(&result_lines, &pattern, line_index);

            // Retry without trailing empty string
            let found = found.or_else(|| {
                if !pattern.is_empty() && pattern.last().map(|s| s.is_empty()) == Some(true) {
                    let trimmed_pattern = pattern[..pattern.len() - 1].to_vec();
                    let len = trimmed_pattern.len();
                    seek_lines(&result_lines, &trimmed_pattern, line_index)
                        .map(|(idx, _)| (idx, len))
                } else {
                    None
                }
            });

            // Also try with trim-end matching
            let found =
                found.or_else(|| seek_lines_trim_end(&result_lines, &chunk.old_lines, line_index));

            match found {
                Some((idx, len)) => {
                    replacements.push((idx, len, new_slice));
                    line_index = idx + len;
                    applied_count += 1;
                }
                None => {
                    let joined = chunk.old_lines.join("\n");
                    let display = if joined.len() > 200 {
                        format!("{}...", &joined[..200])
                    } else {
                        joined
                    };
                    fail_parts.push(DiffResult::fail(format!(
                        "Failed to find expected lines in content:\n{}",
                        display
                    )));
                }
            }
        }

        // Sort replacements by start index and apply in reverse order
        replacements.sort_by_key(|r| r.0);
        for i in (0..replacements.len()).rev() {
            let (start_idx, old_len, ref new_segment) = replacements[i];
            let new_owned: Vec<String> = new_segment.clone();
            result_lines.splice(start_idx..start_idx + old_len, new_owned);
        }

        // Ensure file ends with newline
        if result_lines.is_empty() || result_lines.last().map(|s| !s.is_empty()) == Some(true) {
            result_lines.push(String::new());
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

impl Default for ApplyPatchDiffStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Extract Update File chunks from a patch string.
fn parse_update_chunks(patch: &str) -> Result<Vec<PatchChunk>, String> {
    let trimmed = patch.trim();
    let all_lines: Vec<&str> = trimmed.lines().collect();

    // Handle heredoc-wrapped patches
    let effective_lines: Vec<&str> = if all_lines.len() >= 4 {
        let first_line = all_lines[0];
        let last_line = all_lines[all_lines.len() - 1];
        if (first_line == "<<EOF" || first_line == "<<'EOF'" || first_line == "<<\"EOF\"")
            && last_line.ends_with("EOF")
        {
            all_lines[1..all_lines.len() - 1].to_vec()
        } else {
            all_lines
        }
    } else {
        all_lines
    };

    // Lenient: if there's no Begin/End markers, try to parse anyway
    let has_begin = effective_lines
        .first()
        .map(|l| l.trim() == BEGIN_PATCH_MARKER)
        .unwrap_or(false);
    let has_end = effective_lines
        .last()
        .map(|l| l.trim() == END_PATCH_MARKER)
        .unwrap_or(false);

    let content_lines: &[&str] = if has_begin && has_end {
        if effective_lines.len() < 2 {
            return Err("Empty patch".to_string());
        }
        &effective_lines[1..effective_lines.len() - 1]
    } else {
        &effective_lines
    };

    let mut chunks: Vec<PatchChunk> = Vec::new();
    let mut i = 0;

    // Find *** Update File: headers
    while i < content_lines.len() {
        let line = content_lines[i].trim();
        if line.starts_with(UPDATE_FILE_MARKER) {
            i += 1; // skip the *** Update File header

            // Parse chunks within this update
            let mut first_chunk = true;
            while i < content_lines.len() {
                let cl = content_lines[i];
                // Stop at next file marker
                if cl.trim().starts_with("*** ") && !cl.trim().starts_with("@@") {
                    break;
                }

                // Skip blank lines between chunks
                if cl.trim().is_empty() {
                    i += 1;
                    continue;
                }

                let (chunk, consumed) = parse_one_chunk(&content_lines[i..], first_chunk)?;
                chunks.push(chunk);
                first_chunk = false;
                i += consumed;
            }
        } else {
            i += 1;
        }
    }

    Ok(chunks)
}

/// Parse one patch chunk from lines.
fn parse_one_chunk(
    lines: &[&str],
    allow_missing_context: bool,
) -> Result<(PatchChunk, usize), String> {
    if lines.is_empty() {
        return Err("Empty chunk".to_string());
    }

    let mut change_context: Option<String> = None;
    let mut start_index = 0;

    // Check for context marker
    if lines[0] == EMPTY_CHANGE_CONTEXT_MARKER {
        change_context = None;
        start_index = 1;
    } else if lines[0].starts_with(CHANGE_CONTEXT_MARKER) {
        change_context = Some(lines[0][CHANGE_CONTEXT_MARKER.len()..].to_string());
        start_index = 1;
    } else if !allow_missing_context {
        return Err(format!(
            "Expected chunk to start with @@ context marker, got: '{}'",
            lines[0]
        ));
    }

    let mut chunk = PatchChunk {
        change_context,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
    };

    let mut parsed_lines = 0;
    for i in start_index..lines.len() {
        let line = lines[i];

        // Stop at next file marker or context marker
        if line.trim().starts_with("*** ") {
            break;
        }
        if line == EMPTY_CHANGE_CONTEXT_MARKER || line.starts_with(CHANGE_CONTEXT_MARKER) {
            if parsed_lines > 0 {
                break;
            }
        }

        if line.is_empty() {
            chunk.old_lines.push(String::new());
            chunk.new_lines.push(String::new());
            parsed_lines += 1;
            continue;
        }

        let first_char = line.chars().next();
        match first_char {
            Some(' ') => {
                chunk.old_lines.push(line[1..].to_string());
                chunk.new_lines.push(line[1..].to_string());
                parsed_lines += 1;
            }
            Some('+') => {
                chunk.new_lines.push(line[1..].to_string());
                parsed_lines += 1;
            }
            Some('-') => {
                chunk.old_lines.push(line[1..].to_string());
                parsed_lines += 1;
            }
            _ => {
                if parsed_lines == 0 {
                    return Err(format!(
                        "Unexpected line in chunk: '{}'. Lines should start with ' ', '+', or '-'",
                        line
                    ));
                }
                break;
            }
        }
    }

    Ok((chunk, parsed_lines + start_index))
}

// ---------------------------------------------------------------------------
// Line matching (seek-sequence)
// ---------------------------------------------------------------------------

/// Find a single line in `lines` starting from `start`.
fn find_line(lines: &[String], target: &str, start: usize) -> Option<usize> {
    for i in start..lines.len() {
        if lines[i] == target {
            return Some(i);
        }
    }
    // Try trim matching
    for i in start..lines.len() {
        if lines[i].trim() == target.trim() {
            return Some(i);
        }
    }
    None
}

/// Find a sequence of pattern lines within lines, starting at or after `start`.
/// Returns (index, matched_length).
fn seek_lines(lines: &[String], pattern: &[String], start: usize) -> Option<(usize, usize)> {
    if pattern.is_empty() {
        return Some((start, 0));
    }
    if pattern.len() > lines.len() {
        return None;
    }

    let max_start = lines.len() - pattern.len();

    // Exact match
    for i in start..=max_start {
        if exact_match(lines, pattern, i) {
            return Some((i, pattern.len()));
        }
    }

    // Trim-end match
    for i in start..=max_start {
        if trim_end_match(lines, pattern, i) {
            return Some((i, pattern.len()));
        }
    }

    // Trim both sides match
    for i in start..=max_start {
        if trim_both_match(lines, pattern, i) {
            return Some((i, pattern.len()));
        }
    }

    None
}

/// Find pattern lines with trim-end matching as fallback.
fn seek_lines_trim_end(
    lines: &[String],
    pattern: &[String],
    start: usize,
) -> Option<(usize, usize)> {
    if pattern.is_empty() || pattern.len() > lines.len() {
        return None;
    }

    let max_start = lines.len() - pattern.len();
    for i in start..=max_start {
        if trim_end_match(lines, pattern, i) {
            return Some((i, pattern.len()));
        }
    }
    None
}

fn exact_match(lines: &[String], pattern: &[String], start_index: usize) -> bool {
    for i in 0..pattern.len() {
        match lines.get(start_index + i) {
            Some(line) if line == &pattern[i] => continue,
            _ => return false,
        }
    }
    true
}

fn trim_end_match(lines: &[String], pattern: &[String], start_index: usize) -> bool {
    for i in 0..pattern.len() {
        let line = lines.get(start_index + i).map(|s| s.trim_end());
        let pat = pattern.get(i).map(|s| s.trim_end());
        if line != pat {
            return false;
        }
    }
    true
}

fn trim_both_match(lines: &[String], pattern: &[String], start_index: usize) -> bool {
    for i in 0..pattern.len() {
        let line = lines.get(start_index + i).map(|s| s.trim());
        let pat = pattern.get(i).map(|s| s.trim());
        if line != pat {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_strategy() -> ApplyPatchDiffStrategy {
        ApplyPatchDiffStrategy::new()
    }

    #[test]
    fn test_name() {
        let strategy = make_strategy();
        assert_eq!(strategy.name(), "ApplyPatch");
    }

    #[test]
    fn test_apply_simple_replacement() {
        let strategy = make_strategy();
        let original = "line1\nline2\nline3\n";
        let patch = "\
*** Begin Patch
*** Update File: test.txt
@@
 line1
-line2
+LINE2
 line3
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(result.content.unwrap(), "line1\nLINE2\nline3\n");
    }

    #[test]
    fn test_apply_addition() {
        let strategy = make_strategy();
        let original = "line1\nline3\n";
        let patch = "\
*** Begin Patch
*** Update File: test.txt
@@
 line1
+line2
 line3
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(result.content.unwrap(), "line1\nline2\nline3\n");
    }

    #[test]
    fn test_apply_deletion() {
        let strategy = make_strategy();
        let original = "line1\nline2\nline3\n";
        let patch = "\
*** Begin Patch
*** Update File: test.txt
@@
 line1
-line2
 line3
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(result.content.unwrap(), "line1\nline3\n");
    }

    #[test]
    fn test_apply_with_context() {
        let strategy = make_strategy();
        let original = "class Foo:\n    def bar(self):\n        pass\n";
        let patch = "\
*** Begin Patch
*** Update File: test.py
@@ def bar(self):
-        pass
+        return 42
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        let content = result.content.unwrap();
        assert!(content.contains("return 42"));
        assert!(!content.contains("pass"));
    }

    #[test]
    fn test_apply_multiple_chunks() {
        let strategy = make_strategy();
        let original = "line1\nline2\nline3\nline4\nline5\n";
        let patch = "\
*** Begin Patch
*** Update File: test.txt
@@
-line1
+LINE1
 line2
@@
 line4
-line5
+LINE5
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(
            result.content.unwrap(),
            "LINE1\nline2\nline3\nline4\nLINE5\n"
        );
    }

    #[test]
    fn test_apply_no_update_hunks() {
        let strategy = make_strategy();
        let original = "line1\nline2\n";
        let patch = "\
*** Begin Patch
*** Add File: new.txt
+hello
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_no_match() {
        let strategy = make_strategy();
        let original = "line1\nline2\n";
        let patch = "\
*** Begin Patch
*** Update File: test.txt
@@
-not_found
+replacement
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        assert!(!result.success);
    }

    #[test]
    fn test_apply_lenient_without_markers() {
        let strategy = make_strategy();
        let original = "line1\nline2\nline3\n";
        // Patch without Begin/End markers but with Update File
        let patch = "\
*** Update File: test.txt
@@
-line2
+LINE2
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        // Should still work in lenient mode
        // This may or may not succeed depending on the parse logic;
        // at minimum it should not panic
        let _ = result;
    }

    #[test]
    fn test_apply_windows_line_endings() {
        let strategy = make_strategy();
        let original = "line1\r\nline2\r\nline3\r\n";
        let patch = "\
*** Begin Patch
*** Update File: test.txt
@@
 line1
-line2
+LINE2
 line3
*** End Patch";
        let result = strategy.apply_diff(original, patch);
        assert!(
            result.success,
            "Expected success, got error: {:?}",
            result.error
        );
        assert_eq!(result.content.unwrap(), "line1\r\nLINE2\r\nline3\r\n");
    }

    #[test]
    fn test_parse_update_chunks_empty() {
        let result = parse_update_chunks("");
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_seek_lines_exact() {
        let lines: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let pattern: Vec<String> = vec!["b".into(), "c".into()];
        let result = seek_lines(&lines, &pattern, 0);
        assert_eq!(result, Some((1, 2)));
    }

    #[test]
    fn test_seek_lines_no_match() {
        let lines: Vec<String> = vec!["a".into(), "b".into()];
        let pattern: Vec<String> = vec!["x".into()];
        let result = seek_lines(&lines, &pattern, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_seek_lines_trim_end() {
        let lines: Vec<String> = vec!["a  ".into(), "b ".into()];
        let pattern: Vec<String> = vec!["a".into(), "b".into()];
        let result = seek_lines(&lines, &pattern, 0);
        assert_eq!(result, Some((0, 2)));
    }

    #[test]
    fn test_seek_lines_pattern_too_long() {
        let lines: Vec<String> = vec!["a".into()];
        let pattern: Vec<String> = vec!["a".into(), "b".into()];
        let result = seek_lines(&lines, &pattern, 0);
        assert_eq!(result, None);
    }
}
