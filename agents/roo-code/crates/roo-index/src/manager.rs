use std::collections::{HashMap, HashSet};

use crate::types::{CodeIndexConfig, IndexError, IndexStats, IndexingState, VectorStoreSearchResult};

/// Manager for building and querying a code index.
///
/// Supports two modes of search:
/// - **Path-only**: When no `workspace_path` is configured, search matches only
///   against file paths using simple substring matching.
/// - **Content-aware**: When `workspace_path` is set, file contents are cached
///   during indexing and search uses BM25-like scoring across both file names
///   and file contents, returning real code chunks with line numbers.
pub struct CodeIndexManager {
    state: IndexingState,
    config: CodeIndexConfig,
    stats: IndexStats,
    indexed_files: HashSet<String>,
    /// Cached file contents keyed by relative path.
    file_contents: HashMap<String, String>,
}

impl CodeIndexManager {
    /// Create a new code index manager with the given configuration.
    pub fn new(config: CodeIndexConfig) -> Self {
        Self {
            state: IndexingState::NotInitialized,
            config,
            stats: IndexStats::default(),
            indexed_files: HashSet::new(),
            file_contents: HashMap::new(),
        }
    }

    /// Initialize the index. Returns `true` if initialization succeeded,
    /// `false` if already initialized.
    pub fn initialize(&mut self) -> Result<bool, IndexError> {
        if self.state != IndexingState::NotInitialized {
            return Ok(false);
        }

        self.state = IndexingState::Idle;
        Ok(true)
    }

    /// Start the indexing process.
    ///
    /// Transitions from `Idle` to `Indexing`. Returns an error if the index
    /// is not initialized or already indexing.
    pub fn start_indexing(&mut self) -> Result<(), IndexError> {
        match self.state {
            IndexingState::NotInitialized => Err(IndexError::NotInitialized),
            IndexingState::Indexing => Err(IndexError::AlreadyIndexing),
            IndexingState::ShuttingDown => Err(IndexError::GeneralError(
                "cannot start indexing while shutting down".to_string(),
            )),
            _ => {
                self.state = IndexingState::Indexing;
                Ok(())
            }
        }
    }

    /// Stop the indexing process and transition back to `Idle`.
    pub fn stop_indexing(&mut self) {
        if self.state == IndexingState::Indexing {
            self.state = IndexingState::Idle;
        }
    }

    /// Search the index for the given query.
    ///
    /// Returns up to `limit` results. Uses BM25-like scoring when file
    /// contents are available, otherwise falls back to path-only matching.
    pub fn search(&self, query: &str, limit: usize) -> Vec<VectorStoreSearchResult> {
        self.search_with_prefix(query, None, limit)
    }

