//! Configuration types for OpenAI Native and Codex providers.

use serde::{Deserialize, Serialize};
use roo_types::provider_settings::ProviderSettings;

// ---------------------------------------------------------------------------
// OpenAI Native (Responses API with API key auth)
// ---------------------------------------------------------------------------

/// Configuration for the OpenAI Native provider.
///
/// Uses the Responses API (`POST /v1/responses`) with standard API key
/// authentication.
#[derive(Debug, Clone)]
pub struct OpenAiNativeConfig {
    /// API key for OpenAI.
    pub api_key: String,
    /// Base URL for the OpenAI API (defaults to `https://api.openai.com`).
    pub base_url: Option<String>,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Reasoning effort (e.g. "low", "medium", "high", "xhigh").
    pub reasoning_effort: Option<String>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
    /// Service tier (e.g. "default", "flex", "priority").
    pub service_tier: Option<String>,
    /// Whether to enable reasoning summary in responses.
    pub enable_reasoning_summary: bool,
}

impl OpenAiNativeConfig {
    /// Default OpenAI API base URL (without `/v1` suffix — Responses API
    /// appends `/v1/responses` itself).
    pub const DEFAULT_BASE_URL: &'static str = "https://api.openai.com";

    /// Default temperature for OpenAI Native models.
    pub const DEFAULT_TEMPERATURE: f64 = 0.0;

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.api_key.clone()?;

        Some(Self {
            api_key,
            base_url: None,
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            reasoning_effort: settings.reasoning_effort.map(|v| {
                serde_json::to_string(&v)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string()
            }),
            request_timeout: settings.request_timeout,
            service_tier: None,
            enable_reasoning_summary: true,
        })
    }
}

// ---------------------------------------------------------------------------
// OpenAI Codex (Responses API with OAuth auth)
// ---------------------------------------------------------------------------

/// Configuration for the OpenAI Codex provider.
///
/// Uses the Responses API routed through `chatgpt.com/backend-api/codex`
/// with OAuth Bearer token authentication.
#[derive(Debug, Clone)]
pub struct OpenAiCodexConfig {
    /// OAuth access token (obtained externally).
    pub access_token: String,
    /// ChatGPT account ID for organisation subscriptions.
    pub account_id: Option<String>,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Reasoning effort.
    pub reasoning_effort: Option<String>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl OpenAiCodexConfig {
    /// Codex API base URL.
    pub const CODEX_BASE_URL: &'static str = "https://chatgpt.com/backend-api/codex";

    /// Create configuration from provider settings.
    ///
    /// Note: `access_token` must be set separately after construction
    /// since it comes from the OAuth flow, not provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let access_token = settings.api_key.clone()?;

        Some(Self {
            access_token,
            account_id: None,
            model_id: settings.api_model_id.clone(),
            reasoning_effort: settings.reasoning_effort.map(|v| {
                serde_json::to_string(&v)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string()
            }),
            request_timeout: settings.request_timeout,
        })
    }
}

// ---------------------------------------------------------------------------
// Responses API request body
// ---------------------------------------------------------------------------

/// Request body for the OpenAI Responses API (`POST /v1/responses`).
///
/// This is a structured representation that gets serialised to JSON.
/// Both Native and Codex handlers use this shared type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResponsesApiRequestBody {
    /// Model ID (e.g. "gpt-5.1-codex-max").
    pub model: String,
    /// Conversation history in Responses API format.
    pub input: Vec<serde_json::Value>,
    /// Whether to stream the response.
    pub stream: bool,
    /// System prompt (Responses API uses `instructions` for this).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Tool definitions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    /// Tool choice strategy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    /// Whether to allow parallel tool calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    /// Temperature for generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Maximum output tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    /// Reasoning configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<serde_json::Value>,
    /// Text / verbosity configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<serde_json::Value>,
    /// Whether to store the response server-side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    /// Include additional response data (e.g. reasoning.encrypted_content).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    /// Service tier (e.g. "default", "flex", "priority").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    /// Prompt cache retention policy ("in_memory" or "24h").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
}
