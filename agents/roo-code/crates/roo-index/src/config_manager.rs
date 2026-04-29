//! Configuration manager for code indexing.
//!
//! Corresponds to `config-manager.ts` in the TypeScript source.
//!
//! Manages configuration state and validation for the code indexing feature.

use serde::{Deserialize, Serialize};

/// Default minimum search score threshold.
pub const DEFAULT_SEARCH_MIN_SCORE: f64 = 0.4;
/// Default maximum number of search results.
pub const DEFAULT_MAX_SEARCH_RESULTS: usize = 50;

/// Configuration for the code index feature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeIndexFullConfig {
    /// Whether the code index feature is enabled globally.
    pub codebase_index_enabled: bool,
    /// Qdrant vector database URL.
    pub codebase_index_qdrant_url: String,
    /// The embedding provider to use.
    pub codebase_index_embedder_provider: String,
    /// Base URL for the embedder (used by Ollama).
    pub codebase_index_embedder_base_url: String,
    /// Model ID for the embedder.
    pub codebase_index_embedder_model_id: String,
    /// Minimum search score threshold.
    pub codebase_index_search_min_score: Option<f64>,
    /// Maximum number of search results.
    pub codebase_index_search_max_results: Option<usize>,
    /// Bedrock region.
    pub codebase_index_bedrock_region: String,
    /// Bedrock profile.
    pub codebase_index_bedrock_profile: String,
    /// OpenAI Compatible base URL.
    #[serde(default)]
    pub codebase_index_open_ai_compatible_base_url: Option<String>,
    /// OpenRouter specific provider.
    #[serde(default)]
    pub codebase_index_open_router_specific_provider: Option<String>,
    /// Model dimension override.
    #[serde(default)]
    pub codebase_index_embedder_model_dimension: Option<u64>,
}

impl Default for CodeIndexFullConfig {
    fn default() -> Self {
        Self {
            codebase_index_enabled: false,
            codebase_index_qdrant_url: "http://localhost:6333".to_string(),
            codebase_index_embedder_provider: "openai".to_string(),
            codebase_index_embedder_base_url: String::new(),
            codebase_index_embedder_model_id: String::new(),
            codebase_index_search_min_score: None,
            codebase_index_search_max_results: None,
            codebase_index_bedrock_region: "us-east-1".to_string(),
            codebase_index_bedrock_profile: String::new(),
            codebase_index_open_ai_compatible_base_url: None,
            codebase_index_open_router_specific_provider: None,
            codebase_index_embedder_model_dimension: None,
        }
    }
}

/// Embedder provider types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbedderProvider {
    #[serde(rename = "openai")]
    Openai,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "openai-compatible")]
    OpenaiCompatible,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "mistral")]
    Mistral,
    #[serde(rename = "vercel-ai-gateway")]
    VercelAiGateway,
    #[serde(rename = "bedrock")]
    Bedrock,
    #[serde(rename = "openrouter")]
    Openrouter,
}

impl Default for EmbedderProvider {
    fn default() -> Self {
        Self::Openai
    }
}

/// API handler options for different providers.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ApiHandlerOptions {
    pub open_ai_native_api_key: Option<String>,
    pub ollama_base_url: Option<String>,
}

/// OpenAI-compatible provider options.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenAiCompatibleOptions {
    pub base_url: String,
    pub api_key: String,
}

/// Bedrock provider options.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BedrockOptions {
    pub region: String,
    pub profile: Option<String>,
}

/// OpenRouter provider options.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenRouterOptions {
    pub api_key: String,
    pub specific_provider: Option<String>,
}

/// Resolved configuration after loading and validation.
#[derive(Clone, Debug, Default)]
pub struct ResolvedConfig {
    pub enabled: bool,
    pub embedder_provider: EmbedderProvider,
    pub model_id: Option<String>,
    pub model_dimension: Option<u64>,
    pub open_ai_options: Option<ApiHandlerOptions>,
    pub ollama_options: Option<ApiHandlerOptions>,
    pub open_ai_compatible_options: Option<OpenAiCompatibleOptions>,
    pub gemini_api_key: Option<String>,
    pub mistral_api_key: Option<String>,
    pub vercel_ai_gateway_api_key: Option<String>,
    pub bedrock_options: Option<BedrockOptions>,
    pub open_router_options: Option<OpenRouterOptions>,
    pub qdrant_url: Option<String>,
    pub qdrant_api_key: Option<String>,
    pub search_min_score: f64,
    pub search_max_results: usize,
}

/// Manages configuration state and validation for the code indexing feature.
///
/// Corresponds to `CodeIndexConfigManager` in `config-manager.ts`.
#[derive(Clone)]
pub struct CodeIndexConfigManager {
    config: ResolvedConfig,
    raw_config: CodeIndexFullConfig,
    /// API keys stored separately (secrets).
    secrets: SecretsStore,
}

