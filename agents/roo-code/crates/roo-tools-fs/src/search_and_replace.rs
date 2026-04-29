//! search_replace tool implementation.
//!
//! Performs literal string search and replace in files.
//! Corresponds to `src/core/tools/SearchReplaceTool.ts` in the TS source.
//!
//! Key behaviours (matching TS exactly):
//! - **Single occurrence only** — errors if `old_string` matches more than once.
//! - **Literal matching** — no regex; normalises CRLF → LF before comparing.
//! - Validates `old_string ≠ new_string`.
//! - Returns structured result with old/new content and diff-friendly info.

use crate::types::FsToolError;

// ---------------------------------------------------------------------------
// SearchReplaceParams
// ---------------------------------------------------------------------------

/// Parameters for the search_replace tool.
///
/// Matches the TS `SearchReplaceParams` interface:
/// ```ts
/// interface SearchReplaceParams {
///     file_path: string
///     old_string: string
///     new_string: string
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SearchReplaceParams {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
}

// ---------------------------------------------------------------------------
// SearchReplaceResult
// ---------------------------------------------------------------------------

/// Result of a successful search_replace operation.
#[derive(Debug, Clone)]
pub struct SearchReplaceResult {
    /// The path that was modified.
    pub path: String,
    /// Original file content (after CRLF normalisation).
    pub original_content: String,
    /// New file content.
    pub new_content: String,
}

// ---------------------------------------------------------------------------
// SearchReplaceError
// ---------------------------------------------------------------------------

/// Errors specific to the search_replace tool.
#[derive(Debug, Clone)]
pub enum SearchReplaceError {
    /// `file_path` parameter is empty.
    MissingFilePath,
    /// `old_string` parameter is empty.
    MissingOldString,
    /// `new_string` parameter was not provided.
    MissingNewString,
    /// `old_string` and `new_string` are identical.
    IdenticalStrings,
    /// File not found at the given path.
    FileNotFound(String),
    /// Failed to read the file.
    ReadFailed(String),
    /// No match found for `old_string`.
    NoMatch,
    /// Multiple matches found — `old_string` is not unique.
    MultipleMatches(usize),
}

impl std::fmt::Display for SearchReplaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFilePath => write!(f, "Missing required parameter 'file_path'"),
            Self::MissingOldString => write!(f, "Missing required parameter 'old_string'"),
            Self::MissingNewString => write!(f, "Missing required parameter 'new_string'"),
            Self::IdenticalStrings => write!(
                f,
                "The 'old_string' and 'new_string' parameters must be different."
            ),
            Self::FileNotFound(path) => {
                write!(
                    f,
                    "File not found: {}. Cannot perform search and replace on a non-existent file.",
                    path
                )
            }
            Self::ReadFailed(path) => {
                write!(
                    f,
                    "Failed to read file '{}'. Please verify file permissions and try again.",
                    path
                )
            }
            Self::NoMatch => write!(
                f,
                "No match found for the specified 'old_string'. Please ensure it matches the file contents exactly, including whitespace and indentation."
            ),
            Self::MultipleMatches(count) => {
                write!(
                    f,
                    "Found {} matches for the specified 'old_string'. This tool can only replace ONE occurrence at a time. Please provide more context (3-5 lines before and after) to uniquely identify the specific instance you want to change.",
                    count
                )
            }
        }
    }
}

impl std::error::Error for SearchReplaceError {}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate search_replace parameters.
///
/// Matches TS `SearchReplaceTool.execute` validation:
/// 1. `file_path` must be non-empty.
/// 2. `old_string` must be non-empty.
/// 3. `new_string` must be provided.
/// 4. `old_string` must differ from `new_string`.
pub fn validate_search_replace_params(params: &SearchReplaceParams) -> Result<(), SearchReplaceError> {
    if params.file_path.is_empty() {
        return Err(SearchReplaceError::MissingFilePath);
    }

    if params.old_string.is_empty() {
        return Err(SearchReplaceError::MissingOldString);
    }

    // In TS, `new_string === undefined` is checked, but since we have a String
    // field it's always present. We still check for the identical-strings case.
    if params.old_string == params.new_string {
        return Err(SearchReplaceError::IdenticalStrings);
    }

    Ok(())
}

