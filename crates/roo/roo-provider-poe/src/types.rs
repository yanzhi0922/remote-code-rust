//! Poe-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Poe provider.
///
/// Poe uses the Vercel AI SDK with a Poe-specific provider.
/// In Rust, we use the OpenAI-compatible API as Poe supports it.
/// Source: `src/api/providers/poe.ts`
#[derive(Debug, Clone)]
pub struct PoeConfig {
    /// API key for Poe.
    pub api_key: String,
    /// Optional custom base URL for Poe.
    pub base_url: Option<String>,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Max thinking tokens for reasoning budget.
    pub max_thinking_tokens: Option<u64>,
    /// Reasoning effort level.
    pub reasoning_effort: Option<String>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl PoeConfig {
    /// Default Poe API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.poe.com/bot/";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.poe_api_key.clone()?;
        Some(Self {
            api_key,
            base_url: settings.poe_base_url.clone(),
            model_id: settings.poe_model_id.clone().or(settings.api_model_id.clone()),
            temperature: settings.model_temperature.flatten(),
            max_thinking_tokens: settings.model_max_thinking_tokens,
            reasoning_effort: settings.model_reasoning_effort.clone(),
            request_timeout: settings.request_timeout,
        })
    }
}
