//! edit_file tool implementation.
//!
//! Performs literal string replacement in files. Corresponds to
//! `src/core/tools/EditFileTool.ts`.
//!
//! Supports:
//! - Modifying existing files (provide `old_string` and `new_string`)
//! - Creating new files (set `old_string` to empty string)
//! - Multiple replacements via `expected_replacements`
//! - Three-layer matching strategy: exact → whitespace-tolerant → token-based

use crate::helpers::*;
use crate::types::*;
use regex::Regex;
use roo_ignore::RooIgnoreController;
use roo_types::tool::EditFileParams;

/// Validate edit_file parameters.
pub fn validate_edit_file_params(params: &EditFileParams) -> Result<(), FsToolError> {
    if params.file_path.trim().is_empty() {
        return Err(FsToolError::Validation("file_path must not be empty".to_string()));
    }

    if params.file_path.contains("..") {
        return Err(FsToolError::InvalidPath(
            "file_path must not contain '..'".to_string(),
        ));
    }

    // new_string must be provided (but can be empty to delete text)
    // old_string can be empty only for new file creation
    Ok(())
}

/// Count non-overlapping occurrences of a substring.
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut pos = 0;
    while let Some(idx) = haystack[pos..].find(needle) {
        count += 1;
        pos += idx + needle.len();
    }
    count
}

/// Safely replace all occurrences of a literal string.
/// Handles the case where the replacement string contains `$` which
/// would be interpreted specially by regex-like replace functions.
fn safe_literal_replace(haystack: &str, old: &str, new: &str) -> String {
    if old.is_empty() || !haystack.contains(old) {
        return haystack.to_string();
    }
    haystack.replace(old, new)
}

/// Detect the dominant line ending in content.
fn detect_line_ending(content: &str) -> &str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Normalize line endings to LF.
fn normalize_to_lf(content: &str) -> String {
    content.replace("\r\n", "\n")
}

/// Restore line endings from LF to the detected format.
fn restore_line_ending(content_lf: &str, eol: &str) -> String {
    if eol == "\n" {
        content_lf.to_string()
    } else {
        content_lf.replace('\n', "\r\n")
    }
}

// ---------------------------------------------------------------------------
// Three-layer matching strategy
// ---------------------------------------------------------------------------

/// Try exact literal match and replacement.
///
/// Returns `Some((modified_content, actual_count))` on success, `None` on failure.
fn try_exact_replace(content: &str, old: &str, new: &str, expected: usize) -> Option<(String, usize)> {
    let actual_count = count_occurrences(content, old);
    if actual_count == 0 || actual_count != expected {
        return None;
    }

    let modified = if expected == 1 {
        if let Some(pos) = content.find(old) {
            let mut result = content.to_string();
            result.replace_range(pos..pos + old.len(), new);
            result
        } else {
            return None;
        }
    } else {
        safe_literal_replace(content, old, new)
    };

    Some((modified, actual_count))
}

/// Build a whitespace-tolerant regex from a pattern.
///
/// Matches TS `buildWhitespaceTolerantRegex` exactly:
/// 1. Splits the pattern into alternating runs of whitespace and non-whitespace.
/// 2. For whitespace runs that include `\n`, uses `\s+` (matches any whitespace
///    including newlines, to tolerate wrapping changes across lines).
/// 3. For whitespace runs without `\n`, uses `[\t ]+` (horizontal only, to avoid
///    accidentally consuming line breaks that precede indentation).
/// 4. For non-whitespace parts, escapes them for regex.
fn build_whitespace_tolerant_regex(pattern: &str) -> Result<Regex, regex::Error> {
    // Split into alternating runs of whitespace and non-whitespace,
    // matching TS: `oldLF.match(/(\s+|\S+)/g) ?? []`
    let mut parts: Vec<PatternRun> = Vec::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let is_ws = chars[i].is_whitespace();
        let start = i;
        while i < chars.len() && chars[i].is_whitespace() == is_ws {
            i += 1;
        }
        let run: String = chars[start..i].iter().collect();
        parts.push(PatternRun {
            is_whitespace: is_ws,
            content: run,
        });
    }

    if parts.is_empty() {
        return Err(regex::Error::Syntax(
            "Pattern is empty after splitting by whitespace".to_string(),
        ));
    }

    // Check that there's at least one non-whitespace part
    let has_non_ws = parts.iter().any(|p| !p.is_whitespace);
    if !has_non_ws {
        return Err(regex::Error::Syntax(
            "Pattern has no non-whitespace content".to_string(),
        ));
    }

    let regex_str: String = parts
        .iter()
        .map(|part| {
            if part.is_whitespace {
                // Match TS: if the whitespace run includes a newline, allow matching
                // any whitespace (including newlines); otherwise limit to horizontal
                // whitespace so we don't accidentally consume line breaks.
                if part.content.contains('\n') {
                    r"\s+".to_string()
                } else {
                    r"[\t ]+".to_string()
                }
            } else {
                regex::escape(&part.content)
            }
        })
        .collect();

    Regex::new(&regex_str)
}