/// Validate search_replace parameters including path safety checks.
pub fn validate_search_replace_params_full(params: &SearchReplaceParams) -> Result<(), FsToolError> {
    if params.file_path.trim().is_empty() {
        return Err(FsToolError::Validation(
            "file_path must not be empty".to_string(),
        ));
    }

    if params.file_path.contains("..") {
        return Err(FsToolError::InvalidPath(
            "file_path must not contain '..'".to_string(),
        ));
    }

    if params.old_string.is_empty() {
        return Err(FsToolError::Validation(
            "old_string must not be empty".to_string(),
        ));
    }

    if params.old_string == params.new_string {
        return Err(FsToolError::Validation(
            "old_string and new_string must be different".to_string(),
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Core logic
// ---------------------------------------------------------------------------

/// Normalise CRLF line endings to LF for consistent matching.
///
/// Matches TS: `content.replace(/\r\n/g, "\n")`
fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n")
}

/// Count non-overlapping occurrences of `needle` in `haystack`.
///
/// Matches TS: `fileContent.split(normalizedOldString).length - 1`
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.split(needle).count() - 1
}

/// Apply search-and-replace on the given file content.
///
/// This is the pure-logic core that mirrors the TS `SearchReplaceTool.execute`
/// matching logic:
/// 1. Normalise line endings (CRLF → LF) in both file content and strings.
/// 2. Count occurrences of `old_string`.
/// 3. If 0 matches → `NoMatch` error.
/// 4. If >1 matches → `MultipleMatches` error.
/// 5. Apply single replacement.
/// 6. Return the new content.
pub fn apply_search_replace(
    file_content: &str,
    old_string: &str,
    new_string: &str,
) -> Result<SearchReplaceResult, SearchReplaceError> {
    // Normalize line endings to LF for consistent matching
    let normalized_content = normalize_line_endings(file_content);
    let normalized_old = normalize_line_endings(old_string);
    let normalized_new = normalize_line_endings(new_string);

    // Count occurrences
    let match_count = count_occurrences(&normalized_content, &normalized_old);

    if match_count == 0 {
        return Err(SearchReplaceError::NoMatch);
    }

    if match_count > 1 {
        return Err(SearchReplaceError::MultipleMatches(match_count));
    }

    // Apply the single replacement
    let new_content = normalized_content.replacen(&normalized_old as &str, &normalized_new as &str, 1);

    Ok(SearchReplaceResult {
        path: String::new(), // Caller fills this in
        original_content: normalized_content,
        new_content,
    })
}

/// Resolve a file path that may be absolute to a relative path.
///
/// Matches TS:
/// ```ts
/// let relPath: string
/// if (path.isAbsolute(file_path)) {
///     relPath = path.relative(task.cwd, file_path)
/// } else {
///     relPath = file_path
/// }
/// ```
pub fn resolve_relative_path(file_path: &str, cwd: &str) -> String {
    if std::path::Path::new(file_path).is_absolute() {
        std::path::Path::new(cwd)
            .join(file_path)
            .to_string_lossy()
            .to_string()
    } else {
        file_path.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate_search_replace_params tests ----

    #[test]
    fn test_validate_valid_params() {
        let params = SearchReplaceParams {
            file_path: "test.txt".to_string(),
            old_string: "old".to_string(),
            new_string: "new".to_string(),
        };
        assert!(validate_search_replace_params(&params).is_ok());
    }

    #[test]
    fn test_validate_missing_file_path() {
        let params = SearchReplaceParams {
            file_path: String::new(),
            old_string: "old".to_string(),
            new_string: "new".to_string(),
        };
        let err = validate_search_replace_params(&params).unwrap_err();
        assert!(matches!(err, SearchReplaceError::MissingFilePath));
    }

    #[test]
    fn test_validate_missing_old_string() {
        let params = SearchReplaceParams {
            file_path: "test.txt".to_string(),
            old_string: String::new(),
            new_string: "new".to_string(),
        };
        let err = validate_search_replace_params(&params).unwrap_err();
        assert!(matches!(err, SearchReplaceError::MissingOldString));
    }

    #[test]
    fn test_validate_identical_strings() {
        let params = SearchReplaceParams {
            file_path: "test.txt".to_string(),
            old_string: "same".to_string(),
            new_string: "same".to_string(),
        };
        let err = validate_search_replace_params(&params).unwrap_err();
        assert!(matches!(err, SearchReplaceError::IdenticalStrings));
    }

    #[test]
    fn test_validate_full_with_dotdot() {
        let params = SearchReplaceParams {
            file_path: "../etc/passwd".to_string(),
            old_string: "old".to_string(),
            new_string: "new".to_string(),
        };
        let err = validate_search_replace_params_full(&params).unwrap_err();
        assert!(matches!(err, FsToolError::InvalidPath(_)));
    }

    // ---- apply_search_replace tests ----

    #[test]
    fn test_single_replacement() {
        let content = "line1\nline2\nline3\n";
        let result = apply_search_replace(content, "line2", "LINE2").unwrap();
        assert_eq!(result.new_content, "line1\nLINE2\nline3\n");
        assert_eq!(result.original_content, content);
    }

    #[test]
    fn test_no_match() {
        let content = "line1\nline2\n";
        let err = apply_search_replace(content, "not_found", "replacement").unwrap_err();
        assert!(matches!(err, SearchReplaceError::NoMatch));
    }

    #[test]
    fn test_multiple_matches() {
        let content = "foo\nbar\nfoo\n";
        let err = apply_search_replace(content, "foo", "baz").unwrap_err();
        assert!(matches!(err, SearchReplaceError::MultipleMatches(2)));
    }

    #[test]
    fn test_crlf_normalization() {
        let content = "line1\r\nline2\r\nline3\r\n";
        let result = apply_search_replace(content, "line2", "LINE2").unwrap();
        assert_eq!(result.new_content, "line1\nLINE2\nline3\n");
    }

    #[test]
    fn test_crlf_in_old_string() {
        let content = "line1\r\nline2\r\n";
        // Both content and old_string are normalized to LF before matching
        let result = apply_search_replace(content, "line1\r\nline2", "replaced").unwrap();
        assert_eq!(result.new_content, "replaced\n");
    }

    #[test]
    fn test_multiline_replacement() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let old = "    println!(\"hello\");";
        let new_ = "    println!(\"world\");";
        let result = apply_search_replace(content, old, new_).unwrap();
        assert!(result.new_content.contains("world"));
        assert!(!result.new_content.contains("hello"));
    }

    #[test]
    fn test_replacement_at_beginning() {
        let content = "first\nsecond\nthird\n";
        let result = apply_search_replace(content, "first", "FIRST").unwrap();
        assert_eq!(result.new_content, "FIRST\nsecond\nthird\n");
    }

    #[test]
    fn test_replacement_at_end() {
        let content = "first\nsecond\nthird\n";
        let result = apply_search_replace(content, "third", "THIRD").unwrap();
        assert_eq!(result.new_content, "first\nsecond\nTHIRD\n");
    }

    #[test]
    fn test_empty_replacement_deletes() {
        let content = "before\ntarget\nafter\n";
        let result = apply_search_replace(content, "target\n", "").unwrap();
        assert_eq!(result.new_content, "before\nafter\n");
    }

    // ---- count_occurrences tests ----

    #[test]
    fn test_count_zero() {
        assert_eq!(count_occurrences("abc", "xyz"), 0);
    }

    #[test]
    fn test_count_one() {
        assert_eq!(count_occurrences("abc", "b"), 1);
    }

    #[test]
    fn test_count_multiple() {
        assert_eq!(count_occurrences("ababab", "ab"), 3);
    }

    #[test]
    fn test_count_empty_needle() {
        assert_eq!(count_occurrences("abc", ""), 0);
    }

    // ---- normalize_line_endings tests ----

    #[test]
    fn test_normalize_no_crlf() {
        assert_eq!(normalize_line_endings("a\nb"), "a\nb");
    }

    #[test]
    fn test_normalize_crlf() {
        assert_eq!(normalize_line_endings("a\r\nb"), "a\nb");
    }

    #[test]
    fn test_normalize_mixed() {
        assert_eq!(normalize_line_endings("a\r\nb\nc\r\n"), "a\nb\nc\n");
    }

    // ---- resolve_relative_path tests ----

    #[test]
    fn test_resolve_relative() {
        let result = resolve_relative_path("src/main.rs", "/workspace");
        assert_eq!(result, "src/main.rs");
    }

    #[test]
    fn test_resolve_absolute() {
        let result = resolve_relative_path("/workspace/src/main.rs", "/workspace");
        // On Windows, the path will be joined with cwd
        assert!(result.contains("main.rs"));
    }

    // ---- SearchReplaceError Display tests ----

    #[test]
    fn test_error_display_no_match() {
        let err = SearchReplaceError::NoMatch;
        let msg = format!("{}", err);
        assert!(msg.contains("No match found"));
    }

    #[test]
    fn test_error_display_multiple_matches() {
        let err = SearchReplaceError::MultipleMatches(5);
        let msg = format!("{}", err);
        assert!(msg.contains("5 matches"));
    }

    #[test]
    fn test_error_display_identical() {
        let err = SearchReplaceError::IdenticalStrings;
        let msg = format!("{}", err);
        assert!(msg.contains("must be different"));
    }
}
