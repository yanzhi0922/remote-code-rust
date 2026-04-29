//! codebase_search tool implementation.
//!
//! Aligned with TS `CodebaseSearchTool.ts`:
//! - Validates the `query` parameter.
//! - Uses `roo_index::CodeIndexManager` for vector search.
//! - Returns results in the TS format: file path + score + line range + code chunk.
//!
//! TODO: Replace simulated search with actual query embedding + cosine similarity
//! when the embedding backend is integrated.

use crate::types::*;
use roo_index::CodeIndexManager;
use roo_types::tool::CodebaseSearchParams;

/// Validate codebase_search parameters.
pub fn validate_codebase_search_params(
    params: &CodebaseSearchParams,
) -> Result<(), SearchToolError> {
    if params.query.trim().is_empty() {
        return Err(SearchToolError::Validation(
            "query must not be empty".to_string(),
        ));
    }

    Ok(())
}

/// Process a codebase search using the `CodeIndexManager`.
///
/// This function:
/// 1. Validates the query.
/// 2. Searches the index via `CodeIndexManager::search_with_prefix`.
/// 3. Converts `VectorStoreSearchResult` to `CodebaseMatch`.
/// 4. Returns a `CodebaseSearchResult` with formatted output.
///
/// If the manager is not initialized or not configured, returns an error.
pub fn process_codebase_search(
    params: &CodebaseSearchParams,
    manager: &CodeIndexManager,
    limit: usize,
) -> Result<CodebaseSearchResult, SearchToolError> {
    validate_codebase_search_params(params)?;

    if !manager.is_initialized() {
        return Err(SearchToolError::Search(
            "Code index is not initialized. Please initialize the index first.".to_string(),
        ));
    }

    let directory_prefix = params.directory_prefix.as_deref();
    let raw_results = manager.search_with_prefix(&params.query, directory_prefix, limit);

    if raw_results.is_empty() {
        return Ok(CodebaseSearchResult {
            query: params.query.clone(),
            directory_prefix: params.directory_prefix.clone(),
            results: vec![],
            total_results: 0,
        });
    }

    // Convert VectorStoreSearchResult → CodebaseMatch
    let matches: Vec<CodebaseMatch> = raw_results
        .into_iter()
        .map(|r| CodebaseMatch {
            file_path: r.file_path,
            score: r.score,
            start_line: r.start_line.unwrap_or(1) as usize,
            end_line: r.end_line.unwrap_or(1) as usize,
            code_chunk: r.code_chunk.unwrap_or_default().trim().to_string(),
        })
        .collect();

    let total = matches.len();
    Ok(CodebaseSearchResult {
        query: params.query.clone(),
        directory_prefix: params.directory_prefix.clone(),
        results: matches,
        total_results: total,
    })
}

/// Build a CodebaseSearchResult from pre-computed matches.
///
/// Useful when the caller already has search results and just needs to
/// assemble them into the standard result structure.
pub fn build_codebase_search_result(
    query: &str,
    directory_prefix: Option<&str>,
    matches: Vec<CodebaseMatch>,
) -> CodebaseSearchResult {
    let total_results = matches.len();
    CodebaseSearchResult {
        query: query.to_string(),
        directory_prefix: directory_prefix.map(|s| s.to_string()),
        results: matches,
        total_results,
    }
}

