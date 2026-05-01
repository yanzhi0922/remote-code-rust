//! Search service for code indexing.
//!
//! Corresponds to `search-service.ts` in the TypeScript source.
//!
//! Provides search functionality over the code index using vector similarity.

use crate::config_manager::CodeIndexConfigManager;
use crate::embedder::Embedder;
use crate::state_manager::{CodeIndexStateManager, IndexingState};
use crate::types::{IndexError, VectorStoreSearchResult};

/// Trait for vector store operations.
pub trait VectorStore: Send + Sync {
    /// Initialize the vector store (create collection if needed).
    fn initialize(&self) -> Result<bool, IndexError>;

    /// Search for similar vectors.
    fn search(
        &self,
        query_vector: &[f64],
        directory_prefix: Option<&str>,
        min_score: f64,
        max_results: usize,
    ) -> Result<Vec<VectorStoreSearchResult>, IndexError>;

    /// Check if the store has indexed data.
    fn has_indexed_data(&self) -> Result<bool, IndexError>;

    /// Upsert vectors into the store.
    fn upsert(
        &self,
        ids: &[String],
        vectors: &[Vec<f64>],
        payloads: &[serde_json::Value],
    ) -> Result<(), IndexError>;

    /// Delete vectors by file path prefix.
    fn delete_by_prefix(&self, prefix: &str) -> Result<(), IndexError>;
}

/// Service responsible for searching the code index.
///
/// Corresponds to `CodeIndexSearchService` in `search-service.ts`.
pub struct CodeIndexSearchService {
    config_manager: CodeIndexConfigManager,
    state_manager: CodeIndexStateManager,
    embedder: Box<dyn Embedder>,
    vector_store: Box<dyn VectorStore>,
}

impl CodeIndexSearchService {
    /// Creates a new search service.
    pub fn new(
        config_manager: CodeIndexConfigManager,
        state_manager: CodeIndexStateManager,
        embedder: Box<dyn Embedder>,
        vector_store: Box<dyn VectorStore>,
    ) -> Self {
        Self {
            config_manager,
            state_manager,
            embedder,
            vector_store,
        }
    }

    /// Searches the code index for relevant content.
    ///
    /// Corresponds to `searchIndex` in `search-service.ts`.
    pub fn search_index(
        &self,
        query: &str,
        directory_prefix: Option<&str>,
    ) -> Result<Vec<VectorStoreSearchResult>, IndexError> {
        if !self.config_manager.is_feature_enabled() || !self.config_manager.is_feature_configured() {
            return Err(IndexError::GeneralError(
                "Code index feature is disabled or not configured.".to_string(),
            ));
        }

        let min_score = self.config_manager.current_search_min_score();
        let max_results = self.config_manager.current_search_max_results();

        let current_state = self.state_manager.state();
        if current_state != IndexingState::Indexed && current_state != IndexingState::Indexing {
            return Err(IndexError::GeneralError(format!(
                "Code index is not ready for search. Current state: {}",
                current_state
            )));
        }

        // Generate embedding for query
        let embedding_response = self.embedder.create_embeddings(&[query])?;
        let vector = embedding_response
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| IndexError::GeneralError("Failed to generate embedding for query.".to_string()))?;

        // Normalize directory prefix
        let normalized_prefix = directory_prefix.map(|p| {
            p.replace('\\', "/").trim_end_matches('/').to_string()
        });

        // Perform search
        let results = self.vector_store.search(
            &vector,
            normalized_prefix.as_deref(),
            min_score,
            max_results,
        )?;

        Ok(results)
    }

    /// Returns a reference to the config manager.
    pub fn config_manager(&self) -> &CodeIndexConfigManager {
        &self.config_manager
    }

    /// Returns a reference to the state manager.
    pub fn state_manager(&self) -> &CodeIndexStateManager {
        &self.state_manager
    }
}

/// A simple in-memory vector store for testing.
pub struct InMemoryVectorStore {
    entries: std::sync::Mutex<Vec<VectorStoreEntry>>,
    #[allow(dead_code)]
    dimension: usize,
}

#[derive(Clone)]
struct VectorStoreEntry {
    #[allow(dead_code)]
    id: String,
    vector: Vec<f64>,
    payload: serde_json::Value,
}

