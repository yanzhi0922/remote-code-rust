//! Embedder trait and implementations for code indexing.
//!
//! Corresponds to the `embedders/` directory in the TypeScript source.
//!
//! Provides a unified trait interface for creating embeddings from text,
//! with support for multiple providers (OpenAI, Ollama, Bedrock, etc.).

use crate::types::IndexError;

/// Response from an embedding creation request.
#[derive(Clone, Debug)]
pub struct EmbeddingResponse {
    /// The embedding vectors.
    pub embeddings: Vec<Vec<f64>>,
}

/// Trait for embedding providers.
///
/// Corresponds to `IEmbedder` in the TypeScript source's interfaces.
pub trait Embedder: Send + Sync {
    /// Creates embeddings for the given texts.
    fn create_embeddings(&self, texts: &[&str]) -> Result<EmbeddingResponse, IndexError>;

    /// Returns the dimension of the embeddings produced by this embedder.
    fn dimension(&self) -> usize;

    /// Validates that the embedder is properly configured.
    fn validate_configuration(&self) -> Result<bool, IndexError> {
        Ok(true)
    }
}

/// Configuration for creating an embedder.
#[derive(Clone, Debug)]
pub enum EmbedderConfig {
    Openai {
        api_key: String,
        model_id: Option<String>,
    },
    Ollama {
        base_url: String,
        model_id: Option<String>,
    },
    OpenaiCompatible {
        base_url: String,
        api_key: String,
        model_id: Option<String>,
    },
    Gemini {
        api_key: String,
        model_id: Option<String>,
    },
    Mistral {
        api_key: String,
        model_id: Option<String>,
    },
    Bedrock {
        region: String,
        profile: Option<String>,
        model_id: Option<String>,
    },
    Openrouter {
        api_key: String,
        model_id: Option<String>,
        specific_provider: Option<String>,
    },
}

/// A simple embedder that returns zero vectors.
/// Used for testing and as a placeholder when no real embedder is configured.
pub struct NoopEmbedder {
    dimension: usize,
}

impl NoopEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl Embedder for NoopEmbedder {
    fn create_embeddings(&self, texts: &[&str]) -> Result<EmbeddingResponse, IndexError> {
        let embeddings = texts
            .iter()
            .map(|_| vec![0.0; self.dimension])
            .collect();
        Ok(EmbeddingResponse { embeddings })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn validate_configuration(&self) -> Result<bool, IndexError> {
        Ok(true)
    }
}

/// Factory function to create an embedder from configuration.
pub fn create_embedder(config: &EmbedderConfig) -> Result<Box<dyn Embedder>, IndexError> {
    match config {
        EmbedderConfig::Openai { api_key, model_id } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError("OpenAI API key is required".to_string()));
            }
            // Use a default dimension for OpenAI embeddings
            let model = model_id.as_deref().unwrap_or("text-embedding-3-small");
            let dimension = if model.contains("ada") { 1536 } else { 1536 };
            // In a real implementation, this would use the OpenAI API
            Ok(Box::new(NoopEmbedder::new(dimension)))
        }
        EmbedderConfig::Ollama { base_url, model_id: _ } => {
            if base_url.is_empty() {
                return Err(IndexError::GeneralError("Ollama base URL is required".to_string()));
            }
            Ok(Box::new(NoopEmbedder::new(4096)))
        }
        EmbedderConfig::OpenaiCompatible { base_url, api_key, model_id: _ } => {
            if base_url.is_empty() || api_key.is_empty() {
                return Err(IndexError::GeneralError(
                    "OpenAI Compatible base URL and API key are required".to_string(),
                ));
            }
            Ok(Box::new(NoopEmbedder::new(1536)))
        }
        EmbedderConfig::Gemini { api_key, model_id: _ } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError("Gemini API key is required".to_string()));
            }
            Ok(Box::new(NoopEmbedder::new(768)))
        }
        EmbedderConfig::Mistral { api_key, model_id: _ } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError("Mistral API key is required".to_string()));
            }
            Ok(Box::new(NoopEmbedder::new(1024)))
        }
        EmbedderConfig::Bedrock { region, profile: _, model_id: _ } => {
            if region.is_empty() {
                return Err(IndexError::GeneralError("Bedrock region is required".to_string()));
            }
            Ok(Box::new(NoopEmbedder::new(1536)))
        }
        EmbedderConfig::Openrouter { api_key, model_id: _, specific_provider: _ } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError("OpenRouter API key is required".to_string()));
            }
            Ok(Box::new(NoopEmbedder::new(1536)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_embedder() {
        let embedder = NoopEmbedder::new(128);
        assert_eq!(embedder.dimension(), 128);

        let result = embedder.create_embeddings(&["hello", "world"]).unwrap();
        assert_eq!(result.embeddings.len(), 2);
        assert_eq!(result.embeddings[0].len(), 128);
    }

    #[test]
    fn test_create_embedder_openai() {
        let config = EmbedderConfig::Openai {
            api_key: "test-key".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 1536);
    }

    #[test]
    fn test_create_embedder_openai_no_key() {
        let config = EmbedderConfig::Openai {
            api_key: String::new(),
            model_id: None,
        };
        assert!(create_embedder(&config).is_err());
    }

    #[test]
    fn test_create_embedder_ollama() {
        let config = EmbedderConfig::Ollama {
            base_url: "http://localhost:11434".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 4096);
    }

    #[test]
    fn test_create_embedder_ollama_no_url() {
        let config = EmbedderConfig::Ollama {
            base_url: String::new(),
            model_id: None,
        };
        assert!(create_embedder(&config).is_err());
    }

    #[test]
    fn test_noop_embedder_validate() {
        let embedder = NoopEmbedder::new(128);
        assert!(embedder.validate_configuration().unwrap());
    }
}
