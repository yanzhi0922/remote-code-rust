//! search_files tool implementation.
//!
//! Provides two search backends:
//! - **ripgrep** (`rg`): Fast, respects .gitignore, used when available.
//! - **Pure Rust fallback**: Regex-based search when ripgrep is not installed.

use crate::helpers::*;
use crate::types::*;
use roo_types::tool::SearchFilesParams;
use roo_ignore::RooIgnoreController;
use std::path::Path;
use std::process::Command;

/// Validate search_files parameters.
pub fn validate_search_files_params(params: &SearchFilesParams) -> Result<(), SearchToolError> {
    if params.path.trim().is_empty() {
        return Err(SearchToolError::Validation(
            "path must not be empty".to_string(),
        ));
    }

    if params.regex.trim().is_empty() {
        return Err(SearchToolError::Validation(
            "regex pattern must not be empty".to_string(),
        ));
    }

    // Validate regex with detailed error message
    validate_regex_detailed(&params.regex)?;

    // Validate file pattern if provided
    if let Some(ref pattern) = params.file_pattern {
        if !pattern.is_empty() {
            validate_file_pattern(pattern)?;
        }
    }

    Ok(())
}

/// Validate a regex pattern with a detailed error message.
///
/// Provides suggestions for common regex mistakes.
fn validate_regex_detailed(pattern: &str) -> Result<regex::Regex, SearchToolError> {
    regex::Regex::new(pattern).map_err(|e| {
        let suggestion = if pattern.contains('[') && !pattern.contains(']') {
            "\nHint: Unmatched '[' — did you forget to close the bracket?"
        } else if pattern.contains('(') && !pattern.contains(')') {
            "\nHint: Unmatched '(' — did you forget to close the parenthesis?"
        } else if pattern.ends_with('\\') {
            "\nHint: Trailing backslash — did you mean to escape it ('\\\\')?"
        } else {
            ""
        };
        SearchToolError::InvalidRegex(format!(
            "Invalid regex pattern '{}': {}.{} \
             Please use valid Rust regex syntax (https://docs.rs/regex). \
             Common patterns: '\\d+' for digits, '\\w+' for word characters, \
             'fn\\s+\\w+' for function definitions.",
            pattern, e, suggestion
        ))
    })
}