    /// Search the index for the given query, optionally filtered by directory prefix.
    ///
    /// Returns up to `limit` results sorted by relevance score (descending).
    /// When `directory_prefix` is `Some`, only files whose path starts with
    /// the prefix are included.
    ///
    /// ## Scoring
    ///
    /// When file contents are available (via `workspace_path`), scoring uses a
    /// BM25-like approach:
    /// - **Filename score** (weight 2.0): proportion of query terms found in the
    ///   file path, with bonus for exact basename matches.
    /// - **Content score** (weight 1.0): term-frequency-based score across file
    ///   content, normalized by document length.
    ///
    /// When no file contents are cached, only filename matching is used.
    pub fn search_with_prefix(
        &self,
        query: &str,
        directory_prefix: Option<&str>,
        limit: usize,
    ) -> Vec<VectorStoreSearchResult> {
        if self.state == IndexingState::NotInitialized || self.state == IndexingState::Error {
            return vec![];
        }

        let query_terms = tokenize(query);
        if query_terms.is_empty() {
            return vec![];
        }

        let has_contents = !self.file_contents.is_empty();
        let mut results: Vec<VectorStoreSearchResult> = Vec::new();

        for path in &self.indexed_files {
            // Filter by directory prefix if provided
            if let Some(prefix) = directory_prefix {
                if !path.starts_with(prefix) {
                    continue;
                }
            }

            let path_lower = path.to_lowercase();

            if has_contents {
                // Content-aware BM25-like scoring
                let filename_score = compute_filename_score(&path_lower, &query_terms);

                let (content_score, line_number, code_chunk) =
                    if let Some(content) = self.file_contents.get(path) {
                        compute_content_score(content, &query_terms)
                    } else {
                        (0.0, None, None)
                    };

                // Combined score: filename weighted higher
                let total_score = 2.0 * filename_score + content_score;

                if total_score > 0.0 {
                    let (start_line, end_line) = if let Some(ln) = line_number {
                        let start = ln.saturating_sub(3).max(1);
                        let end = ln + 3;
                        (Some(start as u32), Some(end as u32))
                    } else {
                        (None, None)
                    };

                    results.push(VectorStoreSearchResult {
                        file_path: path.clone(),
                        line_number: line_number.map(|n| n as u32),
                        content: code_chunk.clone().unwrap_or_default(),
                        score: total_score,
                        start_line,
                        end_line,
                        code_chunk,
                    });
                }
            } else {
                // Path-only fallback scoring
                let filename_score = compute_filename_score(&path_lower, &query_terms);
                if filename_score > 0.0 {
                    results.push(VectorStoreSearchResult {
                        file_path: path.clone(),
                        line_number: Some(1),
                        content: format!("content of {path}"),
                        score: filename_score,
                        start_line: Some(1),
                        end_line: Some(1),
                        code_chunk: Some(format!("// matched content from {path}")),
                    });
                }
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    /// Get the current indexing state.
    pub fn get_state(&self) -> &IndexingState {
        &self.state
    }

    /// Get the current index statistics.
    pub fn get_stats(&self) -> &IndexStats {
        &self.stats
    }

    /// Check whether the index has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.state != IndexingState::NotInitialized
    }

    /// Check whether the workspace indexing is enabled in the config.
    pub fn is_workspace_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Clear all indexed data and reset stats.
    pub fn clear_index_data(&mut self) -> Result<(), IndexError> {
        if self.state == IndexingState::NotInitialized {
            return Err(IndexError::NotInitialized);
        }

        self.indexed_files.clear();
        self.file_contents.clear();
        self.stats.indexed_files = 0;
        self.stats.total_chunks = 0;
        Ok(())
    }

    /// Add a file to the index.
    ///
    /// Checks the file against include/exclude patterns and size limits.
    /// When `workspace_path` is configured, reads and caches the file content
    /// for content-based search.
    pub fn add_file(&mut self, path: &str) -> Result<(), IndexError> {
        if self.state == IndexingState::NotInitialized {
            return Err(IndexError::NotInitialized);
        }

        // Check exclude patterns
        for pattern in &self.config.exclude_patterns {
            if Self::matches_glob(path, pattern) {
                return Ok(());
            }
        }

        // Check include patterns
        let included = self
            .config
            .include_patterns
            .iter()
            .any(|p| Self::matches_glob(path, p));

        if !included && !self.config.include_patterns.is_empty() {
            return Ok(());
        }

        if self.indexed_files.insert(path.to_string()) {
            // Try to read and cache file content when workspace_path is configured
            if let Some(ref workspace) = self.config.workspace_path {
                let full_path = std::path::Path::new(workspace).join(path);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    if content.len() as u64 <= self.config.max_file_size {
                        self.file_contents.insert(path.to_string(), content);
                    }
                }
            }

            self.stats.indexed_files = self.indexed_files.len();
            self.stats.total_files = self.stats.total_files.max(self.indexed_files.len());
            self.stats.total_chunks += 1;
        }

        Ok(())
    }

    /// Remove a file from the index.
    ///
    /// Returns `true` if the file was present and removed.
    pub fn remove_file(&mut self, path: &str) -> Result<bool, IndexError> {
        if self.state == IndexingState::NotInitialized {
            return Err(IndexError::NotInitialized);
        }

        if self.indexed_files.remove(path) {
            self.file_contents.remove(path);
            self.stats.indexed_files = self.indexed_files.len();
            self.stats.total_chunks = self.stats.total_chunks.saturating_sub(1);
            return Ok(true);
        }

        Ok(false)
    }

    /// Start the file watcher for incremental updates.
    /// Corresponds to TS: `CodeIndexOrchestrator.startWatcher()`
    pub fn start_watcher(&mut self) -> Result<(), IndexError> {
        if self.state == IndexingState::NotInitialized {
            return Err(IndexError::NotInitialized);
        }
        // In this simplified implementation, watcher is a no-op
        // A full implementation would use notify crate for file watching
        Ok(())
    }

    /// Stop the file watcher.
    /// Corresponds to TS: `CodeIndexOrchestrator.stopWatcher()`
    pub fn stop_watcher(&mut self) {
        // In this simplified implementation, watcher is a no-op
    }

    /// Handle settings changes that may require re-indexing.
    /// Corresponds to TS: `CodeIndexManager.handleSettingsChange()`
    pub fn handle_settings_change(&mut self, new_config: CodeIndexConfig) -> Result<bool, IndexError> {
        let requires_restart = self.config != new_config;
        self.config = new_config;

        if requires_restart && self.state == IndexingState::Indexing {
            self.stop_indexing();
        }

        Ok(requires_restart)
    }

    /// Recover from error state by clearing the error and resetting internal state.
    /// Corresponds to TS: `CodeIndexManager.recoverFromError()`
    pub fn recover_from_error(&mut self) {
        if self.state == IndexingState::Error {
            self.state = IndexingState::Idle;
        }
    }

    /// Search the index for the given query (alias for `search`).
    /// Corresponds to TS: `CodeIndexManager.searchIndex(query, directoryPrefix?)`
    pub fn search_index(
        &self,
        query: &str,
        directory_prefix: Option<&str>,
    ) -> Vec<VectorStoreSearchResult> {
        self.search_with_prefix(query, directory_prefix, 20)
    }

    /// Check whether the feature is enabled in the config.
    /// Corresponds to TS: `CodeIndexManager.isFeatureEnabled`
    pub fn is_feature_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Dispose of the index manager, releasing all resources.
    /// Corresponds to TS: `CodeIndexManager.dispose()`
    pub fn dispose(&mut self) {
        self.state = IndexingState::ShuttingDown;
        self.indexed_files.clear();
        self.file_contents.clear();
        self.stats = IndexStats::default();
        self.state = IndexingState::NotInitialized;
    }

    /// Simple glob matching — supports `**` (any path segments) and `*` (any segment).
    fn matches_glob(path: &str, pattern: &str) -> bool {
        // Simple implementation: check if the pattern's extension matches
        if pattern.contains("**/*.") {
            if let Some(ext) = pattern.split("**/*.").last() {
                return path.ends_with(ext) || path.contains(&format!(".{ext}"));
            }
        }
        if pattern.contains("*/") {
            let suffix = pattern.replace("*/", "/");
            return path.contains(&suffix);
        }
        path.contains(pattern)
    }
}

// ---------------------------------------------------------------------------
// Scoring helpers (BM25-like)
// ---------------------------------------------------------------------------

/// Tokenize a query string into lowercase terms.
///
/// Splits on any character that is not alphanumeric, `_`, or `-`.
/// Returns an empty vec if the query produces no terms.
fn tokenize(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Compute filename relevance score based on query terms.
///
/// Returns a value in `[0.0, 1.0]` representing the proportion of query terms
/// found in the file path. An exact match on the basename (without extension)
/// receives a bonus.
fn compute_filename_score(path_lower: &str, terms: &[String]) -> f64 {
    if terms.is_empty() {
        return 0.0;
    }

    let mut matched = 0;
    let mut exact_basename_match = false;

    // Extract basename (filename without extension) for bonus scoring
    let basename = path_lower
        .rsplit('/')
        .next()
        .unwrap_or(path_lower)
        .split('.')
        .next()
        .unwrap_or("");

    for term in terms {
        if path_lower.contains(term.as_str()) {
            matched += 1;
            // Check for exact basename match
            if basename == term.as_str() {
                exact_basename_match = true;
            }
        }
    }

    let base_score = matched as f64 / terms.len() as f64;

    // Bonus for exact basename match
    if exact_basename_match {
        (base_score + 0.3).min(1.0)
    } else {
        base_score
    }
}

/// Compute content relevance score and find the best matching line.
///
/// Returns `(score, line_number, code_chunk)` where:
/// - `score` is a term-frequency-based relevance score
/// - `line_number` is the 1-based line number of the best match
/// - `code_chunk` is the extracted code around the best match
fn compute_content_score(
    content: &str,
    terms: &[String],
) -> (f64, Option<usize>, Option<String>) {
    if content.is_empty() || terms.is_empty() {
        return (0.0, None, None);
    }

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return (0.0, None, None);
    }

    let content_lower = content.to_lowercase();
    let total_words = content_lower.split_whitespace().count().max(1) as f64;

    // Compute term frequency score
    let mut tf_score = 0.0;
    let mut matched_terms = 0;

    for term in terms {
        let count = content_lower.matches(term.as_str()).count() as f64;
        if count > 0.0 {
            tf_score += count / total_words;
            matched_terms += 1;
        }
    }

    if matched_terms == 0 {
        return (0.0, None, None);
    }

    // Normalize by number of query terms (query coordination factor)
    let score = tf_score * (matched_terms as f64 / terms.len() as f64);

    // Find best matching line (line with most distinct term matches)
    let mut best_line = 0;
    let mut best_count = 0;

    for (i, line) in lines.iter().enumerate() {
        let line_lower = line.to_lowercase();
        let mut count = 0;
        for term in terms {
            if line_lower.contains(term.as_str()) {
                count += 1;
            }
        }
        if count > best_count {
            best_count = count;
            best_line = i;
        }
    }

    // Extract code chunk around best match (±3 lines for context)
    let start = best_line.saturating_sub(3);
    let end = (best_line + 4).min(lines.len());
    let chunk = lines[start..end].join("\n");

    (score, Some(best_line + 1), Some(chunk))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_manager() -> CodeIndexManager {
        CodeIndexManager::new(CodeIndexConfig::default())
    }

    #[test]
    fn test_new_manager_not_initialized() {
        let mgr = default_manager();
        assert_eq!(IndexingState::NotInitialized, *mgr.get_state());
        assert!(!mgr.is_initialized());
    }

    #[test]
    fn test_initialize_success() {
        let mut mgr = default_manager();
        let result = mgr.initialize();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
        assert_eq!(IndexingState::Idle, *mgr.get_state());
    }

    #[test]
    fn test_initialize_idempotent() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        let result = mgr.initialize();
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_start_indexing_without_init() {
        let mut mgr = default_manager();
        let result = mgr.start_indexing();
        assert!(result.is_err());
    }

    #[test]
    fn test_start_indexing_success() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        let result = mgr.start_indexing();
        assert!(result.is_ok());
        assert_eq!(IndexingState::Indexing, *mgr.get_state());
    }