/// Simple secrets store for API keys.
#[derive(Clone, Debug, Default)]
pub struct SecretsStore {
    pub open_ai_key: String,
    pub qdrant_api_key: String,
    pub open_ai_compatible_api_key: String,
    pub gemini_api_key: String,
    pub mistral_api_key: String,
    pub vercel_ai_gateway_api_key: String,
    pub open_router_api_key: String,
}

impl CodeIndexConfigManager {
    /// Creates a new config manager with default configuration.
    pub fn new() -> Self {
        let raw_config = CodeIndexFullConfig::default();
        let mut manager = Self {
            config: ResolvedConfig::default(),
            raw_config,
            secrets: SecretsStore::default(),
        };
        manager.load_and_set_configuration();
        manager
    }

    /// Creates a new config manager with the given raw config.
    pub fn with_config(raw_config: CodeIndexFullConfig, secrets: SecretsStore) -> Self {
        let mut manager = Self {
            config: ResolvedConfig::default(),
            raw_config,
            secrets,
        };
        manager.load_and_set_configuration();
        manager
    }

    /// Loads and sets configuration from the raw config and secrets.
    fn load_and_set_configuration(&mut self) {
        let raw = &self.raw_config;
        let secrets = &self.secrets;

        self.config.enabled = raw.codebase_index_enabled;
        self.config.qdrant_url = Some(raw.codebase_index_qdrant_url.clone());
        self.config.qdrant_api_key = Some(secrets.qdrant_api_key.clone()).filter(|s| !s.is_empty());
        self.config.search_min_score = raw.codebase_index_search_min_score.unwrap_or(DEFAULT_SEARCH_MIN_SCORE);
        self.config.search_max_results = raw.codebase_index_search_max_results.unwrap_or(DEFAULT_MAX_SEARCH_RESULTS);

        // Validate and set model dimension
        if let Some(dim) = raw.codebase_index_embedder_model_dimension {
            if dim > 0 {
                self.config.model_dimension = Some(dim);
            } else {
                self.config.model_dimension = None;
            }
        } else {
            self.config.model_dimension = None;
        }

        // Set embedder provider
        self.config.embedder_provider = match raw.codebase_index_embedder_provider.as_str() {
            "ollama" => EmbedderProvider::Ollama,
            "openai-compatible" => EmbedderProvider::OpenaiCompatible,
            "gemini" => EmbedderProvider::Gemini,
            "mistral" => EmbedderProvider::Mistral,
            "vercel-ai-gateway" => EmbedderProvider::VercelAiGateway,
            "bedrock" => EmbedderProvider::Bedrock,
            "openrouter" => EmbedderProvider::Openrouter,
            _ => EmbedderProvider::Openai,
        };

        // Set model ID
        self.config.model_id = if raw.codebase_index_embedder_model_id.is_empty() {
            None
        } else {
            Some(raw.codebase_index_embedder_model_id.clone())
        };

        // Set provider-specific options
        self.config.open_ai_options = Some(ApiHandlerOptions {
            open_ai_native_api_key: if secrets.open_ai_key.is_empty() {
                None
            } else {
                Some(secrets.open_ai_key.clone())
            },
            ..Default::default()
        });

        self.config.ollama_options = Some(ApiHandlerOptions {
            ollama_base_url: if raw.codebase_index_embedder_base_url.is_empty() {
                None
            } else {
                Some(raw.codebase_index_embedder_base_url.clone())
            },
            ..Default::default()
        });

        self.config.open_ai_compatible_options = match (
            &raw.codebase_index_open_ai_compatible_base_url,
            &secrets.open_ai_compatible_api_key,
        ) {
            (Some(url), key) if !url.is_empty() && !key.is_empty() => Some(OpenAiCompatibleOptions {
                base_url: url.clone(),
                api_key: key.clone(),
            }),
            _ => None,
        };

        self.config.gemini_api_key = if secrets.gemini_api_key.is_empty() {
            None
        } else {
            Some(secrets.gemini_api_key.clone())
        };

        self.config.mistral_api_key = if secrets.mistral_api_key.is_empty() {
            None
        } else {
            Some(secrets.mistral_api_key.clone())
        };

        self.config.vercel_ai_gateway_api_key = if secrets.vercel_ai_gateway_api_key.is_empty() {
            None
        } else {
            Some(secrets.vercel_ai_gateway_api_key.clone())
        };

        self.config.bedrock_options = if raw.codebase_index_bedrock_region.is_empty() {
            None
        } else {
            Some(BedrockOptions {
                region: raw.codebase_index_bedrock_region.clone(),
                profile: if raw.codebase_index_bedrock_profile.is_empty() {
                    None
                } else {
                    Some(raw.codebase_index_bedrock_profile.clone())
                },
            })
        };

        self.config.open_router_options = if secrets.open_router_api_key.is_empty() {
            None
        } else {
            Some(OpenRouterOptions {
                api_key: secrets.open_router_api_key.clone(),
                specific_provider: raw.codebase_index_open_router_specific_provider.clone(),
            })
        };
    }

