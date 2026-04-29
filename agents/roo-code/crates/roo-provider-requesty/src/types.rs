//! Requesty-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Requesty provider.
///
/// Requesty is an LLM router that supports trace_id and mode tracking.
/// Source: `src/api/providers/requesty.ts`
#[derive(Debug, Clone)]
pub struct RequestyConfig {
    /// API key for Requesty.
    pub api_key: String,
    /// Base URL for the Requesty API.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl RequestyConfig {
    /// Default Requesty API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.requesty.ai";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.requesty_api_key.clone()?;
        let base_url = settings
            .requesty_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Some(Self {
            api_key,
            base_url,
            model_id: settings.requesty_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }
}