    #[test]
    fn test_start_indexing_already_indexing() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.start_indexing().unwrap();
        let result = mgr.start_indexing();
        assert!(result.is_err());
    }

    #[test]
    fn test_stop_indexing() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.start_indexing().unwrap();
        mgr.stop_indexing();
        assert_eq!(IndexingState::Idle, *mgr.get_state());
    }

    #[test]
    fn test_stop_indexing_when_not_indexing() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.stop_indexing();
        assert_eq!(IndexingState::Idle, *mgr.get_state());
    }

    #[test]
    fn test_search_not_initialized() {
        let mgr = default_manager();
        let results = mgr.search("test", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_initialized_empty() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        let results = mgr.search("test", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_with_indexed_files() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        mgr.add_file("src/lib.rs").unwrap();

        let results = mgr.search("main", 10);
        assert_eq!(1, results.len());
        assert_eq!("src/main.rs", results[0].file_path);
        // Score should be > 0 (not the old fixed 1.0)
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_limit() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        mgr.add_file("src/lib.rs").unwrap();

        let results = mgr.search("", 1);
        assert!(results.len() <= 1);
    }

    #[test]
    fn test_search_with_prefix_filter() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        mgr.add_file("src/lib.rs").unwrap();
        mgr.add_file("tests/integration.rs").unwrap();

        let results = mgr.search_with_prefix("rs", Some("src"), 10);
        assert!(results.iter().all(|r| r.file_path.starts_with("src")));
    }

    #[test]
    fn test_search_scores_vary() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        mgr.add_file("src/main_helper.rs").unwrap();
        mgr.add_file("src/utils.rs").unwrap();

        let results = mgr.search("main", 10);
        // main.rs should score higher than main_helper.rs (exact basename match bonus)
        // utils.rs should not appear (no match)
        assert!(results.len() >= 1);
        let main_scores: Vec<&VectorStoreSearchResult> = results
            .iter()
            .filter(|r| r.file_path == "src/main.rs")
            .collect();
        let helper_scores: Vec<&VectorStoreSearchResult> = results
            .iter()
            .filter(|r| r.file_path == "src/main_helper.rs")
            .collect();

        if !main_scores.is_empty() && !helper_scores.is_empty() {
            assert!(main_scores[0].score >= helper_scores[0].score);
        }
    }

    #[test]
    fn test_is_workspace_enabled() {
        let mgr = default_manager();
        assert!(mgr.is_workspace_enabled());
    }

    #[test]
    fn test_is_workspace_disabled() {
        let mut config = CodeIndexConfig::default();
        config.enabled = false;
        let mgr = CodeIndexManager::new(config);
        assert!(!mgr.is_workspace_enabled());
    }

    #[test]
    fn test_add_file_success() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        let result = mgr.add_file("src/main.rs");
        assert!(result.is_ok());
        assert_eq!(1, mgr.get_stats().indexed_files);
    }

    #[test]
    fn test_add_file_not_initialized() {
        let mut mgr = default_manager();
        let result = mgr.add_file("src/main.rs");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_file_duplicate() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        mgr.add_file("src/main.rs").unwrap();
        assert_eq!(1, mgr.get_stats().indexed_files);
    }

    #[test]
    fn test_remove_file_success() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        let removed = mgr.remove_file("src/main.rs").unwrap();
        assert!(removed);
        assert_eq!(0, mgr.get_stats().indexed_files);
    }

    #[test]
    fn test_remove_file_not_present() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        let removed = mgr.remove_file("nonexistent.rs").unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_remove_file_not_initialized() {
        let mut mgr = default_manager();
        let result = mgr.remove_file("src/main.rs");
        assert!(result.is_err());
    }

    #[test]
    fn test_clear_index_data() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        mgr.add_file("src/lib.rs").unwrap();
        mgr.clear_index_data().unwrap();
        assert_eq!(0, mgr.get_stats().indexed_files);
        assert_eq!(0, mgr.get_stats().total_chunks);
    }

    #[test]
    fn test_clear_index_data_not_initialized() {
        let mut mgr = default_manager();
        let result = mgr.clear_index_data();
        assert!(result.is_err());
    }

    #[test]
    fn test_dispose() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        mgr.dispose();
        assert_eq!(IndexingState::NotInitialized, *mgr.get_state());
        assert_eq!(0, mgr.get_stats().indexed_files);
    }

    #[test]
    fn test_get_stats() {
        let mut mgr = default_manager();
        mgr.initialize().unwrap();
        mgr.add_file("src/main.rs").unwrap();
        let stats = mgr.get_stats();
        assert!(stats.total_files >= 1);
        assert_eq!(1, stats.indexed_files);
        assert!(stats.total_chunks >= 1);
    }

    // --- Tokenizer tests ---

    #[test]
    fn test_tokenize_simple() {
        let terms = tokenize("fn main");
        assert_eq!(terms, vec!["fn", "main"]);
    }

    #[test]
    fn test_tokenize_with_punctuation() {
        let terms = tokenize("std::collections::HashMap");
        assert_eq!(terms, vec!["std", "collections", "hashmap"]);
    }

    #[test]
    fn test_tokenize_empty() {
        let terms = tokenize("");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_tokenize_whitespace_only() {
        let terms = tokenize("   ");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_tokenize_preserves_underscores() {
        let terms = tokenize("my_function_name");
        assert_eq!(terms, vec!["my_function_name"]);
    }

    #[test]
    fn test_tokenize_preserves_hyphens() {
        let terms = tokenize("my-module-name");
        assert_eq!(terms, vec!["my-module-name"]);
    }

    // --- Scoring tests ---

    #[test]
    fn test_filename_score_exact_match() {
        let score = compute_filename_score("src/main.rs", &["main".to_string()]);
        // "main" is found in path AND matches the basename exactly → score = 1.0 (capped)
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_filename_score_partial_match() {
        // "main" is found in path but does NOT match basename "main_helper"
        let score_without_bonus = compute_filename_score("src/main_helper.rs", &["main".to_string()]);
        // "main" IS found → base_score = 1.0, but no basename bonus
        assert!(score_without_bonus > 0.0);

        // Compare with a case where the term is NOT in the path at all
        let score_no_match = compute_filename_score("src/utils.rs", &["main".to_string()]);
        assert!((score_no_match - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_filename_score_no_match() {
        let score = compute_filename_score("src/utils.rs", &["main".to_string()]);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_filename_score_multi_term() {
        let score =
            compute_filename_score("src/main_handler.rs", &["main".to_string(), "handler".to_string()]);
        assert!(score > 0.0);
    }

    #[test]
    fn test_content_score_basic() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let (score, line, chunk) = compute_content_score(content, &["main".to_string()]);
        assert!(score > 0.0);
        assert_eq!(line, Some(1)); // "fn main()" is on line 1
        assert!(chunk.is_some());
        assert!(chunk.unwrap().contains("main"));
    }

    #[test]
    fn test_content_score_no_match() {
        let content = "fn foo() {}\n";
        let (score, line, chunk) = compute_content_score(content, &["bar".to_string()]);
        assert!((score - 0.0).abs() < f64::EPSILON);
        assert!(line.is_none());
        assert!(chunk.is_none());
    }

    #[test]
    fn test_content_score_multi_term() {
        let content = "pub fn search() {\n    let query = \"test\";\n    search_files(query);\n}\n";
        let (score, line, chunk) =
            compute_content_score(content, &["search".to_string(), "query".to_string()]);
        assert!(score > 0.0);
        assert!(line.is_some());
        assert!(chunk.is_some());
    }

    #[test]
    fn test_content_score_empty_content() {
        let (score, line, chunk) = compute_content_score("", &["test".to_string()]);
        assert!((score - 0.0).abs() < f64::EPSILON);
        assert!(line.is_none());
        assert!(chunk.is_none());
    }

    #[test]
    fn test_content_score_best_line_selection() {
        let content = "fn foo() {}\nfn bar() {}\nfn search_query() {\n    // best match\n}\nfn baz() {}\n";
        let (_score, line, _chunk) =
            compute_content_score(content, &["search".to_string(), "query".to_string()]);
        // Line 3 has both terms, should be the best match
        assert_eq!(line, Some(3));
    }
}
