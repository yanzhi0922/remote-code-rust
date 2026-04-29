//! Types for search tool results and errors.

use serde::{Deserialize, Serialize};

/// Result of a search_files operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub pattern: String,
    pub file_pattern: Option<String>,
    pub matches: Vec<FileMatch>,
    pub total_files_searched: usize,
    pub truncated: bool,
}

/// A single file match from search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatch {
    pub file_path: String,
    pub line_number: usize,
    pub line_content: String,
    /// Context lines before the match (L5.2 enhancement).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_before: Vec<String>,
    /// Context lines after the match (L5.2 enhancement).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_after: Vec<String>,
}

/// Result of a list_files operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResult {
    pub path: String,
    pub recursive: bool,
    pub files: Vec<String>,
    pub directories: Vec<String>,
    pub total_count: usize,
    pub truncated: bool,
}

/// Result of a codebase_search operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseSearchResult {
    pub query: String,
    pub directory_prefix: Option<String>,
    pub results: Vec<CodebaseMatch>,
    pub total_results: usize,
}

/// A single codebase search match, aligned with TS `CodebaseSearchTool.ts` output.
///
/// Each match includes the file path, relevance score, line range, and the
/// matching code chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseMatch {
    /// Relative file path of the match.
    pub file_path: String,
    /// Cosine similarity score (0.0–1.0).
    pub score: f64,
    /// Start line of the code chunk (1-based).
    pub start_line: usize,
    /// End line of the code chunk (1-based).
    pub end_line: usize,
    /// The matching code chunk content.
    pub code_chunk: String,
}

/// Search options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub max_results: usize,
    pub context_lines: usize,
    pub case_sensitive: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_results: 100,
            context_lines: 2,
            case_sensitive: false,
        }
    }
}

/// Error type for search tool operations.
#[derive(Debug, thiserror::Error)]
pub enum SearchToolError {
    #[error("Invalid regex: {0}")]
    InvalidRegex(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Invalid file pattern: {0}")]
    InvalidFilePattern(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Search error: {0}")]
    Search(String),
}

/// Maximum number of search results.
pub const MAX_SEARCH_RESULTS: usize = 500;

/// Maximum number of file list entries.
pub const MAX_FILE_LIST_ENTRIES: usize = 1000;
