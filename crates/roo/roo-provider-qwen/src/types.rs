//! Qwen-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Qwen provider.
#[derive(Debug, Clone)]
pub struct QwenConfig {
    /// API key for Qwen.
    pub api_key: String,
    /// Base URL for the Qwen API.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl QwenConfig {
    /// Default Qwen API base URL (DashScope compatible mode).
    pub const DEFAULT_BASE_URL: &'static str = "https://dashscope.aliyuncs.com/compatible-mode/v1";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.qwen_api_key.clone()?;
        let base_url = settings
            .qwen_base_url
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
