use serde::{Deserialize, Serialize};

/// Current state of the code index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexingState {
    NotInitialized,
    Idle,
    Indexing,
    Error,
    ShuttingDown,
}

/// A single search result from the vector store.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VectorStoreSearchResult {
    pub file_path: String,
    pub line_number: Option<u32>,
    pub content: String,
    pub score: f64,
    /// Start line of the code chunk (1-based).
    #[serde(default)]
    pub start_line: Option<u32>,
    /// End line of the code chunk (1-based).
    #[serde(default)]
    pub end_line: Option<u32>,
    /// The code chunk content.
    #[serde(default)]
    pub code_chunk: Option<String>,
}

/// Configuration for the code index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeIndexConfig {
    pub enabled: bool,
    pub max_file_size: u64,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    /// Root directory for resolving relative file paths.
    /// When set, file contents will be read and cached during indexing
    /// to enable content-based search with BM25-like scoring.
    #[serde(default)]
    pub workspace_path: Option<String>,
}

impl Default for CodeIndexConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_file_size: 1_000_000, // 1 MB
            include_patterns: vec!["**/*.rs".to_string(), "**/*.ts".to_string()],
            exclude_patterns: vec!["**/target/**".to_string(), "**/node_modules/**".to_string()],
            workspace_path: None,
        }
    }
}

/// Statistics about the current state of the index.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_files: usize,
    pub indexed_files: usize,
    pub total_chunks: usize,
}

/// Errors that can occur during indexing operations.
#[derive(Clone, Debug, thiserror::Error)]
pub enum IndexError {
    #[error("index not initialized")]
    NotInitialized,

    #[error("indexing already in progress")]
    AlreadyIndexing,

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("file too large: {0} bytes")]
    FileTooLarge(u64),

    #[error("index error: {0}")]
    GeneralError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexing_state_serde_roundtrip() {
        let states = vec![
            IndexingState::NotInitialized,
            IndexingState::Idle,
            IndexingState::Indexing,
            IndexingState::Error,
            IndexingState::ShuttingDown,
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let deserialized: IndexingState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, deserialized);
        }
    }

    #[test]
    fn test_vector_store_search_result_serialization() {
        let result = VectorStoreSearchResult {
            file_path: "src/main.rs".to_string(),
            line_number: Some(42),
            content: "fn main() {}".to_string(),
            score: 0.95,
            start_line: Some(42),
            end_line: Some(50),
            code_chunk: Some("fn main() {}".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: VectorStoreSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.file_path, deserialized.file_path);
        assert_eq!(result.line_number, deserialized.line_number);
        assert!((result.score - deserialized.score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_code_index_config_default() {
        let config = CodeIndexConfig::default();
        assert!(config.enabled);
        assert_eq!(1_000_000, config.max_file_size);
        assert!(!config.include_patterns.is_empty());
        assert!(!config.exclude_patterns.is_empty());
    }

    #[test]
    fn test_code_index_config_custom() {
        let config = CodeIndexConfig {
            enabled: false,
            max_file_size: 500,
            include_patterns: vec!["*.py".to_string()],
            exclude_patterns: vec![],
            workspace_path: None,
        };
        assert!(!config.enabled);
        assert_eq!(500, config.max_file_size);
    }

    #[test]
    fn test_index_stats_default() {
        let stats = IndexStats::default();
        assert_eq!(0, stats.total_files);
        assert_eq!(0, stats.indexed_files);
        assert_eq!(0, stats.total_chunks);
    }

    #[test]
    fn test_index_error_display() {
        assert_eq!(
            "index not initialized",
            format!("{}", IndexError::NotInitialized)
        );
        assert_eq!(
            "indexing already in progress",
            format!("{}", IndexError::AlreadyIndexing)
        );
        assert_eq!(
            "file not found: test.rs",
            format!("{}", IndexError::FileNotFound("test.rs".to_string()))
        );
        assert_eq!(
            "file too large: 2000000 bytes",
            format!("{}", IndexError::FileTooLarge(2_000_000))
        );
        assert_eq!(
            "index error: something went wrong",
            format!("{}", IndexError::GeneralError("something went wrong".to_string()))
        );
    }

    #[test]
    fn test_index_stats_serialization() {
        let stats = IndexStats {
            total_files: 100,
            indexed_files: 80,
            total_chunks: 500,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: IndexStats = serde_json::from_str(&json).unwrap();
        assert_eq!(100, deserialized.total_files);
        assert_eq!(80, deserialized.indexed_files);
        assert_eq!(500, deserialized.total_chunks);
    }
}
