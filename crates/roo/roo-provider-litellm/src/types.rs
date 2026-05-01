//! LiteLLM-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the LiteLLM provider.
///
/// LiteLLM acts as a proxy to various LLM providers, following the OpenAI API format.
/// Source: `src/api/providers/lite-llm.ts`
#[derive(Debug, Clone)]
pub struct LiteLlmConfig {
    /// API key for the LiteLLM server.
    pub api_key: String,
    /// Base URL for the LiteLLM server.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Whether to use prompt caching.
    pub use_prompt_cache: bool,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl LiteLlmConfig {
    /// Default LiteLLM API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "http://localhost:4000";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings
            .litellm_api_key
            .clone()
            .unwrap_or_else(|| "dummy-key".to_string());
        let base_url = settings
            .litellm_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Some(Self {
            api_key,
            base_url,
            model_id: settings.litellm_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            use_prompt_cache: false,
            request_timeout: settings.request_timeout,
        })
    }
}
