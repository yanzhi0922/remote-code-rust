//! Service factory for code indexing.
//!
//! Corresponds to `service-factory.ts` in the TypeScript source.
//!
//! Factory class responsible for creating and configuring code indexing
//! service dependencies (embedders, vector stores, scanners, watchers).

use crate::cache_manager::CacheManager;
use crate::config_manager::{CodeIndexConfigManager, EmbedderProvider};
use crate::embedder::{self, Embedder, EmbedderConfig};
use crate::processor::{CodeParser, FileProcessor};
use crate::search_service::{CodeIndexSearchService, InMemoryVectorStore, VectorStore};
use crate::state_manager::CodeIndexStateManager;
use crate::types::IndexError;

/// Default batch segment threshold for embedding operations.
pub const BATCH_SEGMENT_THRESHOLD: usize = 100;

/// Factory for creating code indexing services.
///
/// Corresponds to `CodeIndexServiceFactory` in `service-factory.ts`.
pub struct CodeIndexServiceFactory {
    config_manager: CodeIndexConfigManager,
    workspace_path: String,
    #[allow(dead_code)]
    cache_manager: CacheManager,
}

impl CodeIndexServiceFactory {
    /// Creates a new service factory.
    pub fn new(
        config_manager: CodeIndexConfigManager,
        workspace_path: String,
        cache_manager: CacheManager,
    ) -> Self {
        Self {
            config_manager,
            workspace_path,
            cache_manager,
        }
    }

    /// Creates an embedder based on the current configuration.
    pub fn create_embedder(&self) -> Result<Box<dyn Embedder>, IndexError> {
        let config = self.config_manager.get_config();

        let embedder_config = match config.embedder_provider {
            EmbedderProvider::Openai => EmbedderConfig::Openai {
                api_key: config
                    .open_ai_options
                    .as_ref()
                    .and_then(|o| o.open_ai_native_api_key.clone())
                    .ok_or_else(|| IndexError::GeneralError("OpenAI API key is required".to_string()))?,
                model_id: config.model_id.clone(),
            },
            EmbedderProvider::Ollama => EmbedderConfig::Ollama {
                base_url: config
                    .ollama_options
                    .as_ref()
                    .and_then(|o| o.ollama_base_url.clone())
                    .ok_or_else(|| IndexError::GeneralError("Ollama base URL is required".to_string()))?,
                model_id: config.model_id.clone(),
            },
            EmbedderProvider::OpenaiCompatible => {
                let opts = config
                    .open_ai_compatible_options
                    .as_ref()
                    .ok_or_else(|| {
                        IndexError::GeneralError("OpenAI Compatible configuration is required".to_string())
                    })?;
                EmbedderConfig::OpenaiCompatible {
                    base_url: opts.base_url.clone(),
                    api_key: opts.api_key.clone(),
                    model_id: config.model_id.clone(),
                }
            }
            EmbedderProvider::Gemini => EmbedderConfig::Gemini {
                api_key: config
                    .gemini_api_key
                    .clone()
                    .ok_or_else(|| IndexError::GeneralError("Gemini API key is required".to_string()))?,
                model_id: config.model_id.clone(),
            },
            EmbedderProvider::Mistral => EmbedderConfig::Mistral {
                api_key: config
                    .mistral_api_key
                    .clone()
                    .ok_or_else(|| IndexError::GeneralError("Mistral API key is required".to_string()))?,
                model_id: config.model_id.clone(),
            },
            EmbedderProvider::Bedrock => {
                let opts = config
                    .bedrock_options
                    .as_ref()
                    .ok_or_else(|| IndexError::GeneralError("Bedrock configuration is required".to_string()))?;
                EmbedderConfig::Bedrock {
                    region: opts.region.clone(),
                    profile: opts.profile.clone(),
                    model_id: config.model_id.clone(),
                }
            }
            EmbedderProvider::Openrouter => EmbedderConfig::Openrouter {
                api_key: config
                    .open_router_options
                    .as_ref()
                    .map(|o| o.api_key.clone())
                    .ok_or_else(|| IndexError::GeneralError("OpenRouter API key is required".to_string()))?,
                model_id: config.model_id.clone(),
                specific_provider: config
                    .open_router_options
                    .as_ref()
                    .and_then(|o| o.specific_provider.clone()),
            },
            EmbedderProvider::VercelAiGateway => {
                // VercelAiGateway uses OpenAI-compatible endpoint
                let api_key = config
                    .vercel_ai_gateway_api_key
                    .clone()
                    .ok_or_else(|| IndexError::GeneralError("Vercel AI Gateway API key is required".to_string()))?;
                EmbedderConfig::OpenaiCompatible {
                    base_url: "https://ai-gateway.vercel.com/v1".to_string(),
                    api_key,
                    model_id: config.model_id.clone(),
                }
            }
        };

        embedder::create_embedder(&embedder_config)
    }

