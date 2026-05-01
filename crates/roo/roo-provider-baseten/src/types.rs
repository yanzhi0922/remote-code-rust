//! Baseten-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Baseten provider.
#[derive(Debug, Clone)]
pub struct BasetenConfig {
    /// API key for Baseten.
    pub api_key: String,
    /// Base URL for the Baseten API.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl BasetenConfig {
    /// Default Baseten API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://inference.baseten.co/v1";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.baseten_api_key.clone()?;
        let base_url = settings
            .baseten_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());
        let model_id = settings.baseten_model_id.clone().or(settings.api_model_id.clone());

        Some(Self {
            api_key,
            base_url,
            model_id,
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }
}
