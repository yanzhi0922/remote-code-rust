//! MiniMax-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the MiniMax provider.
#[derive(Debug, Clone)]
pub struct MiniMaxConfig {
    /// API key for MiniMax.
    pub api_key: String,
    /// Base URL for the MiniMax API.
    pub base_url: String,
    /// MiniMax group ID (optional, used as header).
    pub group_id: Option<String>,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl MiniMaxConfig {
    /// Default MiniMax API base URL (Anthropic-compatible endpoint).
    pub const DEFAULT_BASE_URL: &'static str = "https://api.minimaxi.com/anthropic";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.minimax_api_key.clone()?;
        let base_url = settings
            .minimax_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Some(Self {
            api_key,
            base_url,
            group_id: settings.minimax_base_url.clone(),
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }
}
