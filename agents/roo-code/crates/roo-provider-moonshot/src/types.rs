//! Moonshot-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Moonshot provider.
#[derive(Debug, Clone)]
pub struct MoonshotConfig {
    /// API key for Moonshot.
    pub api_key: String,
    /// Base URL for the Moonshot API.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl MoonshotConfig {
    /// Default Moonshot API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.moonshot.ai/v1";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.moonshot_api_key.clone()?;
        let base_url = settings
            .moonshot_base_url
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
