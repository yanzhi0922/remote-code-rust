//! Unbound-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Unbound provider.
///
/// Unbound provides access to multiple LLM providers through a unified API.
/// Source: `src/api/providers/unbound.ts`
#[derive(Debug, Clone)]
pub struct UnboundConfig {
    /// API key for Unbound.
    pub api_key: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl UnboundConfig {
    /// Default Unbound API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.getunbound.ai/v1";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.unbound_api_key.clone()?;
        Some(Self {
            api_key,
            model_id: settings.unbound_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }
}