/// A run of characters that are either all whitespace or all non-whitespace.
struct PatternRun {
    is_whitespace: bool,
    content: String,
}

/// Try whitespace-tolerant match and replacement.
///
/// Returns `Some((modified_content, actual_count))` on success, `None` on failure.
fn try_whitespace_tolerant_replace(
    content: &str,
    old: &str,
    new: &str,
    expected: usize,
) -> Option<(String, usize)> {
    let re = match build_whitespace_tolerant_regex(old) {
        Ok(r) => r,
        Err(_) => return None,
    };

    let matches: Vec<_> = re.find_iter(content).collect();
    let actual_count = matches.len();

    if actual_count == 0 || actual_count != expected {
        return None;
    }

    // Replace from end to start to preserve positions
    let mut result = content.to_string();
    for m in matches.into_iter().rev() {
        result.replace_range(m.start()..m.end(), new);
    }

    Some((result, actual_count))
}

/// Build a token-based regex from a pattern.
///
/// Extracts identifiers (alphanumeric + _) and joins them with `.*?`
/// to allow arbitrary content between tokens.
fn build_token_regex(pattern: &str) -> Result<Regex, regex::Error> {
    let tokens: Vec<&str> = pattern
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .collect();

    if tokens.is_empty() {
        return Err(regex::Error::Syntax(
            "No tokens found in pattern".to_string(),
        ));
    }

    let regex_parts: Vec<String> = tokens.iter().map(|t| regex::escape(t)).collect();
    let regex_str = regex_parts.join(r".*?");

    // Use (?s) for dotall mode so . matches newlines
    Regex::new(&format!("(?s){}", regex_str))
}

/// Try token-based match and replacement.
///
/// Returns `Some((modified_content, actual_count))` on success, `None` on failure.
fn try_token_replace(
    content: &str,
    old: &str,
    new: &str,
    expected: usize,
) -> Option<(String, usize)> {
    let re = match build_token_regex(old) {
        Ok(r) => r,
        Err(_) => return None,
    };

    let matches: Vec<_> = re.find_iter(content).collect();
    let actual_count = matches.len();

    if actual_count == 0 || actual_count != expected {
        return None;
    }

    // Replace from end to start to preserve positions
    let mut result = content.to_string();
    for m in matches.into_iter().rev() {
        result.replace_range(m.start()..m.end(), new);
    }

    Some((result, actual_count))
}

/// Build a detailed error message when all matching strategies fail.
fn build_detailed_error(
    file_path: &str,
    old_string: &str,
    new_string: &str,
    content: &str,
) -> FsToolError {
    // Provide context about what was found vs what was expected
    let old_preview = if old_string.len() > 200 {
        format!("{}... (truncated, {} chars total)", &old_string[..200], old_string.len())
    } else {
        old_string.to_string()
    };

    let new_preview = if new_string.len() > 200 {
        format!("{}... (truncated, {} chars total)", &new_string[..200], new_string.len())
    } else {
        new_string.to_string()
    };

    // Try to find partial matches for diagnostic
    let mut diagnostics = Vec::new();

    // Check if old_string first line exists somewhere
    if let Some(first_line) = old_string.lines().next() {
        if content.contains(first_line) {
            diagnostics.push(format!(
                "  - First line of old_string found in file: \"{}\"",
                if first_line.len() > 80 {
                    format!("{}...", &first_line[..80])
                } else {
                    first_line.to_string()
                }
            ));
        }
    }

    // Check whitespace-tolerant regex diagnostics
    if let Ok(re) = build_whitespace_tolerant_regex(old_string) {
        let ws_count = re.find_iter(content).count();
        if ws_count > 0 {
            diagnostics.push(format!(
                "  - Whitespace-tolerant matching found {} occurrence(s) (expected 1)",
                ws_count
            ));
        }
    }

    // Check token-based regex diagnostics
    if let Ok(re) = build_token_regex(old_string) {
        let tok_count = re.find_iter(content).count();
        if tok_count > 0 {
            diagnostics.push(format!(
                "  - Token-based matching found {} occurrence(s) (expected 1)",
                tok_count
            ));
        }
    }

    let diag_section = if diagnostics.is_empty() {
        String::new()
    } else {
        format!("\nDiagnostics:\n{}\n", diagnostics.join("\n"))
    };

    FsToolError::Validation(format!(
        "No exact match found for old_string in file: {file_path}\n\
         \n\
         <error_details>\n\
         <old_string>{old_preview}</old_string>\n\
         <new_string>{new_preview}</new_string>\n\
         <error>No exact match found for old_string in file</error>\n\
         <suggestions>\n\
         - Check for whitespace differences (tabs vs spaces, extra newlines)\n\
         - Use read_file to see the actual file content\n\
         - Ensure the old_string exactly matches the file content\n\
         - Try copying the exact text from the file using read_file\n\
         </suggestions>\n\
         {diag_section}\
         </error_details>"
    ))
}

