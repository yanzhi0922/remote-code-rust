//! Orchestrator for code indexing workflow.
//!
//! Corresponds to `orchestrator.ts` in the TypeScript source.
//!
//! Manages the code indexing workflow, coordinating between different
//! services and managers for file scanning, embedding, and indexing.

use crate::cache_manager::CacheManager;
use crate::config_manager::CodeIndexConfigManager;
use crate::processor::{BatchProcessingSummary, CodeParser, FileProcessor};
use crate::search_service::VectorStore;
use crate::state_manager::{CodeIndexStateManager, IndexingState};
use crate::types::IndexError;

/// Manages the code indexing workflow.
///
/// Corresponds to `CodeIndexOrchestrator` in `orchestrator.ts`.
pub struct CodeIndexOrchestrator {
    config_manager: CodeIndexConfigManager,
    state_manager: CodeIndexStateManager,
    workspace_path: String,
    cache_manager: CacheManager,
    vector_store: Box<dyn VectorStore>,
    processor: FileProcessor,
    is_processing: bool,
}

impl CodeIndexOrchestrator {
    /// Creates a new orchestrator.
    pub fn new(
        config_manager: CodeIndexConfigManager,
        state_manager: CodeIndexStateManager,
        workspace_path: String,
        cache_manager: CacheManager,
        vector_store: Box<dyn VectorStore>,
        parser: Option<Box<dyn CodeParser>>,
    ) -> Self {
        let processor = match parser {
            Some(p) => FileProcessor::new(p),
            None => FileProcessor::with_default_parser(),
        };

        Self {
            config_manager,
            state_manager,
            workspace_path,
            cache_manager,
            vector_store,
            processor,
            is_processing: false,
        }
    }

    /// Returns the current indexing state.
    pub fn state(&self) -> IndexingState {
        self.state_manager.state()
    }

    /// Starts the indexing process.
    ///
    /// Corresponds to `startIndexing` in `orchestrator.ts`.
    pub fn start_indexing(&mut self) -> Result<(), IndexError> {
        if !self.config_manager.is_feature_configured() {
            self.state_manager.set_system_state(
                IndexingState::Standby,
                Some("Missing configuration. Save your settings to start indexing."),
            );
            return Ok(());
        }

        if self.is_processing
            || !matches!(
                self.state_manager.state(),
                IndexingState::Standby | IndexingState::Error | IndexingState::Indexed
            )
        {
            return Ok(());
        }

        self.is_processing = true;
        self.state_manager.set_system_state(IndexingState::Indexing, Some("Initializing services..."));

        // Initialize vector store
        let collection_created = self.vector_store.initialize()?;

        if collection_created {
            if let Err(e) = self.cache_manager.clear_cache_file() {
                // Log but don't fail
                eprintln!("Warning: Failed to clear cache: {}", e);
            }
        }

        // Check for existing data
        let has_existing_data = self.vector_store.has_indexed_data().unwrap_or(false);

        if has_existing_data && !collection_created {
            self.state_manager.set_system_state(
                IndexingState::Indexing,
                Some("Checking for new or modified files..."),
            );
        }

        self.state_manager.set_system_state(
            IndexingState::Indexed,
            Some("Index up-to-date."),
        );

        self.is_processing = false;
        Ok(())
    }

    /// Stops the indexing process.
    pub fn stop_indexing(&mut self) {
        if self.is_processing {
            self.state_manager.set_system_state(IndexingState::Stopping, Some("Stopping..."));
            self.is_processing = false;
            self.state_manager.set_system_state(IndexingState::Standby, Some("Indexing stopped."));
        }
    }