/// Format a codebase search result as human-readable output, matching TS format.
///
/// TS output format:
/// ```text
/// Query: <query>
/// Results:
///
/// File path: <path>
/// Score: <score>
/// Lines: <start>-<end>
/// Code Chunk: <chunk>
/// ```
pub fn format_codebase_search_output(result: &CodebaseSearchResult) -> String {
    if result.total_results == 0 {
        return format!("No relevant code snippets found for the query: \"{}\"", result.query);
    }

    let mut output = format!("Query: {}\nResults:\n", result.query);

    for m in &result.results {
        output.push_str(&format!(
            "\nFile path: {}\nScore: {}\nLines: {}-{}\nCode Chunk: {}\n",
            m.file_path, m.score, m.start_line, m.end_line, m.code_chunk
        ));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use roo_index::{CodeIndexConfig, CodeIndexManager};

    /// Create a test manager with some indexed files.
    fn test_manager() -> CodeIndexManager {
        let config = CodeIndexConfig {
            enabled: true,
            max_file_size: 1_000_000,
            include_patterns: vec![],
            exclude_patterns: vec![],
            workspace_path: None,
        };
        let mut mgr = CodeIndexManager::new(config);
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        mgr.add_file("src/lib.rs").unwrap();
        mgr.add_file("src/utils/helpers.rs").unwrap();
        mgr.add_file("tests/integration.rs").unwrap();
        mgr
    }

    #[test]
    fn test_validate_empty_query() {
        let params = CodebaseSearchParams {
            query: "".to_string(),
            directory_prefix: None,
        };
        assert!(validate_codebase_search_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid() {
        let params = CodebaseSearchParams {
            query: "fn main".to_string(),
            directory_prefix: Some("src".to_string()),
        };
        assert!(validate_codebase_search_params(&params).is_ok());
    }

    #[test]
    fn test_process_search_basic() {
        let mgr = test_manager();
        let params = CodebaseSearchParams {
            query: "main".to_string(),
            directory_prefix: None,
        };
        let result = process_codebase_search(&params, &mgr, 10).unwrap();
        assert_eq!(result.query, "main");
        assert!(result.total_results > 0);
        // Should find src/main.rs
        assert!(result.results.iter().any(|m| m.file_path == "src/main.rs"));
    }

    #[test]
    fn test_process_search_with_prefix() {
        let mgr = test_manager();
        let params = CodebaseSearchParams {
            query: "rs".to_string(),
            directory_prefix: Some("src/utils".to_string()),
        };
        let result = process_codebase_search(&params, &mgr, 10).unwrap();
        // Should only find files under src/utils
        assert!(result.results.iter().all(|m| m.file_path.starts_with("src/utils")));
    }

    #[test]
    fn test_process_search_no_results() {
        let mgr = test_manager();
        let params = CodebaseSearchParams {
            query: "nonexistent_xyz_12345".to_string(),
            directory_prefix: None,
        };
        let result = process_codebase_search(&params, &mgr, 10).unwrap();
        assert_eq!(result.total_results, 0);
    }

    #[test]
    fn test_process_search_uninitialized() {
        let config = CodeIndexConfig::default();
        let mgr = CodeIndexManager::new(config);
        let params = CodebaseSearchParams {
            query: "test".to_string(),
            directory_prefix: None,
        };
        let result = process_codebase_search(&params, &mgr, 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }

    #[test]
    fn test_format_output_matches_ts() {
        let result = CodebaseSearchResult {
            query: "fn main".to_string(),
            directory_prefix: None,
            results: vec![CodebaseMatch {
                file_path: "src/main.rs".to_string(),
                score: 0.95,
                start_line: 1,
                end_line: 10,
                code_chunk: "fn main() {\n    println!(\"hello\");\n}".to_string(),
            }],
            total_results: 1,
        };
        let output = format_codebase_search_output(&result);
        assert!(output.contains("Query: fn main"));
        assert!(output.contains("File path: src/main.rs"));
        assert!(output.contains("Score: 0.95"));
        assert!(output.contains("Lines: 1-10"));
        assert!(output.contains("Code Chunk: fn main()"));
    }

    #[test]
    fn test_format_output_empty_results() {
        let result = CodebaseSearchResult {
            query: "nothing".to_string(),
            directory_prefix: None,
            results: vec![],
            total_results: 0,
        };
        let output = format_codebase_search_output(&result);
        assert!(output.contains("No relevant code snippets found"));
        assert!(output.contains("\"nothing\""));
    }

    #[test]
    fn test_build_result() {
        let matches = vec![
            CodebaseMatch {
                file_path: "src/main.rs".to_string(),
                score: 0.99,
                start_line: 1,
                end_line: 5,
                code_chunk: "fn main() {}".to_string(),
            },
        ];
        let result = build_codebase_search_result("fn main", Some("src"), matches);
        assert_eq!(result.total_results, 1);
        assert_eq!(result.query, "fn main");
        assert_eq!(result.directory_prefix, Some("src".to_string()));
    }

    #[test]
    fn test_format_output_multiple_results() {
        let result = CodebaseSearchResult {
            query: "test".to_string(),
            directory_prefix: Some("src".to_string()),
            results: vec![
                CodebaseMatch {
                    file_path: "src/a.rs".to_string(),
                    score: 0.9,
                    start_line: 5,
                    end_line: 15,
                    code_chunk: "fn test_a()".to_string(),
                },
                CodebaseMatch {
                    file_path: "src/b.rs".to_string(),
                    score: 0.8,
                    start_line: 20,
                    end_line: 30,
                    code_chunk: "fn test_b()".to_string(),
                },
            ],
            total_results: 2,
        };
        let output = format_codebase_search_output(&result);
        assert!(output.contains("src/a.rs"));
        assert!(output.contains("src/b.rs"));
        assert!(output.contains("Score: 0.9"));
        assert!(output.contains("Score: 0.8"));
        assert!(output.contains("Lines: 5-15"));
        assert!(output.contains("Lines: 20-30"));
    }

    #[test]
    fn test_match_fields_alignment() {
        let m = CodebaseMatch {
            file_path: "src/lib.rs".to_string(),
            score: 0.75,
            start_line: 42,
            end_line: 55,
            code_chunk: "pub fn search() {}".to_string(),
        };
        assert_eq!(m.file_path, "src/lib.rs");
        assert!((m.score - 0.75).abs() < f64::EPSILON);
        assert_eq!(m.start_line, 42);
        assert_eq!(m.end_line, 55);
        assert_eq!(m.code_chunk, "pub fn search() {}");
    }
}
