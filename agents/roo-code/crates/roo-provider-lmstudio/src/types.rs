//! LM Studio-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the LM Studio provider.
#[derive(Debug, Clone)]
pub struct LmStudioConfig {
    /// Base URL for the LM Studio API (default: "http://localhost:1234").
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
    /// Whether speculative decoding is enabled.
    pub speculative_decoding_enabled: bool,
    /// Draft model ID for speculative decoding.
    pub draft_model_id: Option<String>,
}

impl LmStudioConfig {
    /// Default LM Studio API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "http://localhost:1234/v1";

    /// LM Studio uses "noop" as a placeholder API key.
    pub const PLACEHOLDER_API_KEY: &'static str = "noop";

    /// Create configuration from provider settings.
    ///
    /// LM Studio doesn't require an API key, so this always returns a config.
    pub fn from_settings(settings: &ProviderSettings) -> Self {
        let base_url = settings
            .lm_studio_base_url
            .clone()
            .map(|url| {
                // Ensure URL ends with /v1
                let url = url.trim_end_matches('/');
                if url.ends_with("/v1") {
                    url.to_string()
                } else {
                    format!("{}/v1", url)
                }
            })
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Self {
            base_url,
            model_id: settings
                .lm_studio_model_id
                .clone()
                .or(settings.api_model_id.clone()),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
            speculative_decoding_enabled: settings
                .lm_studio_speculative_decoding_enabled
                .unwrap_or(false),
            draft_model_id: settings.lm_studio_draft_model_id.clone(),
        }
    }
}
