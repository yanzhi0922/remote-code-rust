//! apply_patch tool implementation.
//!
//! Handles parsing and applying patches in the Codex apply_patch format.
//! Corresponds to `src/core/tools/apply-patch/` in the TS source:
//! - `parser.ts` → patch parsing
//! - `seek-sequence.ts` → fuzzy sequence matching
//! - `apply.ts` → patch application
//! - `ApplyPatchTool.ts` → tool handler logic

use crate::types::FsToolError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";
const CHANGE_CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

// ---------------------------------------------------------------------------
// ParseError
// ---------------------------------------------------------------------------

/// Error during patch parsing.
///
/// Corresponds to `ParseError` in `parser.ts`.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line_number: Option<usize>,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line_number: None,
        }
    }

    pub fn with_line(message: impl Into<String>, line_number: usize) -> Self {
        Self {
            message: message.into(),
            line_number: Some(line_number),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line_number {
            Some(ln) => write!(f, "Line {}: {}", ln, self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// ApplyPatchError
// ---------------------------------------------------------------------------

/// Error during patch application.
///
/// Corresponds to `ApplyPatchError` in `apply.ts`.
#[derive(Debug, Clone)]
pub struct ApplyPatchError {
    pub message: String,
}

impl ApplyPatchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApplyPatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApplyPatchError {}

// ---------------------------------------------------------------------------
// UpdateFileChunk
// ---------------------------------------------------------------------------

/// A chunk within an UpdateFile hunk.
///
/// Corresponds to `UpdateFileChunk` in `parser.ts`.
#[derive(Debug, Clone)]
pub struct UpdateFileChunk {
    /// Optional context line (e.g., class or function name) to narrow search.
    pub change_context: Option<String>,
    /// Lines to find and replace (context + removed lines).
    pub old_lines: Vec<String>,
    /// Lines to replace with (context + added lines).
    pub new_lines: Vec<String>,
    /// If true, old_lines must match at end of file.
    pub is_end_of_file: bool,
}

// ---------------------------------------------------------------------------
// Hunk
// ---------------------------------------------------------------------------

/// Represents a file operation in a patch.
///
/// Corresponds to `Hunk` in `parser.ts`.
#[derive(Debug, Clone)]
pub enum Hunk {
    AddFile {
        path: String,
        contents: String,
    },
    DeleteFile {
        path: String,
    },
    UpdateFile {
        path: String,
        move_path: Option<String>,
        chunks: Vec<UpdateFileChunk>,
    },
}

// ---------------------------------------------------------------------------
// ApplyPatchArgs
// ---------------------------------------------------------------------------

/// Result of parsing a patch.
///
/// Corresponds to `ApplyPatchArgs` in `parser.ts`.
#[derive(Debug, Clone)]
pub struct ApplyPatchArgs {
    pub hunks: Vec<Hunk>,
    pub patch: String,
}

// ---------------------------------------------------------------------------
// FileChangeType / ApplyPatchFileChange
// ---------------------------------------------------------------------------

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeType {
    Add,
    Delete,
    Update,
}

/// Result of applying a patch to a file.
///
/// Corresponds to `ApplyPatchFileChange` in `apply.ts`.
#[derive(Debug, Clone)]
pub struct ApplyPatchFileChange {
    pub change_type: FileChangeType,
    /// Original path of the file.
    pub path: String,
    /// New path if the file was moved/renamed.
    pub move_path: Option<String>,
    /// Original content (for delete/update).
    pub original_content: Option<String>,
    /// New content (for add/update).
    pub new_content: Option<String>,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate apply_patch parameters.
///
/// Matches TS `ApplyPatchTool.execute` validation: patch must be non-empty.
pub fn validate_apply_patch_params(patch: &str) -> Result<(), FsToolError> {
    if patch.trim().is_empty() {
        return Err(FsToolError::Validation("patch must not be empty".to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Parser (from parser.ts)
// ---------------------------------------------------------------------------

/// Check that lines start and end with correct patch markers.
fn check_patch_boundaries(lines: &[&str]) -> Result<(), ParseError> {
    if lines.is_empty() {
        return Err(ParseError::new("Empty patch"));
    }

    let first_line = lines[0].trim();
    let last_line = lines[lines.len() - 1].trim();

    if first_line != BEGIN_PATCH_MARKER {
        return Err(ParseError::new(
            "The first line of the patch must be '*** Begin Patch'",
        ));
    }

    if last_line != END_PATCH_MARKER {
        return Err(ParseError::new(
            "The last line of the patch must be '*** End Patch'",
        ));
    }

    Ok(())
}

/// Parse a single UpdateFileChunk from lines.
/// Returns the parsed chunk and number of lines consumed.
fn parse_update_file_chunk(
    lines: &[&str],
    line_number: usize,
    allow_missing_context: bool,
) -> Result<(UpdateFileChunk, usize), ParseError> {
    if lines.is_empty() {
        return Err(ParseError::with_line(
            "Update hunk does not contain any lines",
            line_number,
        ));
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
        return Err(ParseError::with_line(
            format!(
                "Expected update hunk to start with a @@ context marker, got: '{}'",
                lines[0]
            ),
            line_number,
        ));
    }

    if start_index >= lines.len() {
        return Err(ParseError::with_line(
            "Update hunk does not contain any lines",
            line_number + 1,
        ));
    }

    let mut chunk = UpdateFileChunk {
        change_context,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        is_end_of_file: false,
    };

    let mut parsed_lines = 0;
    for i in start_index..lines.len() {
        let line = lines[i];

        if line == EOF_MARKER {
            if parsed_lines == 0 {
                return Err(ParseError::with_line(
                    "Update hunk does not contain any lines",
                    line_number + 1,
                ));
            }
            chunk.is_end_of_file = true;
            parsed_lines += 1;
            break;
        }

        // Empty line is treated as context
        if line.is_empty() {
            chunk.old_lines.push(String::new());
            chunk.new_lines.push(String::new());
            parsed_lines += 1;
            continue;
        }

        let first_char = line.chars().next();

        match first_char {
            Some(' ') => {
                // Context line
                chunk.old_lines.push(line[1..].to_string());
                chunk.new_lines.push(line[1..].to_string());
                parsed_lines += 1;
            }
            Some('+') => {
                // Added line
                chunk.new_lines.push(line[1..].to_string());
                parsed_lines += 1;
            }
            Some('-') => {
                // Removed line
                chunk.old_lines.push(line[1..].to_string());
                parsed_lines += 1;
            }
            _ => {
                // If we haven't parsed any lines yet, it's an error
                if parsed_lines == 0 {
                    return Err(ParseError::with_line(
                        format!(
                            "Unexpected line found in update hunk: '{}'. Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)",
                            line
                        ),
                        line_number + 1,
                    ));
                }
                // Otherwise, assume this is the start of the next hunk
                return Ok((chunk, parsed_lines + start_index));
            }
        }
    }

    Ok((chunk, parsed_lines + start_index))
}

/// Parse a single hunk (file operation) from lines.
/// Returns the parsed hunk and number of lines consumed.
fn parse_one_hunk(lines: &[&str], line_number: usize) -> Result<(Hunk, usize), ParseError> {
    let first_line = lines[0].trim();

    // Add File
    if first_line.starts_with(ADD_FILE_MARKER) {
        let path = first_line[ADD_FILE_MARKER.len()..].to_string();
        let mut contents = String::new();
        let mut parsed_lines = 1;

        for i in 1..lines.len() {
            if lines[i].starts_with('+') {
                contents.push_str(&lines[i][1..]);
                contents.push('\n');
                parsed_lines += 1;
            } else {
                break;
            }
        }

        return Ok((Hunk::AddFile { path, contents }, parsed_lines));
    }

    // Delete File
    if first_line.starts_with(DELETE_FILE_MARKER) {
        let path = first_line[DELETE_FILE_MARKER.len()..].to_string();
        return Ok((Hunk::DeleteFile { path }, 1));
    }

    // Update File
    if first_line.starts_with(UPDATE_FILE_MARKER) {
        let path = first_line[UPDATE_FILE_MARKER.len()..].to_string();
        let mut parsed_lines = 1;

        // Check for optional Move to line
        let mut move_path: Option<String> = None;
        if lines.len() > 1 && lines[1].starts_with(MOVE_TO_MARKER) {
            move_path = Some(lines[1][MOVE_TO_MARKER.len()..].to_string());
            parsed_lines += 1;
        }

        let mut chunks: Vec<UpdateFileChunk> = Vec::new();
        let mut pos = parsed_lines;

        while pos < lines.len() {
            // Skip blank lines between chunks
            if lines[pos].trim().is_empty() {
                parsed_lines += 1;
                pos += 1;
                continue;
            }

            // Stop if we hit another file operation marker
            if lines[pos].starts_with("***") {
                break;
            }

            let remaining = &lines[pos..];
            let (chunk, lines_consumed) = parse_update_file_chunk(
                remaining,
                line_number + parsed_lines,
                chunks.is_empty(), // Allow missing context for first chunk
            )?;
            chunks.push(chunk);
            parsed_lines += lines_consumed;
            pos += lines_consumed;
        }

        if chunks.is_empty() {
            return Err(ParseError::with_line(
                format!("Update file hunk for path '{}' is empty", path),
                line_number,
            ));
        }

        return Ok((
            Hunk::UpdateFile {
                path,
                move_path,
                chunks,
            },
            parsed_lines,
        ));
    }

    Err(ParseError::with_line(
        format!(
            "'{}' is not a valid hunk header. Valid hunk headers: '*** Add File: {{path}}', '*** Delete File: {{path}}', '*** Update File: {{path}}'",
            first_line
        ),
        line_number,
    ))
}

/// Parse a patch string into structured hunks.
///
/// Corresponds to `parsePatch` in `parser.ts`.
pub fn parse_patch(patch: &str) -> Result<ApplyPatchArgs, ParseError> {
    let trimmed_patch = patch.trim();
    let all_lines: Vec<&str> = trimmed_patch.lines().collect();

    // Handle heredoc-wrapped patches (lenient mode)
    let effective_lines: Vec<&str> = if all_lines.len() >= 4 {
        let first_line = all_lines[0];
        let last_line = all_lines[all_lines.len() - 1];
        if (first_line == "<<EOF"
            || first_line == "<<'EOF'"
            || first_line == "<<\"EOF\"")
            && last_line.ends_with("EOF")
        {
            all_lines[1..all_lines.len() - 1].to_vec()
        } else {
            all_lines
        }
    } else {
        all_lines
    };

    check_patch_boundaries(&effective_lines)?;

    let mut hunks: Vec<Hunk> = Vec::new();
    let last_line_index = effective_lines.len() - 1;
    let remaining: &[&str] = &effective_lines[1..last_line_index]; // Skip Begin and End markers
    let mut line_number = 2; // Start at line 2 (after Begin Patch)
    let mut offset = 0;

    while offset < remaining.len() {
        let sub_remaining = &remaining[offset..];
        let (hunk, lines_consumed) = parse_one_hunk(sub_remaining, line_number)?;
        hunks.push(hunk);
        line_number += lines_consumed;
        offset += lines_consumed;
    }

    Ok(ApplyPatchArgs {
        hunks,
        patch: effective_lines.join("\n"),
    })
}

// ---------------------------------------------------------------------------
// seek-sequence (from seek-sequence.ts)
// ---------------------------------------------------------------------------

/// Normalize common Unicode punctuation to ASCII equivalents.
///
/// Corresponds to `normalizeUnicode` in `seek-sequence.ts`.
fn normalize_unicode(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| {
            // Various dash/hyphen code-points → ASCII '-'
            if "\u{2010}\u{2011}\u{2012}\u{2013}\u{2014}\u{2015}\u{2212}".contains(c) {
                '-'
            }
            // Fancy single quotes → '\''
            else if "\u{2018}\u{2019}\u{201A}\u{201B}".contains(c) {
                '\''
            }
            // Fancy double quotes → '"'
            else if "\u{201C}\u{201D}\u{201E}\u{201F}".contains(c) {
                '"'
            }
            // Non-breaking space and other odd spaces → normal space
            else if "\u{00A0}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{202F}\u{205F}\u{3000}"
                .contains(c)
            {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// Check if two arrays of lines match exactly.
fn exact_match(lines: &[&str], pattern: &[String], start_index: usize) -> bool {
    for i in 0..pattern.len() {
        match lines.get(start_index + i) {
            Some(line) if *line == pattern[i] => continue,
            _ => return false,
        }
    }
    true
}

/// Check if two arrays of lines match after trimming trailing whitespace.
fn trim_end_match(lines: &[&str], pattern: &[String], start_index: usize) -> bool {
    for i in 0..pattern.len() {
        let line = lines.get(start_index + i).map(|s| s.trim_end());
        let pat = pattern.get(i).map(|s| s.as_str().trim_end());
        if line != pat {
            return false;
        }
    }
    true
}

/// Check if two arrays of lines match after trimming both sides.
fn trim_match_fn(lines: &[&str], pattern: &[String], start_index: usize) -> bool {
    for i in 0..pattern.len() {
        let line = lines.get(start_index + i).map(|s| s.trim());
        let pat = pattern.get(i).map(|s| s.as_str().trim());
        if line != pat {
            return false;
        }
    }
    true
}

/// Check if two arrays of lines match after Unicode normalization.
fn normalized_match(lines: &[&str], pattern: &[String], start_index: usize) -> bool {
    for i in 0..pattern.len() {
        let line = lines
            .get(start_index + i)
            .map(|s| normalize_unicode(s));
        let pat = pattern.get(i).map(|s| normalize_unicode(s));
        if line != pat {
            return false;
        }
    }
    true
}

/// Attempt to find the sequence of pattern lines within lines beginning at or
/// after `start`.
///
/// Matches are attempted with decreasing strictness:
/// 1. Exact match
/// 2. Ignoring trailing whitespace
/// 3. Ignoring leading and trailing whitespace
/// 4. Unicode-normalized (handles typographic characters)
///
/// When `eof` is true, first try starting at the end-of-file.
///
/// Corresponds to `seekSequence` in `seek-sequence.ts`.
pub fn seek_sequence(
    lines: &[&str],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }

    // When the pattern is longer than available input, there's no possible match
    if pattern.len() > lines.len() {
        return None;
    }

    let search_start = if eof && lines.len() >= pattern.len() {
        lines.len() - pattern.len()
    } else {
        start
    };

    let max_start = lines.len() - pattern.len();

    // Prevent underflow
    if search_start > max_start {
        return None;
    }

    // Pass 1: Exact match
    for i in search_start..=max_start {
        if exact_match(lines, pattern, i) {
            return Some(i);
        }
    }

    // Pass 2: Trim-end match
    for i in search_start..=max_start {
        if trim_end_match(lines, pattern, i) {
            return Some(i);
        }
    }

    // Pass 3: Trim both sides match
    for i in search_start..=max_start {
        if trim_match_fn(lines, pattern, i) {
            return Some(i);
        }
    }

    // Pass 4: Unicode-normalized match
    for i in search_start..=max_start {
        if normalized_match(lines, pattern, i) {
            return Some(i);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Apply logic (from apply.ts)
// ---------------------------------------------------------------------------

/// Compute the replacements needed to transform original_lines into the new
/// lines. Each replacement is `(start_index, old_length, new_lines)`.
fn compute_replacements(
    original_lines: &[&str],
    file_path: &str,
    chunks: &[UpdateFileChunk],
) -> Result<Vec<(usize, usize, Vec<String>)>, ApplyPatchError> {
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut line_index = 0;

    for chunk in chunks {
        // If a chunk has a change_context, find it first
        if let Some(ref ctx) = chunk.change_context {
            let ctx_vec = vec![ctx.clone()];
            let idx = seek_sequence(original_lines, &ctx_vec, line_index, false);
            match idx {
                Some(i) => line_index = i + 1,
                None => {
                    return Err(ApplyPatchError::new(format!(
                        "Failed to find context '{}' in {}",
                        ctx, file_path
                    )));
                }
            }
        }

        if chunk.old_lines.is_empty() {
            // Pure addition (no old lines). Add at the end or before final empty line.
            let insertion_idx =
                if !original_lines.is_empty() && original_lines[original_lines.len() - 1].is_empty()
                {
                    original_lines.len() - 1
                } else {
                    original_lines.len()
                };
            replacements.push((insertion_idx, 0, chunk.new_lines.clone()));
            continue;
        }

        // Try to find the old_lines in the file
        let mut pattern = chunk.old_lines.clone();
        let mut new_slice = chunk.new_lines.clone();
        let mut found = seek_sequence(original_lines, &pattern, line_index, chunk.is_end_of_file);

        // If not found and pattern ends with empty string (trailing newline),
        // retry without it
        if found.is_none()
            && !pattern.is_empty()
            && pattern.last().map(|s| s.is_empty()) == Some(true)
        {
            pattern = pattern[..pattern.len() - 1].to_vec();
            if !new_slice.is_empty() && new_slice.last().map(|s| s.is_empty()) == Some(true) {
                new_slice = new_slice[..new_slice.len() - 1].to_vec();
            }
            found = seek_sequence(original_lines, &pattern, line_index, chunk.is_end_of_file);
        }

        match found {
            Some(idx) => {
                replacements.push((idx, pattern.len(), new_slice));
                line_index = idx + pattern.len();
            }
            None => {
                let joined = chunk.old_lines.join("\n");
                let display = if joined.chars().count() > 200 {
                    let truncated: String = joined.chars().take(200).collect();
                    format!("{}...", truncated)
                } else {
                    joined
                };
                return Err(ApplyPatchError::new(format!(
                    "Failed to find expected lines in {}:\n{}",
                    file_path, display
                )));
            }
        }
    }

    // Sort replacements by start index
    replacements.sort_by_key(|r| r.0);

    Ok(replacements)
}

/// Apply replacements to the original lines, returning the modified content.
/// Replacements must be applied in reverse order to preserve indices.
fn apply_replacements(
    lines: &[&str],
    replacements: &[(usize, usize, Vec<String>)],
) -> Vec<String> {
    let mut result: Vec<String> = lines.iter().map(|s| s.to_string()).collect();

    // Apply in reverse order so earlier replacements don't shift later indices
    for i in (0..replacements.len()).rev() {
        let (start_idx, old_len, ref new_segment) = replacements[i];
        let new_owned: Vec<String> = new_segment.clone();
        result.splice(start_idx..start_idx + old_len, new_owned);
    }

    result
}

/// Apply chunks to file content, returning the new content.
///
/// Corresponds to `applyChunksToContent` in `apply.ts`.
pub fn apply_chunks_to_content(
    original_content: &str,
    file_path: &str,
    chunks: &[UpdateFileChunk],
) -> Result<String, ApplyPatchError> {
    // Split content into lines
    let all_lines: Vec<&str> = original_content.split('\n').collect();
    let mut original_lines: Vec<&str> = all_lines;

    // Drop trailing empty element that results from final newline
    // so that line counts match standard diff behavior
    if !original_lines.is_empty() && original_lines.last().map(|s| s.is_empty()) == Some(true) {
        original_lines.pop();
    }

    let replacements = compute_replacements(&original_lines, file_path, chunks)?;
    let mut new_lines = apply_replacements(&original_lines, &replacements);

    // Ensure file ends with newline
    if new_lines.is_empty() || new_lines.last().map(|s| !s.is_empty()) == Some(true) {
        new_lines.push(String::new());
    }

    Ok(new_lines.join("\n"))
}

/// Process a single hunk and return the file change.
///
/// Corresponds to `processHunk` in `apply.ts`.
pub fn process_hunk<F>(hunk: &Hunk, read_file: F) -> Result<ApplyPatchFileChange, ApplyPatchError>
where
    F: Fn(&str) -> Result<String, std::io::Error>,
{
    match hunk {
        Hunk::AddFile { path, contents } => Ok(ApplyPatchFileChange {
            change_type: FileChangeType::Add,
            path: path.clone(),
            move_path: None,
            original_content: None,
            new_content: Some(contents.clone()),
        }),

        Hunk::DeleteFile { path } => {
            let content = read_file(path).map_err(|e| {
                ApplyPatchError::new(format!("Failed to read file '{}': {}", path, e))
            })?;
            Ok(ApplyPatchFileChange {
                change_type: FileChangeType::Delete,
                path: path.clone(),
                move_path: None,
                original_content: Some(content),
                new_content: None,
            })
        }

        Hunk::UpdateFile {
            path,
            move_path,
            chunks,
        } => {
            let original_content = read_file(path).map_err(|e| {
                ApplyPatchError::new(format!("Failed to read file '{}': {}", path, e))
            })?;
            let new_content = apply_chunks_to_content(&original_content, path, chunks)?;
            Ok(ApplyPatchFileChange {
                change_type: FileChangeType::Update,
                path: path.clone(),
                move_path: move_path.clone(),
                original_content: Some(original_content),
                new_content: Some(new_content),
            })
        }
    }
}

/// Process all hunks in a patch.
///
/// Corresponds to `processAllHunks` in `apply.ts`.
pub fn process_all_hunks<F>(
    hunks: &[Hunk],
    read_file: F,
) -> Result<Vec<ApplyPatchFileChange>, ApplyPatchError>
where
    F: Fn(&str) -> Result<String, std::io::Error>,
{
    let mut changes = Vec::new();
    for hunk in hunks {
        let change = process_hunk(hunk, |p| read_file(p))?;
        changes.push(change);
    }
    Ok(changes)
}

/// Extract the first file path from a patch string.
/// Looks for `*** Add File: `, `*** Delete File: `, or `*** Update File: ` markers.
///
/// Corresponds to `extractFirstPathFromPatch` in `ApplyPatchTool.ts`.
pub fn extract_first_path_from_patch(patch: &str) -> Option<String> {
    if patch.is_empty() {
        return None;
    }

    let lines: Vec<&str> = patch.lines().collect();
    let has_trailing_newline = patch.ends_with('\n');
    let complete_lines: &[&str] = if has_trailing_newline {
        &lines
    } else {
        // Skip last line if it's incomplete
        if lines.is_empty() {
            return None;
        }
        &lines[..lines.len() - 1]
    };

    let markers = [ADD_FILE_MARKER, DELETE_FILE_MARKER, UPDATE_FILE_MARKER];

    for raw_line in complete_lines {
        let line = raw_line.trim();

        for marker in &markers {
            if !line.starts_with(marker) {
                continue;
            }

            let candidate_path = line[marker.len()..].trim();
            if !candidate_path.is_empty() {
                return Some(candidate_path.to_string());
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_patch tests ----

    #[test]
    fn test_parse_simple_add_file() {
        let patch = "*** Begin Patch\n*** Add File: hello.txt\n+Hello world\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        assert_eq!(result.hunks.len(), 1);
        match &result.hunks[0] {
            Hunk::AddFile { path, contents } => {
                assert_eq!(path, "hello.txt");
                assert_eq!(contents, "Hello world\n");
            }
            _ => panic!("Expected AddFile hunk"),
        }
    }

    #[test]
    fn test_parse_delete_file() {
        let patch = "*** Begin Patch\n*** Delete File: obsolete.txt\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        assert_eq!(result.hunks.len(), 1);
        match &result.hunks[0] {
            Hunk::DeleteFile { path } => {
                assert_eq!(path, "obsolete.txt");
            }
            _ => panic!("Expected DeleteFile hunk"),
        }
    }

    #[test]
    fn test_parse_update_file() {
        let patch = "*** Begin Patch\n*** Update File: src/app.py\n@@ def greet():\n-print(\"Hi\")\n+print(\"Hello, world!\")\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        assert_eq!(result.hunks.len(), 1);
        match &result.hunks[0] {
            Hunk::UpdateFile {
                path,
                move_path,
                chunks,
            } => {
                assert_eq!(path, "src/app.py");
                assert!(move_path.is_none());
                assert_eq!(chunks.len(), 1);
                assert_eq!(
                    chunks[0].change_context,
                    Some("def greet():".to_string())
                );
                assert_eq!(chunks[0].old_lines, vec!["print(\"Hi\")"]);
                assert_eq!(chunks[0].new_lines, vec!["print(\"Hello, world!\")"]);
            }
            _ => panic!("Expected UpdateFile hunk"),
        }
    }

    #[test]
    fn test_parse_update_file_with_move() {
        let patch = "*** Begin Patch\n*** Update File: src/app.py\n*** Move to: src/main.py\n@@\n-old\n+new\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        assert_eq!(result.hunks.len(), 1);
        match &result.hunks[0] {
            Hunk::UpdateFile { move_path, .. } => {
                assert_eq!(move_path, &Some("src/main.py".to_string()));
            }
            _ => panic!("Expected UpdateFile hunk"),
        }
    }

    #[test]
    fn test_parse_invalid_patch_missing_begin() {
        let patch = "*** Add File: hello.txt\n+Hello\n*** End Patch";
        let result = parse_patch(patch);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Begin Patch"));
    }

    #[test]
    fn test_parse_invalid_patch_missing_end() {
        let patch = "*** Begin Patch\n*** Add File: hello.txt\n+Hello";
        let result = parse_patch(patch);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("End Patch"));
    }

    #[test]
    fn test_parse_empty_patch() {
        let result = parse_patch("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_heredoc_wrapped_patch() {
        let patch = "<<EOF\n*** Begin Patch\n*** Add File: test.txt\n+content\n*** End Patch\nEOF";
        let result = parse_patch(patch).unwrap();
        assert_eq!(result.hunks.len(), 1);
    }

    #[test]
    fn test_parse_multiple_hunks() {
        let patch = "*** Begin Patch\n*** Add File: a.txt\n+hello\n*** Delete File: b.txt\n*** Update File: c.txt\n@@\n-old\n+new\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        assert_eq!(result.hunks.len(), 3);
    }

    #[test]
    fn test_parse_update_with_context_lines() {
        let patch = "*** Begin Patch\n*** Update File: foo.rs\n@@\n line1\n line2\n-old\n+new\n line4\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        match &result.hunks[0] {
            Hunk::UpdateFile { chunks, .. } => {
                // Context lines (" " prefix) go into both old_lines and new_lines
                assert_eq!(chunks[0].old_lines, vec!["line1", "line2", "old", "line4"]);
                assert_eq!(chunks[0].new_lines, vec!["line1", "line2", "new", "line4"]);
            }
            _ => panic!("Expected UpdateFile hunk"),
        }
    }

    #[test]
    fn test_parse_update_with_eof_marker() {
        let patch = "*** Begin Patch\n*** Update File: foo.rs\n@@\n-old\n*** End of File\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        match &result.hunks[0] {
            Hunk::UpdateFile { chunks, .. } => {
                assert!(chunks[0].is_end_of_file);
            }
            _ => panic!("Expected UpdateFile hunk"),
        }
    }

    // ---- seek_sequence tests ----

    #[test]
    fn test_seek_sequence_exact_match() {
        let lines = vec!["a", "b", "c", "d", "e"];
        let pattern = vec!["c".to_string(), "d".to_string()];
        let result = seek_sequence(&lines, &pattern, 0, false);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn test_seek_sequence_no_match() {
        let lines = vec!["a", "b", "c"];
        let pattern = vec!["x".to_string(), "y".to_string()];
        let result = seek_sequence(&lines, &pattern, 0, false);
        assert_eq!(result, None);
    }

    #[test]
    fn test_seek_sequence_empty_pattern() {
        let lines = vec!["a", "b"];
        let pattern: Vec<String> = vec![];
        let result = seek_sequence(&lines, &pattern, 0, false);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn test_seek_sequence_trim_end_match() {
        let lines = vec!["a  ", "b ", "c"];
        let pattern = vec!["a".to_string(), "b".to_string()];
        let result = seek_sequence(&lines, &pattern, 0, false);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn test_seek_sequence_unicode_match() {
        // \u{2013} is EN DASH, should match ASCII '-'
        let lines = vec!["foo\u{2013}bar"];
        let pattern = vec!["foo-bar".to_string()];
        let result = seek_sequence(&lines, &pattern, 0, false);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn test_seek_sequence_eof_mode() {
        let lines = vec!["a", "b", "c", "a", "b"];
        let pattern = vec!["a".to_string(), "b".to_string()];
        let result = seek_sequence(&lines, &pattern, 0, true);
        // EOF mode should find the match at the end
        assert_eq!(result, Some(3));
    }

    #[test]
    fn test_seek_sequence_pattern_too_long() {
        let lines = vec!["a"];
        let pattern = vec!["a".to_string(), "b".to_string()];
        let result = seek_sequence(&lines, &pattern, 0, false);
        assert_eq!(result, None);
    }

    // ---- apply_chunks_to_content tests ----

    #[test]
    fn test_apply_simple_replacement() {
        let content = "line1\nline2\nline3\n";
        let chunks = vec![UpdateFileChunk {
            change_context: None,
            old_lines: vec!["line2".to_string()],
            new_lines: vec!["LINE2".to_string()],
            is_end_of_file: false,
        }];
        let result = apply_chunks_to_content(content, "test.txt", &chunks).unwrap();
        assert_eq!(result, "line1\nLINE2\nline3\n");
    }

    #[test]
    fn test_apply_addition() {
        let content = "line1\nline2\n";
        let chunks = vec![UpdateFileChunk {
            change_context: None,
            old_lines: vec![],
            new_lines: vec!["inserted".to_string()],
            is_end_of_file: false,
        }];
        let result = apply_chunks_to_content(content, "test.txt", &chunks).unwrap();
        assert!(result.contains("inserted"));
    }

    #[test]
    fn test_apply_with_context() {
        let content = "class Foo:\n    def bar(self):\n        pass\n";
        let chunks = vec![UpdateFileChunk {
            change_context: Some("def bar(self):".to_string()),
            old_lines: vec!["        pass".to_string()],
            new_lines: vec!["        return 42".to_string()],
            is_end_of_file: false,
        }];
        let result = apply_chunks_to_content(content, "test.py", &chunks).unwrap();
        assert!(result.contains("return 42"));
        assert!(!result.contains("pass"));
    }

    #[test]
    fn test_apply_no_match_error() {
        let content = "line1\nline2\n";
        let chunks = vec![UpdateFileChunk {
            change_context: None,
            old_lines: vec!["not_found".to_string()],
            new_lines: vec!["replacement".to_string()],
            is_end_of_file: false,
        }];
        let result = apply_chunks_to_content(content, "test.txt", &chunks);
        assert!(result.is_err());
    }

    // ---- extract_first_path_from_patch tests ----

    #[test]
    fn test_extract_first_path_add() {
        let patch = "*** Begin Patch\n*** Add File: hello.txt\n+Hello\n*** End Patch";
        let path = extract_first_path_from_patch(patch);
        assert_eq!(path, Some("hello.txt".to_string()));
    }

    #[test]
    fn test_extract_first_path_update() {
        let patch = "*** Begin Patch\n*** Update File: src/app.py\n@@\n-old\n+new\n*** End Patch";
        let path = extract_first_path_from_patch(patch);
        assert_eq!(path, Some("src/app.py".to_string()));
    }

    #[test]
    fn test_extract_first_path_delete() {
        let patch = "*** Begin Patch\n*** Delete File: old.txt\n*** End Patch";
        let path = extract_first_path_from_patch(patch);
        assert_eq!(path, Some("old.txt".to_string()));
    }

    #[test]
    fn test_extract_first_path_empty() {
        let path = extract_first_path_from_patch("");
        assert_eq!(path, None);
    }

    #[test]
    fn test_extract_first_path_no_markers() {
        let path = extract_first_path_from_patch("some random text\nno markers here");
        assert_eq!(path, None);
    }

    // ---- process_hunk tests ----

    #[test]
    fn test_process_hunk_add_file() {
        let hunk = Hunk::AddFile {
            path: "new.txt".to_string(),
            contents: "hello\n".to_string(),
        };
        let result = process_hunk(&hunk, |_| panic!("should not read file")).unwrap();
        assert_eq!(result.change_type, FileChangeType::Add);
        assert_eq!(result.path, "new.txt");
        assert_eq!(result.new_content, Some("hello\n".to_string()));
    }

    #[test]
    fn test_process_hunk_delete_file() {
        let hunk = Hunk::DeleteFile {
            path: "old.txt".to_string(),
        };
        let result = process_hunk(&hunk, |p| {
            assert_eq!(p, "old.txt");
            Ok("content\n".to_string())
        })
        .unwrap();
        assert_eq!(result.change_type, FileChangeType::Delete);
        assert_eq!(result.original_content, Some("content\n".to_string()));
    }

    #[test]
    fn test_process_hunk_update_file() {
        let hunk = Hunk::UpdateFile {
            path: "test.txt".to_string(),
            move_path: None,
            chunks: vec![UpdateFileChunk {
                change_context: None,
                old_lines: vec!["old".to_string()],
                new_lines: vec!["new".to_string()],
                is_end_of_file: false,
            }],
        };
        let result = process_hunk(&hunk, |p| {
            assert_eq!(p, "test.txt");
            Ok("old\n".to_string())
        })
        .unwrap();
        assert_eq!(result.change_type, FileChangeType::Update);
        assert_eq!(result.new_content, Some("new\n".to_string()));
    }

    // ---- validate_apply_patch_params tests ----

    #[test]
    fn test_validate_empty_patch() {
        let result = validate_apply_patch_params("");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_valid_patch() {
        let result = validate_apply_patch_params("*** Begin Patch\n*** End Patch");
        assert!(result.is_ok());
    }

    // ---- normalize_unicode tests ----

    #[test]
    fn test_normalize_unicode_dashes() {
        assert_eq!(normalize_unicode("\u{2013}"), "-"); // EN DASH
        assert_eq!(normalize_unicode("\u{2014}"), "-"); // EM DASH
    }

    #[test]
    fn test_normalize_unicode_quotes() {
        assert_eq!(normalize_unicode("\u{2018}"), "'"); // LEFT SINGLE QUOTE
        assert_eq!(normalize_unicode("\u{201C}"), "\""); // LEFT DOUBLE QUOTE
    }

    #[test]
    fn test_normalize_unicode_spaces() {
        // NBSP is trimmed by trim() in both TS and Rust, so it becomes ""
        assert_eq!(normalize_unicode("\u{00A0}"), "");
        // But NBSP within text is converted to regular space
        assert_eq!(normalize_unicode("hello\u{00A0}world"), "hello world");
    }
}
