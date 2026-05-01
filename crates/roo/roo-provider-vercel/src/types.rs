//! Vercel AI Gateway-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Vercel AI Gateway provider.
///
/// Vercel AI Gateway provides a unified API for accessing various LLM models.
/// It uses a fixed URL and supports prompt caching.
/// Source: `src/api/providers/vercel-ai-gateway.ts`
#[derive(Debug, Clone)]
pub struct VercelConfig {
    /// API key for Vercel AI Gateway.
    pub api_key: String,
    /// Optional custom base URL for Vercel AI Gateway.
    pub base_url: Option<String>,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation (default: 0.5).
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl VercelConfig {
    /// Default Vercel AI Gateway base URL.
    pub const DEFAULT_BASE_URL: &'static str =
        "https://sdk.vercel.ai/api/v1/ai-gateway-gateway";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.vercel_api_key.clone()?;
        Some(Self {
            api_key,
            base_url: settings.vercel_base_url.clone(),
            model_id: settings
                .vercel_model_id
                .clone()
                .or(settings.api_model_id.clone()),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }
}
