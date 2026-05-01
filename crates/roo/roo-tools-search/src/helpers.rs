//! Helper functions for search tools.

use crate::types::SearchToolError;
use regex::Regex;

/// Validate a regex pattern.
pub fn validate_regex(pattern: &str) -> Result<Regex, SearchToolError> {
    Regex::new(pattern)
        .map_err(|e| SearchToolError::InvalidRegex(format!("Invalid regex '{pattern}': {e}")))
}

/// Validate a file glob pattern.
///
/// Basic validation: must not be empty, must not contain path traversal.
pub fn validate_file_pattern(pattern: &str) -> Result<(), SearchToolError> {
    if pattern.is_empty() {
        return Err(SearchToolError::InvalidFilePattern(
            "file pattern must not be empty".to_string(),
        ));
    }

    if pattern.contains("..") {
        return Err(SearchToolError::InvalidFilePattern(
            "file pattern must not contain '..'".to_string(),
        ));
    }

    Ok(())
}

/// Format search results with line numbers and context.
pub fn format_search_results(matches: &[crate::types::FileMatch]) -> String {
    let mut output = String::new();
    for m in matches {
        output.push_str(&format!(
            "{}:{}: {}\n",
            m.file_path, m.line_number, m.line_content
        ));
    }
    output
}

/// Format a file list for display.
///
/// Sorts files and directories, and applies a limit.
pub fn format_file_list(
    files: &[String],
    directories: &[String],
    limit: usize,
) -> (String, bool) {
    let mut all_entries: Vec<&str> = directories.iter().map(|s| s.as_str()).collect();
    let mut file_entries: Vec<&str> = files.iter().map(|s| s.as_str()).collect();

    all_entries.sort();
    file_entries.sort();

    all_entries.extend(file_entries);

    let truncated = all_entries.len() > limit;
    let display: Vec<&str> = all_entries.into_iter().take(limit).collect();

    let result = if truncated {
        format!(
            "{}\n... ({} more entries)",
            display.join("\n"),
            display.len().saturating_sub(limit)
        )
    } else {
        display.join("\n")
    };

    (result, truncated)
}

/// Format codebase search results as structured output.
pub fn format_codebase_results(results: &[crate::types::CodebaseMatch]) -> String {
    let mut output = String::new();
    for r in results {
        output.push_str(&format!(
            "{}:{}-{}: {} (score: {:.3})\n",
            r.file_path, r.start_line, r.end_line, r.code_chunk, r.score
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate_regex tests ----

    #[test]
    fn test_valid_regex() {
        assert!(validate_regex(r"\d+").is_ok());
    }

    #[test]
    fn test_valid_regex_simple() {
        assert!(validate_regex("hello").is_ok());
    }

    #[test]
    fn test_invalid_regex() {
        assert!(validate_regex("[invalid").is_err());
    }

    #[test]
    fn test_valid_regex_complex() {
        assert!(validate_regex(r"fn\s+\w+\(").is_ok());
    }

    // ---- validate_file_pattern tests ----

    #[test]
    fn test_valid_pattern() {
        assert!(validate_file_pattern("*.rs").is_ok());
    }

    #[test]
    fn test_valid_pattern_complex() {
        assert!(validate_file_pattern("**/*.ts").is_ok());
    }

    #[test]
    fn test_invalid_empty_pattern() {
        assert!(validate_file_pattern("").is_err());
    }

    #[test]
    fn test_invalid_traversal_pattern() {
        assert!(validate_file_pattern("../secret").is_err());
    }

    // ---- format_search_results tests ----

    #[test]
    fn test_format_results_basic() {
        let matches = vec![
            crate::types::FileMatch {
                file_path: "src/main.rs".to_string(),
                line_number: 10,
                line_content: "fn main() {".to_string(),
                context_before: vec![],
                context_after: vec![],
            },
            crate::types::FileMatch {
                file_path: "src/lib.rs".to_string(),
                line_number: 5,
                line_content: "pub mod foo;".to_string(),
                context_before: vec![],
                context_after: vec![],
            },
        ];
        let result = format_search_results(&matches);
        assert!(result.contains("src/main.rs:10: fn main()"));
        assert!(result.contains("src/lib.rs:5: pub mod foo"));
    }

    #[test]
    fn test_format_results_empty() {
        let result = format_search_results(&[]);
        assert!(result.is_empty());
    }

    // ---- format_file_list tests ----

    #[test]
    fn test_format_file_list_basic() {
        let files = vec!["b.rs".to_string(), "a.rs".to_string()];
        let dirs = vec!["src".to_string()];
        let (result, truncated) = format_file_list(&files, &dirs, 100);
        assert!(!truncated);
        assert!(result.contains("src"));
        assert!(result.contains("a.rs"));
    }

    #[test]
    fn test_format_file_list_truncated() {
        let files: Vec<String> = (0..20).map(|i| format!("file{i}.rs")).collect();
        let (result, truncated) = format_file_list(&files, &[], 5);
        assert!(truncated);
        assert!(result.contains("more entries"));
    }

    #[test]
    fn test_format_file_list_empty() {
        let (result, truncated) = format_file_list(&[], &[], 100);
        assert!(!truncated);
        assert!(result.is_empty());
    }

    // ---- format_codebase_results tests ----

    #[test]
    fn test_format_codebase_results() {
        let results = vec![
            crate::types::CodebaseMatch {
                file_path: "src/main.rs".to_string(),
                score: 0.95,
                start_line: 1,
                end_line: 10,
                code_chunk: "fn main() {}".to_string(),
            },
        ];
        let result = format_codebase_results(&results);
        assert!(result.contains("src/main.rs:1-10: fn main() {}"));
        assert!(result.contains("score: 0.950"));
    }

    // ---- SearchResult tests ----

    #[test]
    fn test_search_result_serde() {
        let r = crate::types::SearchResult {
            path: "src".to_string(),
            pattern: "fn main".to_string(),
            file_pattern: Some("*.rs".to_string()),
            matches: vec![],
            total_files_searched: 10,
            truncated: false,
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: crate::types::SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_files_searched, 10);
    }

    // ---- FileListResult tests ----

    #[test]
    fn test_file_list_result_truncated() {
        let r = crate::types::FileListResult {
            path: ".".to_string(),
            recursive: true,
            files: vec!["a".to_string()],
            directories: vec![],
            total_count: 100,
            truncated: true,
        };
        assert!(r.truncated);
        assert_eq!(r.total_count, 100);
    }

    // ---- SearchToolError tests ----

    #[test]
    fn test_search_tool_error_display() {
        let err = SearchToolError::InvalidRegex("bad regex".to_string());
        assert!(format!("{err}").contains("bad regex"));
    }

    // ---- SearchOptions tests ----

    #[test]
    fn test_search_options_default() {
        let opts = crate::types::SearchOptions::default();
        assert_eq!(opts.max_results, 100);
        assert_eq!(opts.context_lines, 2);
        assert!(!opts.case_sensitive);
    }
}