/// Check if ripgrep (`rg`) is available on the system PATH.
pub fn is_ripgrep_available() -> bool {
    Command::new("rg")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Execute a file search, preferring ripgrep when available.
///
/// Returns a list of file matches with context lines.
pub fn search_files(
    params: &SearchFilesParams,
    cwd: &Path,
) -> Result<Vec<FileMatch>, SearchToolError> {
    validate_search_files_params(params)?;

    let search_path = if Path::new(&params.path).is_absolute() {
        std::path::PathBuf::from(&params.path)
    } else {
        cwd.join(&params.path)
    };

    if !search_path.exists() {
        return Err(SearchToolError::Validation(format!(
            "path does not exist: {}",
            params.path
        )));
    }

    // Try ripgrep first
    if is_ripgrep_available() {
        if let Ok(results) = search_with_ripgrep(&params.regex, &search_path, params.file_pattern.as_deref()) {
            return Ok(results);
        }
    }

    // Fallback to pure Rust search
    search_with_regex(&params.regex, &search_path, params.file_pattern.as_deref())
}

/// Search files using ripgrep (`rg`).
///
/// Uses `--json` flag for structured output. Respects .gitignore by default.
/// Falls back to regex search on error.
fn search_with_ripgrep(
    pattern: &str,
    search_path: &Path,
    file_pattern: Option<&str>,
) -> Result<Vec<FileMatch>, SearchToolError> {
    let mut cmd = Command::new("rg");
    cmd.arg("--json")
        .arg("--max-count=50")
        .arg("--context=2")
        .arg("--no-heading")
        .arg("--color=never")
        .arg(pattern)
        .arg(search_path);

    // Add glob filter if file_pattern is specified
    if let Some(fp) = file_pattern {
        if !fp.is_empty() && fp != "*" {
            cmd.arg("--glob").arg(fp);
        }
    }

    let output = cmd.output().map_err(|e| {
        SearchToolError::Validation(format!("Failed to execute ripgrep: {}", e))
    })?;

    if !output.status.success() && !output.status.code().map_or(false, |c| c == 1) {
        // Exit code 1 means no matches, which is fine.
        // Other errors fall through to the regex fallback.
        return Err(SearchToolError::Validation(
            "ripgrep execution failed".to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ripgrep_output(&stdout)
}

/// Parse ripgrep JSON output into FileMatch results.
fn parse_ripgrep_output(output: &str) -> Result<Vec<FileMatch>, SearchToolError> {
    let mut matches = Vec::new();

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        // Try to parse as JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

            if msg_type == "match" {
                let file_path = json
                    .get("data")
                    .and_then(|d| d.get("path"))
                    .and_then(|p| p.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();

                let line_content = json
                    .get("data")
                    .and_then(|d| d.get("lines"))
                    .and_then(|l| l.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();

                let line_number = json
                    .get("data")
                    .and_then(|d| d.get("line_number"))
                    .and_then(|n| n.as_u64())
                    .unwrap_or(0) as usize;

                if !file_path.is_empty() {
                    matches.push(FileMatch {
                        file_path,
                        line_number,
                        line_content,
                        context_before: vec![],
                        context_after: vec![],
                    });
                }

                if matches.len() >= 100 {
                    break;
                }
            }
        }
    }

    Ok(matches)
}

/// Search files using pure Rust regex (fallback when ripgrep is unavailable).
///
/// Walks the directory tree, reads each file, and applies the regex pattern.
fn search_with_regex(
    pattern: &str,
    search_path: &Path,
    file_pattern: Option<&str>,
) -> Result<Vec<FileMatch>, SearchToolError> {
    let re = regex::Regex::new(pattern)
        .map_err(|e| SearchToolError::Validation(format!("Invalid regex: {}", e)))?;

    let mut results = Vec::new();
    search_in_dir_pure(&re, search_path, file_pattern, &mut results, 100);
    Ok(results)
}

/// Recursively search a directory using regex.
fn search_in_dir_pure(
    re: &regex::Regex,
    dir: &Path,
    file_pattern: Option<&str>,
    results: &mut Vec<FileMatch>,
    max_results: usize,
) {
    if results.len() >= max_results {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if results.len() >= max_results {
            return;
        }

        let path = entry.path();

        // Skip hidden directories and common ignored directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "__pycache__"
            {
                continue;
            }
        }

        if path.is_dir() {
            search_in_dir_pure(re, &path, file_pattern, results, max_results);
        } else if path.is_file() {
            // Apply file pattern filter
            if let Some(fp) = file_pattern {
                if !fp.is_empty()
                    && fp != "*"
                    && !matches_file_pattern(path.to_str().unwrap_or(""), fp)
                {
                    continue;
                }
            }

            // Try to read and search the file
            if let Ok(content) = std::fs::read_to_string(&path) {
                for (line_num, line) in content.lines().enumerate() {
                    if results.len() >= max_results {
                        return;
                    }
                    if re.is_match(line) {
                        results.push(FileMatch {
                            file_path: path.to_string_lossy().to_string(),
                            line_number: line_num + 1,
                            line_content: line.to_string(),
                            context_before: vec![],
                            context_after: vec![],
                        });
                    }
                }
            }
        }
    }
}

/// Check if a file path matches a glob pattern.
///
/// Simple glob matching: supports `*` (any chars) and `**` (recursive).
pub fn matches_file_pattern(file_path: &str, pattern: &str) -> bool {
    if pattern.is_empty() || pattern == "*" {
        return true;
    }

    // Simple glob matching
    let pattern = pattern.replace("**", "⟶"); // Temporarily replace **
    let parts: Vec<&str> = pattern.split('*').collect();

    let mut remaining = file_path;
    for (i, part) in parts.iter().enumerate() {
        let part = part.replace("⟶", "");
        if part.is_empty() {
            continue;
        }
        if let Some(pos) = remaining.find(&part) {
            remaining = &remaining[pos + part.len()..];
        } else if i == 0 {
            return remaining.starts_with(part.as_str());
        } else {
            return false;
        }
    }
    true
}

/// Filter search results by removing matches from paths ignored by .rooignore.
///
/// Each match's `file_path` is checked against the controller's ignore rules.
/// Matches to ignored paths are removed from the results.
pub fn filter_results_by_rooignore(
    results: Vec<FileMatch>,
    controller: Option<&RooIgnoreController>,
) -> Vec<FileMatch> {
    match controller {
        Some(ctrl) => results
            .into_iter()
            .filter(|m| ctrl.validate_access(&m.file_path))
            .collect(),
        None => results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_path() {
        let params = SearchFilesParams {
            path: "".to_string(),
            regex: "pattern".to_string(),
            file_pattern: None,
        };
        assert!(validate_search_files_params(&params).is_err());
    }

    #[test]
    fn test_validate_empty_regex() {
        let params = SearchFilesParams {
            path: "src".to_string(),
            regex: "".to_string(),
            file_pattern: None,
        };
        assert!(validate_search_files_params(&params).is_err());
    }

    #[test]
    fn test_validate_invalid_regex() {
        let params = SearchFilesParams {
            path: "src".to_string(),
            regex: "[invalid".to_string(),
            file_pattern: None,
        };
        assert!(validate_search_files_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid() {
        let params = SearchFilesParams {
            path: "src".to_string(),
            regex: r"fn\s+\w+".to_string(),
            file_pattern: Some("*.rs".to_string()),
        };
        assert!(validate_search_files_params(&params).is_ok());
    }

    #[test]
    fn test_validate_no_file_pattern() {
        let params = SearchFilesParams {
            path: "src".to_string(),
            regex: "pattern".to_string(),
            file_pattern: None,
        };
        assert!(validate_search_files_params(&params).is_ok());
    }

    #[test]
    fn test_matches_file_pattern_rs() {
        assert!(matches_file_pattern("src/main.rs", "*.rs"));
    }

    #[test]
    fn test_matches_file_pattern_any() {
        assert!(matches_file_pattern("any/path.txt", "*"));
    }

    #[test]
    fn test_matches_file_pattern_no_match() {
        assert!(!matches_file_pattern("main.py", "*.rs"));
    }

    #[test]
    fn test_matches_file_pattern_empty() {
        assert!(matches_file_pattern("anything", ""));
    }

    // --- L5.2: ripgrep integration tests ---

    #[test]
    fn test_is_ripgrep_available() {
        // This test just checks the function doesn't panic
        let _ = is_ripgrep_available();
    }

    #[test]
    fn test_search_with_regex_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let results = search_with_regex(r"fn\s+\w+", dir.path(), Some("*.rs")).unwrap();

        assert!(!results.is_empty());
        assert!(results[0].line_content.contains("fn main"));
        assert_eq!(results[0].line_number, 1);
    }

    #[test]
    fn test_search_with_regex_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "struct Foo;\n").unwrap();

        let results = search_with_regex(r"fn\s+\w+", dir.path(), None).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn test_search_with_regex_file_pattern_filter() {
        let dir = tempfile::tempdir().unwrap();
        let rs_file = dir.path().join("test.rs");
        let py_file = dir.path().join("test.py");
        std::fs::write(&rs_file, "fn main() {}\n").unwrap();
        std::fs::write(&py_file, "def main():\n    pass\n").unwrap();

        let results = search_with_regex("main", dir.path(), Some("*.rs")).unwrap();

        // Should only find the .rs file
        assert!(results.iter().all(|m| m.file_path.ends_with(".rs")));
    }

    #[test]
    fn test_search_with_regex_max_results() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content: String = (0..200).map(|i| format!("match_line_{}\n", i)).collect();
        std::fs::write(&file_path, content).unwrap();

        let mut results = Vec::new();
        search_in_dir_pure(
            &regex::Regex::new("match_line").unwrap(),
            dir.path(),
            None,
            &mut results,
            10,
        );

        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_search_files_function() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn helper() -> i32 {\n    42\n}\n").unwrap();

        let params = SearchFilesParams {
            path: dir.path().to_str().unwrap().to_string(),
            regex: r"fn\s+\w+".to_string(),
            file_pattern: None,
        };

        let results = search_files(&params, Path::new(".")).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].line_content.contains("fn helper"));
    }

    #[test]
    fn test_parse_ripgrep_output_empty() {
        let result = parse_ripgrep_output("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_ripgrep_output_invalid_json() {
        let result = parse_ripgrep_output("not json\nalso not json").unwrap();
        assert!(result.is_empty());
    }

    // --- RooIgnore filtering tests ---

    #[test]
    fn test_filter_results_by_rooignore_no_controller() {
        let results = vec![FileMatch {
            file_path: "secret.txt".to_string(),
            line_number: 1,
            line_content: "secret".to_string(),
            context_before: vec![],
            context_after: vec![],
        }];
        let filtered = filter_results_by_rooignore(results, None);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_filter_results_by_rooignore_blocks_ignored() {
        let mut ctrl = roo_ignore::RooIgnoreController::new("/tmp");
        ctrl.load_patterns("secret.txt");
        let results = vec![
            FileMatch {
                file_path: "src/main.rs".to_string(),
                line_number: 1,
                line_content: "fn main".to_string(),
                context_before: vec![],
                context_after: vec![],
            },
            FileMatch {
                file_path: "secret.txt".to_string(),
                line_number: 1,
                line_content: "secret".to_string(),
                context_before: vec![],
                context_after: vec![],
            },
        ];
        let filtered = filter_results_by_rooignore(results, Some(&ctrl));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].file_path, "src/main.rs");
    }

    // --- Detailed regex error tests ---

    #[test]
    fn test_invalid_regex_detailed_error_unmatched_bracket() {
        let params = SearchFilesParams {
            path: "src".to_string(),
            regex: "[invalid".to_string(),
            file_pattern: None,
        };
        let result = validate_search_files_params(&params);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("[invalid"), "Error should include the pattern");
        assert!(err.contains("regex syntax"), "Error should mention regex syntax");
    }

    #[test]
    fn test_invalid_regex_detailed_error_trailing_backslash() {
        let params = SearchFilesParams {
            path: "src".to_string(),
            regex: "test\\".to_string(),
            file_pattern: None,
        };
        let result = validate_search_files_params(&params);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Trailing backslash"), "Error should hint about trailing backslash");
    }
}
