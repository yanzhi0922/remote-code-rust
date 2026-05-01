//! Codebase index type definitions.
//!
//! Derived from `packages/types/src/codebase-index.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default values for codebase index search.
pub const CODEBASE_INDEX_DEFAULTS: CodebaseIndexDefaults = CodebaseIndexDefaults {
    min_search_results: 10,
    max_search_results: 200,
    default_search_results: 50,
    search_results_step: 10,
    min_search_score: 0.0,
    max_search_score: 1.0,
    default_search_min_score: 0.4,
    search_score_step: 0.05,
};

/// Codebase index defaults constants.
pub struct CodebaseIndexDefaults {
    pub min_search_results: u64,
    pub max_search_results: u64,
    pub default_search_results: u64,
    pub search_results_step: u64,
    pub min_search_score: f64,
    pub max_search_score: f64,
    pub default_search_min_score: f64,
    pub search_score_step: f64,
}

// ---------------------------------------------------------------------------
// CodebaseIndexEmbedderProvider
// ---------------------------------------------------------------------------

/// Embedder provider for codebase indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodebaseIndexEmbedderProvider {
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

// ---------------------------------------------------------------------------
// CodebaseIndexConfig
// ---------------------------------------------------------------------------

/// Configuration for codebase indexing.
///
/// Source: `packages/types/src/codebase-index.ts` — `codebaseIndexConfigSchema`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodebaseIndexConfig {
    pub codebase_index_enabled: Option<bool>,
    pub codebase_index_qdrant_url: Option<String>,
    pub codebase_index_embedder_provider: Option<CodebaseIndexEmbedderProvider>,
    pub codebase_index_embedder_base_url: Option<String>,
    pub codebase_index_embedder_model_id: Option<String>,
    pub codebase_index_embedder_model_dimension: Option<u64>,
    pub codebase_index_search_min_score: Option<f64>,
    pub codebase_index_search_max_results: Option<u64>,
    // OpenAI Compatible specific
    pub codebase_index_open_ai_compatible_base_url: Option<String>,
    pub codebase_index_open_ai_compatible_model_dimension: Option<u64>,
    // Bedrock specific
    pub codebase_index_bedrock_region: Option<String>,
    pub codebase_index_bedrock_profile: Option<String>,
    // OpenRouter specific
    pub codebase_index_open_router_specific_provider: Option<String>,
}

// ---------------------------------------------------------------------------
// CodebaseIndexModels
// ---------------------------------------------------------------------------

/// Model dimension info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDimensionInfo {
    pub dimension: u64,
}

/// Codebase index models by provider.
///
/// Source: `packages/types/src/codebase-index.ts` — `codebaseIndexModelsSchema`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodebaseIndexModels {
    pub openai: Option<std::collections::HashMap<String, ModelDimensionInfo>>,
    pub ollama: Option<std::collections::HashMap<String, ModelDimensionInfo>>,
    #[serde(rename = "openai-compatible")]
    pub openai_compatible: Option<std::collections::HashMap<String, ModelDimensionInfo>>,
    pub gemini: Option<std::collections::HashMap<String, ModelDimensionInfo>>,
    pub mistral: Option<std::collections::HashMap<String, ModelDimensionInfo>>,
    #[serde(rename = "vercel-ai-gateway")]
    pub vercel_ai_gateway: Option<std::collections::HashMap<String, ModelDimensionInfo>>,
    pub openrouter: Option<std::collections::HashMap<String, ModelDimensionInfo>>,
    pub bedrock: Option<std::collections::HashMap<String, ModelDimensionInfo>>,
}

// ---------------------------------------------------------------------------
// CodebaseIndexProvider
// ---------------------------------------------------------------------------

/// Codebase index provider settings.
///
/// Source: `packages/types/src/codebase-index.ts` — `codebaseIndexProviderSchema`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodebaseIndexProvider {
    pub code_index_open_ai_key: Option<String>,
    pub code_index_qdrant_api_key: Option<String>,
    pub codebase_index_open_ai_compatible_base_url: Option<String>,
    pub codebase_index_open_ai_compatible_api_key: Option<String>,
    pub codebase_index_open_ai_compatible_model_dimension: Option<u64>,
    pub codebase_index_gemini_api_key: Option<String>,
    pub codebase_index_mistral_api_key: Option<String>,
    pub codebase_index_vercel_ai_gateway_api_key: Option<String>,
    pub codebase_index_open_router_api_key: Option<String>,
}
