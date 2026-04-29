//! xAI-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the xAI provider.
#[derive(Debug, Clone)]
pub struct XaiConfig {
    /// API key for xAI.
    pub api_key: String,
    /// Base URL for the xAI API.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl XaiConfig {
    /// Default xAI API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.x.ai/v1";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.api_key.clone()?;
        let base_url = settings
            .xai_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Some(Self {
            api_key,
            base_url,
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }
}