impl InMemoryVectorStore {
    pub fn new(dimension: usize) -> Self {
        Self {
            entries: std::sync::Mutex::new(Vec::new()),
            dimension,
        }
    }

    /// Computes cosine similarity between two vectors.
    pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
        let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }
}

impl VectorStore for InMemoryVectorStore {
    fn initialize(&self) -> Result<bool, IndexError> {
        Ok(true)
    }

    fn search(
        &self,
        query_vector: &[f64],
        directory_prefix: Option<&str>,
        min_score: f64,
        max_results: usize,
    ) -> Result<Vec<VectorStoreSearchResult>, IndexError> {
        let entries = self.entries.lock().unwrap();

        let mut results: Vec<VectorStoreSearchResult> = entries
            .iter()
            .filter_map(|entry| {
                let score = Self::cosine_similarity(query_vector, &entry.vector);
                if score < min_score {
                    return None;
                }

                // Filter by directory prefix
                if let Some(prefix) = directory_prefix {
                    let file_path = entry.payload.get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !file_path.starts_with(prefix) {
                        return None;
                    }
                }

                Some(VectorStoreSearchResult {
                    file_path: entry.payload.get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    line_number: entry.payload.get("line_number")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    content: entry.payload.get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    score,
                    start_line: entry.payload.get("start_line")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    end_line: entry.payload.get("end_line")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    code_chunk: entry.payload.get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                })
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(max_results);

        Ok(results)
    }

    fn has_indexed_data(&self) -> Result<bool, IndexError> {
        Ok(!self.entries.lock().unwrap().is_empty())
    }

    fn upsert(
        &self,
        ids: &[String],
        vectors: &[Vec<f64>],
        payloads: &[serde_json::Value],
    ) -> Result<(), IndexError> {
        let mut entries = self.entries.lock().unwrap();
        for ((id, vector), payload) in ids.iter().zip(vectors.iter()).zip(payloads.iter()) {
            entries.push(VectorStoreEntry {
                id: id.clone(),
                vector: vector.clone(),
                payload: payload.clone(),
            });
        }
        Ok(())
    }

    fn delete_by_prefix(&self, prefix: &str) -> Result<(), IndexError> {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|e| {
            let file_path = e.payload.get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            !file_path.starts_with(prefix)
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = InMemoryVectorStore::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = InMemoryVectorStore::cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = InMemoryVectorStore::cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_in_memory_vector_store_upsert_and_search() {
        let store = InMemoryVectorStore::new(3);
        store.initialize().unwrap();

        store.upsert(
            &["id1".to_string()],
            &[vec![1.0, 0.0, 0.0]],
            &[serde_json::json!({
                "file_path": "src/main.rs",
                "content": "fn main() {}",
                "line_number": 1
            })],
        ).unwrap();

        assert!(store.has_indexed_data().unwrap());

        let results = store.search(&[1.0, 0.0, 0.0], None, 0.5, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "src/main.rs");
    }

    #[test]
    fn test_in_memory_vector_store_search_with_prefix() {
        let store = InMemoryVectorStore::new(3);
        store.initialize().unwrap();

        store.upsert(
            &["id1".to_string(), "id2".to_string()],
            &[vec![1.0, 0.0, 0.0], vec![0.9, 0.1, 0.0]],
            &[
                serde_json::json!({"file_path": "src/main.rs", "content": "main"}),
                serde_json::json!({"file_path": "lib/utils.rs", "content": "utils"}),
            ],
        ).unwrap();

        let results = store.search(&[1.0, 0.0, 0.0], Some("src/"), 0.0, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "src/main.rs");
    }

    #[test]
    fn test_in_memory_vector_store_delete_by_prefix() {
        let store = InMemoryVectorStore::new(3);
        store.initialize().unwrap();

        store.upsert(
            &["id1".to_string()],
            &[vec![1.0, 0.0, 0.0]],
            &[serde_json::json!({"file_path": "src/main.rs", "content": "main"})],
        ).unwrap();

        store.delete_by_prefix("src/").unwrap();
        assert!(!store.has_indexed_data().unwrap());
    }
}
