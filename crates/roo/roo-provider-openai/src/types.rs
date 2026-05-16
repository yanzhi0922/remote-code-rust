//! OpenAI-specific configuration types.

use std::collections::HashMap;

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the OpenAI provider.
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    /// API key for OpenAI.
    pub api_key: String,
    /// Base URL for the OpenAI API.
    pub base_url: String,
    /// Organization ID for OpenAI.
    pub org_id: Option<String>,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Reasoning effort (e.g. "low", "medium", "high").
    pub reasoning_effort: Option<String>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
    /// Whether to use Azure OpenAI.
    pub use_azure: bool,
    /// Azure API version (when use_azure is true).
    pub azure_api_version: Option<String>,
    /// Whether streaming is enabled.
    pub streaming_enabled: bool,
    /// Additional HTTP headers to send with requests.
    pub headers: HashMap<String, String>,
    /// Whether to use the R1 format for reasoning.
    pub r1_format_enabled: bool,
    /// Custom model info override.
    pub custom_model_info: Option<roo_types::model::ModelInfo>,
}

impl OpenAiConfig {
    /// Default OpenAI API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.openai.com/v1";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings
            .open_ai_api_key
            .clone()
            .or_else(|| settings.api_key.clone())?;
        let base_url = settings
            .open_ai_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Some(Self {
            api_key,
            base_url,
            org_id: settings.open_ai_org_id.clone(),
            model_id: settings
                .open_ai_model_id
                .clone()
                .or_else(|| settings.api_model_id.clone()),
            temperature: settings.model_temperature.flatten(),
            reasoning_effort: settings.model_reasoning_effort.clone().or_else(|| {
                settings.reasoning_effort.map(|v| {
                    serde_json::to_string(&v)
                        .unwrap_or_default()
                        .trim_matches('"')
                        .to_string()
                })
            }),
            request_timeout: settings.request_timeout,
            use_azure: settings.open_ai_use_azure.unwrap_or(false),
            azure_api_version: settings.azure_api_version.clone(),
            streaming_enabled: settings.open_ai_streaming_enabled.unwrap_or(true),
            headers: settings.open_ai_headers.clone().unwrap_or_default(),
            r1_format_enabled: settings.open_ai_r1_format_enabled.unwrap_or(false),
            custom_model_info: settings.open_ai_custom_model_info.clone().map(|b| *b),
        })
    }
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: Self::DEFAULT_BASE_URL.to_string(),
            org_id: None,
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
            use_azure: false,
            azure_api_version: None,
            streaming_enabled: true,
            headers: HashMap::new(),
            r1_format_enabled: false,
            custom_model_info: None,
        }
    }
}