/// Process an edit_file operation.
///
/// Implements the logic from `EditFileTool.ts`:
/// 1. If `old_string` is empty → create new file
/// 2. If file doesn't exist and `old_string` is not empty → error
/// 3. If file exists and `old_string` is empty → error (file already exists)
/// 4. Otherwise → perform literal string replacement using three-layer matching:
///    a. Exact literal match
///    b. Whitespace-tolerant regex match
///    c. Token-based regex match
pub fn process_edit_file(
    params: &EditFileParams,
    cwd: &std::path::Path,
    ignore_controller: Option<&RooIgnoreController>,
) -> Result<EditFileResult, FsToolError> {
    validate_edit_file_params(params)?;

    // Check .rooignore before any file I/O
    check_roo_ignore(&params.file_path, ignore_controller)?;

    // Resolve file path (can be absolute or relative)
    let file_path = if std::path::Path::new(&params.file_path).is_absolute() {
        std::path::PathBuf::from(&params.file_path)
    } else {
        cwd.join(&params.file_path)
    };

    let file_exists = file_path.exists();
    let old_string = &params.old_string;
    let new_string = &params.new_string;
    let expected_replacements = params.expected_replacements.unwrap_or(1).max(1) as usize;

    // Case: Creating a new file (old_string is empty)
    if old_string.is_empty() {
        if file_exists {
            return Err(FsToolError::Validation(format!(
                "File already exists: {}. To modify an existing file, provide a non-empty old_string.",
                params.file_path
            )));
        }

        // Create parent directories
        create_directories_for_file(&file_path)?;

        // Write the new file
        std::fs::write(&file_path, new_string)?;

        return Ok(EditFileResult {
            path: params.file_path.clone(),
            success: true,
            message: Some(format!(
                "Created new file: {} ({} lines)",
                params.file_path,
                new_string.lines().count()
            )),
        });
    }

    // Case: Modifying an existing file
    if !file_exists {
        return Err(FsToolError::FileNotFound(format!(
            "File does not exist: {}. If you intended to create a new file, set old_string to an empty string.",
            params.file_path
        )));
    }

    // Read the current content
    let current_content = std::fs::read_to_string(&file_path)?;

    // Detect and normalize line endings
    let eol = detect_line_ending(&current_content);
    let current_lf = normalize_to_lf(&current_content);
    let old_lf = normalize_to_lf(old_string);
    let new_lf = normalize_to_lf(new_string);

    // Validate that old_string and new_string are different
    if old_lf == new_lf {
        return Err(FsToolError::Validation(
            "old_string and new_string are identical after normalizing line endings. No changes to apply.".to_string(),
        ));
    }

    // --- Three-layer matching strategy ---

    // Layer 1: Exact literal match
    if let Some((modified_lf, actual_count)) =
        try_exact_replace(&current_lf, &old_lf, &new_lf, expected_replacements)
    {
        let modified = restore_line_ending(&modified_lf, eol);
        std::fs::write(&file_path, &modified)?;

        return Ok(EditFileResult {
            path: params.file_path.clone(),
            success: true,
            message: Some(format!(
                "Successfully applied {} replacement(s) in {} (exact match)",
                actual_count, params.file_path
            )),
        });
    }

    // Layer 2: Whitespace-tolerant match
    if let Some((modified_lf, actual_count)) =
        try_whitespace_tolerant_replace(&current_lf, &old_lf, &new_lf, expected_replacements)
    {
        let modified = restore_line_ending(&modified_lf, eol);
        std::fs::write(&file_path, &modified)?;

        return Ok(EditFileResult {
            path: params.file_path.clone(),
            success: true,
            message: Some(format!(
                "Successfully applied {} replacement(s) in {} (whitespace-tolerant match)",
                actual_count, params.file_path
            )),
        });
    }

    // Layer 3: Token-based match
    if let Some((modified_lf, actual_count)) =
        try_token_replace(&current_lf, &old_lf, &new_lf, expected_replacements)
    {
        let modified = restore_line_ending(&modified_lf, eol);
        std::fs::write(&file_path, &modified)?;

        return Ok(EditFileResult {
            path: params.file_path.clone(),
            success: true,
            message: Some(format!(
                "Successfully applied {} replacement(s) in {} (token-based match)",
                actual_count, params.file_path
            )),
        });
    }

    // All strategies failed — produce detailed error
    Err(build_detailed_error(
        &params.file_path,
        &old_lf,
        &new_lf,
        &current_lf,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_path() {
        let params = EditFileParams {
            file_path: "".to_string(),
            old_string: "old".to_string(),
            new_string: "new".to_string(),
            expected_replacements: None,
        };
        assert!(validate_edit_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_path_traversal() {
        let params = EditFileParams {
            file_path: "../secret".to_string(),
            old_string: "old".to_string(),
            new_string: "new".to_string(),
            expected_replacements: None,
        };
        assert!(validate_edit_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid() {
        let params = EditFileParams {
            file_path: "test.txt".to_string(),
            old_string: "old".to_string(),
            new_string: "new".to_string(),
            expected_replacements: None,
        };
        assert!(validate_edit_file_params(&params).is_ok());
    }

    #[test]
    fn test_process_edit_file_not_found() {
        let params = EditFileParams {
            file_path: "nonexistent.txt".to_string(),
            old_string: "foo".to_string(),
            new_string: "bar".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_edit_file_create_new() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "".to_string(),
            new_string: "hello world\n".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);
        assert!(result.message.unwrap().contains("Created new file"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello world\n");
    }

    #[test]
    fn test_process_edit_file_create_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "existing content").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "".to_string(),
            new_string: "new content".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_edit_file_replace_success() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world\nfoo bar\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "hello world".to_string(),
            new_string: "HELLO WORLD".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);
        assert!(result.message.unwrap().contains("exact match"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("HELLO WORLD"));
        assert!(!content.contains("hello world"));
    }

    #[test]
    fn test_process_edit_file_not_found_string() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "nonexistent string".to_string(),
            new_string: "replacement".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None);
        assert!(result.is_err());
        // Verify detailed error message
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("<error_details>"));
        assert!(err_msg.contains("<suggestions>"));
    }

    #[test]
    fn test_process_edit_file_wrong_count() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "foo foo foo\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "foo".to_string(),
            new_string: "bar".to_string(),
            expected_replacements: Some(1),
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_edit_file_multiple_replacements() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "foo and foo and foo\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "foo".to_string(),
            new_string: "bar".to_string(),
            expected_replacements: Some(3),
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "bar and bar and bar\n");
    }

    #[test]
    fn test_process_edit_file_identical_strings() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "hello".to_string(),
            new_string: "hello".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_count_occurrences() {
        assert_eq!(count_occurrences("foo foo foo", "foo"), 3);
        assert_eq!(count_occurrences("hello world", "foo"), 0);
        assert_eq!(count_occurrences("aaa", "aa"), 1); // non-overlapping
        assert_eq!(count_occurrences("any text", ""), 0);
    }

    #[test]
    fn test_process_edit_file_crlf_handling() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("crlf.txt");
        std::fs::write(&file_path, "hello\r\nworld\r\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "hello\nworld".to_string(),
            new_string: "HELLO\nWORLD".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("HELLO\r\nWORLD"));
    }

    #[test]
    fn test_process_edit_file_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("a").join("b").join("deep.txt");

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "".to_string(),
            new_string: "deep content".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);
        assert!(file_path.exists());
    }

    // -----------------------------------------------------------------------
    // Three-layer matching strategy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_exact_match_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "hello world".to_string(),
            new_string: "HELLO WORLD".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);
        assert!(result.message.unwrap().contains("exact match"));
    }

    #[test]
    fn test_whitespace_tolerant_match_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        // File has multiple spaces between words
        std::fs::write(&file_path, "hello    world\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            // old_string has single space
            old_string: "hello world".to_string(),
            new_string: "HELLO WORLD".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);
        assert!(result.message.unwrap().contains("whitespace-tolerant match"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "HELLO WORLD\n");
    }

    #[test]
    fn test_whitespace_tolerant_match_tabs_vs_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        // File has tab between words
        std::fs::write(&file_path, "hello\tworld\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            // old_string has spaces
            old_string: "hello world".to_string(),
            new_string: "HELLO WORLD".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "HELLO WORLD\n");
    }

    #[test]
    fn test_token_based_match_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        // File has non-whitespace separators between identifiers
        // Whitespace-tolerant won't match because ':' is not whitespace
        std::fs::write(&file_path, "function:myFunc\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "function myFunc".to_string(),
            new_string: "def myFunc".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None).unwrap();
        assert!(result.success);
        assert!(result.message.unwrap().contains("token-based match"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "def myFunc\n");
    }

    #[test]
    fn test_all_strategies_fail_detailed_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "completely different content\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "fn foo() { bar }".to_string(),
            new_string: "fn baz() { qux }".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("<error_details>"));
        assert!(err_msg.contains("<old_string>"));
        assert!(err_msg.contains("<new_string>"));
        assert!(err_msg.contains("<suggestions>"));
    }

    #[test]
    fn test_build_whitespace_tolerant_regex() {
        let re = build_whitespace_tolerant_regex("hello world").unwrap();
        assert!(re.is_match("hello    world"));
        assert!(re.is_match("hello\tworld"));
        assert!(re.is_match("hello world"));
    }

    #[test]
    fn test_build_token_regex() {
        let re = build_token_regex("fn foo() { bar }").unwrap();
        assert!(re.is_match("fn  foo(  )  {  bar  }"));
        assert!(re.is_match("fn foo() { bar }"));
        assert!(re.is_match("fn...foo...bar"));
    }

    #[test]
    fn test_build_token_regex_no_tokens() {
        let result = build_token_regex("!@#$%^&*()");
        assert!(result.is_err());
    }

    #[test]
    fn test_try_exact_replace_success() {
        let content = "hello world\nfoo bar\n";
        let result = try_exact_replace(content, "hello world", "HELLO WORLD", 1);
        assert!(result.is_some());
        let (modified, count) = result.unwrap();
        assert_eq!(count, 1);
        assert_eq!(modified, "HELLO WORLD\nfoo bar\n");
    }

    #[test]
    fn test_try_exact_replace_no_match() {
        let content = "hello world\n";
        let result = try_exact_replace(content, "nonexistent", "replacement", 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_try_exact_replace_wrong_count() {
        let content = "foo foo foo\n";
        let result = try_exact_replace(content, "foo", "bar", 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_try_whitespace_tolerant_replace_success() {
        let content = "hello    world\n";
        let result = try_whitespace_tolerant_replace(content, "hello world", "HELLO WORLD", 1);
        assert!(result.is_some());
        let (modified, count) = result.unwrap();
        assert_eq!(count, 1);
        assert_eq!(modified, "HELLO WORLD\n");
    }

    #[test]
    fn test_try_whitespace_tolerant_replace_no_match() {
        let content = "completely different\n";
        let result = try_whitespace_tolerant_replace(content, "hello world", "HELLO WORLD", 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_try_token_replace_success() {
        // Token-based match: identifiers match despite non-whitespace separators
        let content = "function:myFunc\n";
        let result = try_token_replace(content, "function myFunc", "def myFunc", 1);
        assert!(result.is_some());
        let (modified, count) = result.unwrap();
        assert_eq!(count, 1);
        assert_eq!(modified, "def myFunc\n");
    }

    #[test]
    fn test_try_token_replace_no_match() {
        let content = "completely different\n";
        let result = try_token_replace(content, "fn foo() { bar }", "fn baz() { qux }", 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_detailed_error_contains_suggestions() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "some content here\n").unwrap();

        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "fn foo() { bar }".to_string(),
            new_string: "fn baz() { qux }".to_string(),
            expected_replacements: None,
        };
        let result = process_edit_file(&params, std::path::Path::new("."), None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("read_file"));
        assert!(err.contains("whitespace"));
    }
}