    /// Creates a vector store instance.
    pub fn create_vector_store(&self) -> Result<Box<dyn VectorStore>, IndexError> {
        let config = self.config_manager.get_config();
        let dimension = config.model_dimension.unwrap_or(1536) as usize;
        Ok(Box::new(InMemoryVectorStore::new(dimension)))
    }

    /// Creates a file processor with the default parser.
    pub fn create_file_processor(&self) -> FileProcessor {
        FileProcessor::with_default_parser()
    }

    /// Creates a file processor with a custom parser.
    pub fn create_file_processor_with_parser(&self, parser: Box<dyn CodeParser>) -> FileProcessor {
        FileProcessor::new(parser)
    }

    /// Creates a complete search service.
    pub fn create_search_service(
        &self,
        state_manager: CodeIndexStateManager,
    ) -> Result<CodeIndexSearchService, IndexError> {
        let embedder = self.create_embedder()?;
        let vector_store = self.create_vector_store()?;

        Ok(CodeIndexSearchService::new(
            self.config_manager.clone_config(),
            state_manager,
            embedder,
            vector_store,
        ))
    }

    /// Returns the workspace path.
    pub fn workspace_path(&self) -> &str {
        &self.workspace_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache_manager::CacheManager;
    use crate::config_manager::{CodeIndexFullConfig, SecretsStore};

    fn create_test_factory() -> CodeIndexServiceFactory {
        let mut raw_config = CodeIndexFullConfig::default();
        raw_config.codebase_index_enabled = true;
        let mut secrets = SecretsStore::default();
        secrets.open_ai_key = "test-key".to_string();

        let config_manager = CodeIndexConfigManager::with_config(raw_config, secrets);
        let cache_dir = std::env::temp_dir().join("roo-test-factory");
        let _ = std::fs::remove_dir_all(&cache_dir);
        let mut cache_manager = CacheManager::new(&cache_dir, "/test/workspace");
        let _ = cache_manager.initialize();

        CodeIndexServiceFactory::new(
            config_manager,
            "/test/workspace".to_string(),
            cache_manager,
        )
    }

    #[test]
    fn test_create_embedder() {
        let factory = create_test_factory();
        let embedder = factory.create_embedder().unwrap();
        assert_eq!(embedder.dimension(), 1536);
    }

    #[test]
    fn test_create_vector_store() {
        let factory = create_test_factory();
        let store = factory.create_vector_store().unwrap();
        store.initialize().unwrap();
        assert!(!store.has_indexed_data().unwrap());
    }

    #[test]
    fn test_create_file_processor() {
        let factory = create_test_factory();
        let processor = factory.create_file_processor();
        let result = processor.process_file("test.rs", "fn main() {\n    x();\n    y();\n}").unwrap();
        assert_eq!(result.file_path, "test.rs");
    }

    #[test]
    fn test_workspace_path() {
        let factory = create_test_factory();
        assert_eq!(factory.workspace_path(), "/test/workspace");
    }
}
