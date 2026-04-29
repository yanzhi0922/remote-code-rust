//! Roo Code Cloud-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Roo Code Cloud provider.
///
/// Roo Code Cloud provides access to models via Roo's own infrastructure.
/// Uses session token authentication and supports dynamic model loading.
/// Source: `src/api/providers/roo.ts`
#[derive(Debug, Clone)]
pub struct RooConfig {
    /// API key for Roo Code Cloud.
    pub api_key: Option<String>,
    /// Optional custom base URL for Roo.
    pub base_url: Option<String>,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl RooConfig {
    /// Default Roo API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.roocode.com/v1";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        // Roo provider requires at least one of api_key or base_url
        let has_config = settings.roo_api_key.is_some() || settings.roo_base_url.is_some();
        if !has_config {
            return None;
        }

        Some(Self {
            api_key: settings.roo_api_key.clone(),
            base_url: settings
                .roo_base_url
                .clone()
                .or_else(|| Some(Self::DEFAULT_BASE_URL.to_string())),
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }
}