    /// Returns whether the feature is enabled.
    pub fn is_feature_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Returns whether the feature is properly configured.
    pub fn is_feature_configured(&self) -> bool {
        match self.config.embedder_provider {
            EmbedderProvider::Openai => self.config.open_ai_options.as_ref().and_then(|o| o.open_ai_native_api_key.as_ref()).is_some(),
            EmbedderProvider::Ollama => self.config.ollama_options.as_ref().and_then(|o| o.ollama_base_url.as_ref()).is_some(),
            EmbedderProvider::OpenaiCompatible => self.config.open_ai_compatible_options.is_some(),
            EmbedderProvider::Gemini => self.config.gemini_api_key.is_some(),
            EmbedderProvider::Mistral => self.config.mistral_api_key.is_some(),
            EmbedderProvider::VercelAiGateway => self.config.vercel_ai_gateway_api_key.is_some(),
            EmbedderProvider::Bedrock => self.config.bedrock_options.is_some(),
            EmbedderProvider::Openrouter => self.config.open_router_options.is_some(),
        }
    }

    /// Returns the current resolved configuration.
    pub fn get_config(&self) -> &ResolvedConfig {
        &self.config
    }

    /// Returns the current search minimum score.
    pub fn current_search_min_score(&self) -> f64 {
        self.config.search_min_score
    }

    /// Returns the current search max results.
    pub fn current_search_max_results(&self) -> usize {
        self.config.search_max_results
    }

    /// Clones this config manager.
    pub fn clone_config(&self) -> Self {
        Self {
            config: self.config.clone(),
            raw_config: self.raw_config.clone(),
            secrets: self.secrets.clone(),
        }
    }
}

impl Default for CodeIndexConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_manager_default() {
        let cm = CodeIndexConfigManager::new();
        assert!(!cm.is_feature_enabled());
        assert!(!cm.is_feature_configured()); // No API key set
    }

    #[test]
    fn test_config_manager_with_openai_key() {
        let mut raw = CodeIndexFullConfig::default();
        raw.codebase_index_enabled = true;
        let mut secrets = SecretsStore::default();
        secrets.open_ai_key = "test-key".to_string();

        let cm = CodeIndexConfigManager::with_config(raw, secrets);
        assert!(cm.is_feature_enabled());
        assert!(cm.is_feature_configured());
        assert_eq!(cm.get_config().embedder_provider, EmbedderProvider::Openai);
    }

    #[test]
    fn test_config_manager_ollama() {
        let mut raw = CodeIndexFullConfig::default();
        raw.codebase_index_enabled = true;
        raw.codebase_index_embedder_provider = "ollama".to_string();
        raw.codebase_index_embedder_base_url = "http://localhost:11434".to_string();

        let cm = CodeIndexConfigManager::with_config(raw, SecretsStore::default());
        assert!(cm.is_feature_enabled());
        assert!(cm.is_feature_configured());
        assert_eq!(cm.get_config().embedder_provider, EmbedderProvider::Ollama);
    }

    #[test]
    fn test_config_manager_search_defaults() {
        let cm = CodeIndexConfigManager::new();
        assert!((cm.current_search_min_score() - DEFAULT_SEARCH_MIN_SCORE).abs() < f64::EPSILON);
        assert_eq!(cm.current_search_max_results(), DEFAULT_MAX_SEARCH_RESULTS);
    }

    #[test]
    fn test_config_manager_model_dimension() {
        let mut raw = CodeIndexFullConfig::default();
        raw.codebase_index_embedder_model_dimension = Some(1536);
        let cm = CodeIndexConfigManager::with_config(raw, SecretsStore::default());
        assert_eq!(cm.get_config().model_dimension, Some(1536));
    }

    #[test]
    fn test_config_manager_invalid_model_dimension() {
        let mut raw = CodeIndexFullConfig::default();
        raw.codebase_index_embedder_model_dimension = Some(0);
        let cm = CodeIndexConfigManager::with_config(raw, SecretsStore::default());
        assert_eq!(cm.get_config().model_dimension, None);
    }

    #[test]
    fn test_embedder_provider_default() {
        assert_eq!(EmbedderProvider::default(), EmbedderProvider::Openai);
    }

    #[test]
    fn test_full_config_default() {
        let config = CodeIndexFullConfig::default();
        assert!(!config.codebase_index_enabled);
        assert_eq!(config.codebase_index_qdrant_url, "http://localhost:6333");
        assert_eq!(config.codebase_index_embedder_provider, "openai");
    }
}
