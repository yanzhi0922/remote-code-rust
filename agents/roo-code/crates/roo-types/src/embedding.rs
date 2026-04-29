//! Embedding type definitions.
//!
//! Derived from `packages/types/src/embedding.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// EmbedderProvider
// ---------------------------------------------------------------------------

/// Supported embedding providers.
///
/// Source: `packages/types/src/embedding.ts` — `EmbedderProvider`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

// ---------------------------------------------------------------------------
// EmbeddingModelProfile
// ---------------------------------------------------------------------------

/// Profile for an embedding model.
///
/// Source: `packages/types/src/embedding.ts` — `EmbeddingModelProfile`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelProfile {
    pub dimension: u64,
    pub score_threshold: Option<f64>,
    pub query_prefix: Option<String>,
}

/// Embedding model profiles by provider.
///
/// Source: `packages/types/src/embedding.ts` — `EmbeddingModelProfiles`
pub type EmbeddingModelProfiles = std::collections::HashMap<String, std::collections::HashMap<String, EmbeddingModelProfile>>;