    /// Indexes a single file.
    pub fn index_file(&mut self, file_path: &str, content: &str) -> Result<(), IndexError> {
        // Check if file has changed
        if !self.cache_manager.has_file_changed(file_path, content.as_bytes()) {
            return Ok(());
        }

        // Process the file
        let result = self.processor.process_file(file_path, content)?;

        // Update cache
        self.cache_manager.update_hash(file_path, &result.content_hash);

        // Index the blocks
        if !result.blocks.is_empty() {
            let mut ids = Vec::new();
            let mut payloads = Vec::new();

            for block in &result.blocks {
                ids.push(format!("{}:{}:{}", block.file_path, block.start_line, block.end_line));
                payloads.push(serde_json::json!({
                    "file_path": block.file_path,
                    "content": block.content,
                    "start_line": block.start_line,
                    "end_line": block.end_line,
                    "language": block.language,
                }));
            }

            // For now, use zero vectors since we don't have a real embedder
            let dimension = 1536;
            let vectors: Vec<Vec<f64>> = ids.iter().map(|_| vec![0.0; dimension]).collect();

            self.vector_store.upsert(&ids, &vectors, &payloads)?;
        }

        Ok(())
    }

    /// Indexes multiple files in batch.
    pub fn index_batch(&mut self, files: &[(&str, &str)]) -> BatchProcessingSummary {
        let mut summary = BatchProcessingSummary::default();

        for (path, content) in files {
            match self.index_file(path, content) {
                Ok(()) => {
                    summary.processed_files.push(
                        crate::processor::FileProcessingResult {
                            file_path: path.to_string(),
                            blocks: vec![],
                            content_hash: CacheManager::compute_hash(content.as_bytes()),
                        },
                    );
                }
                Err(e) => {
                    summary.errors.push(format!("{}: {}", path, e));
                }
            }
        }

        summary
    }

    /// Returns whether the orchestrator is currently processing.
    pub fn is_processing(&self) -> bool {
        self.is_processing
    }

    /// Returns the workspace path.
    pub fn workspace_path(&self) -> &str {
        &self.workspace_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_manager::{CodeIndexFullConfig, SecretsStore};
    use crate::search_service::InMemoryVectorStore;

    fn create_test_orchestrator() -> CodeIndexOrchestrator {
        let mut raw_config = CodeIndexFullConfig::default();
        raw_config.codebase_index_enabled = true;
        let mut secrets = SecretsStore::default();
        secrets.open_ai_key = "test-key".to_string();

        let config_manager = CodeIndexConfigManager::with_config(raw_config, secrets);
        let state_manager = CodeIndexStateManager::new();

        let cache_dir = std::env::temp_dir().join("roo-test-orchestrator");
        let _ = std::fs::remove_dir_all(&cache_dir);
        let mut cache_manager = CacheManager::new(&cache_dir, "/test/workspace");
        let _ = cache_manager.initialize();

        let vector_store = InMemoryVectorStore::new(1536);

        CodeIndexOrchestrator::new(
            config_manager,
            state_manager,
            "/test/workspace".to_string(),
            cache_manager,
            Box::new(vector_store),
            None,
        )
    }

    #[test]
    fn test_initial_state() {
        let orch = create_test_orchestrator();
        assert_eq!(orch.state(), IndexingState::Standby);
        assert!(!orch.is_processing());
    }

    #[test]
    fn test_start_indexing() {
        let mut orch = create_test_orchestrator();
        orch.start_indexing().unwrap();
        assert_eq!(orch.state(), IndexingState::Indexed);
    }

    #[test]
    fn test_index_file() {
        let mut orch = create_test_orchestrator();
        let content = "fn main() {\n    println!(\"hello\");\n    println!(\"world\");\n}";
        orch.index_file("test.rs", content).unwrap();
    }

    #[test]
    fn test_index_batch() {
        let mut orch = create_test_orchestrator();
        let files = vec![
            ("a.rs", "fn a() {\n    x();\n    y();\n}"),
            ("b.rs", "fn b() {\n    z();\n    w();\n}"),
        ];
        let summary = orch.index_batch(&files);
        assert_eq!(summary.processed_files.len(), 2);
        assert!(summary.errors.is_empty());
    }

    #[test]
    fn test_workspace_path() {
        let orch = create_test_orchestrator();
        assert_eq!(orch.workspace_path(), "/test/workspace");
    }
}
