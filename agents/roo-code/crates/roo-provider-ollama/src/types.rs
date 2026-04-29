//! Ollama-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Ollama provider.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Base URL for the Ollama API.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
    /// Additional Ollama API options.
    pub api_options: Option<serde_json::Value>,
}

impl OllamaConfig {
    /// Default Ollama API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "http://localhost:11434/v1";

    /// Create configuration from provider settings.
    /// Ollama doesn't require an API key, so this always returns Some.
    pub fn from_settings(settings: &ProviderSettings) -> Self {
        let base_url = settings
            .ollama_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Self {
            base_url,
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
            api_options: settings.ollama_api_options.clone(),
        }
    }
}
