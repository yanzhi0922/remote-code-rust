//! OpenAI-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the OpenAI provider.
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    /// API key for OpenAI.
    pub api_key: String,
    /// Base URL for the OpenAI API.
    pub base_url: String,
    /// Organization ID for OpenAI.
    pub org_id: Option<String>,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Reasoning effort (e.g. "low", "medium", "high").
    pub reasoning_effort: Option<String>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl OpenAiConfig {
    /// Default OpenAI API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.openai.com/v1";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.api_key.clone()?;
        let base_url = settings
            .open_ai_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Some(Self {
            api_key,
            base_url,
            org_id: settings.open_ai_org_id.clone(),
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            reasoning_effort: settings
                .model_reasoning_effort
                .clone()
                .or_else(|| {
                    settings.reasoning_effort.map(|v| {
                        serde_json::to_string(&v)
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string()
                    })
                }),
            request_timeout: settings.request_timeout,
        })
    }
}
