//! Anthropic-specific configuration and SSE response types.

use serde::Deserialize;
use roo_types::provider_settings::ProviderSettings;
use roo_types::model::ModelInfo;

/// Configuration for the Anthropic provider.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// API key for Anthropic.
    pub api_key: String,
    /// Base URL for the Anthropic API.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Whether to use extended thinking.
    pub use_extended_thinking: Option<bool>,
    /// Max thinking tokens for extended thinking.
    pub max_thinking_tokens: Option<u64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl AnthropicConfig {
    /// Default Anthropic API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.anthropic.com";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings.api_key.clone()?;
        let base_url = settings
            .anthropic_base_url
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_string());

        Some(Self {
            api_key,
            base_url,
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            use_extended_thinking: settings.anthropic_use_extended_thinking,
            max_thinking_tokens: settings.model_max_thinking_tokens,
            request_timeout: settings.request_timeout,
        })
    }
}

// ---------------------------------------------------------------------------
// Anthropic SSE response types
// ---------------------------------------------------------------------------

/// Anthropic SSE event types.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(tag = "type")]
pub enum AnthropicSseEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: AnthropicMessage },

    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: u64,
        content_block: AnthropicContentBlock,
    },

    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: u64,
        delta: AnthropicDelta,
    },

    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: AnthropicMessageDelta,
        usage: Option<AnthropicUsage>,
    },

    #[serde(rename = "message_stop")]
    MessageStop,

    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u64 },

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "error")]
    Error { error: AnthropicErrorBody },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AnthropicMessage {
    pub id: Option<String>,
    pub model: Option<String>,
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta {
        partial_json: String,
    },
    #[serde(rename = "signature_delta")]
    SignatureDelta {
        signature: String,
    },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AnthropicMessageDelta {
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AnthropicUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicErrorBody {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// Anthropic Vertex AI configuration
// ---------------------------------------------------------------------------

/// Model IDs that support the 1M context beta on Vertex AI.
pub const VERTEX_1M_CONTEXT_MODEL_IDS: &[&str] = &[
    "claude-sonnet-4@20250514",
    "claude-sonnet-4-5@20250929",
    "claude-sonnet-4-6",
    "claude-opus-4-6",
    "claude-opus-4-5@20251101",
    "claude-opus-4-1@20250805",
];

/// Default Anthropic Vertex model ID.
pub const ANTHROPIC_VERTEX_DEFAULT_MODEL_ID: &str = "claude-sonnet-4-5@20250929";

/// Configuration for the Anthropic Vertex AI provider.
///
/// Uses the Anthropic Messages API format through Vertex AI's
/// `streamRawPredict` endpoint with OAuth2 bearer token authentication.
#[derive(Debug, Clone)]
pub struct AnthropicVertexConfig {
    /// Google Cloud project ID.
    pub project_id: String,
    /// Vertex AI region (default: "us-east5").
    pub region: String,
    /// Google Cloud OAuth2 access token for authentication.
    ///
    /// In a production environment, this should be obtained through
    /// proper OAuth2 authentication using service account credentials
    /// or the Application Default Credentials mechanism.
    pub access_token: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
    /// Whether to enable 1M context window (beta).
    pub enable_1m_context: bool,
    /// Whether to use extended thinking.
    pub use_extended_thinking: Option<bool>,
    /// Max thinking tokens for extended thinking.
    pub max_thinking_tokens: Option<u64>,
}

impl AnthropicVertexConfig {
    /// Default Vertex AI region for Anthropic models.
    pub const DEFAULT_REGION: &'static str = "us-east5";
    /// Default project ID placeholder.
    pub const DEFAULT_PROJECT_ID: &'static str = "not-provided";

    /// Build the Vertex AI base URL for Anthropic models.
    ///
    /// Returns: `https://{region}-aiplatform.googleapis.com/v1/projects/{project_id}/locations/{region}/publishers/anthropic/models`
    pub fn base_url(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models",
            self.region, self.project_id, self.region
        )
    }

    /// Build the streaming URL for the `streamRawPredict` endpoint.
    ///
    /// The endpoint format is:
    /// `{base_url}/{model_id}:streamRawPredict`
    pub fn stream_url(&self, model_id: &str) -> String {
        // Strip :thinking suffix for the actual API call
        let clean_id = if model_id.ends_with(":thinking") {
            &model_id[..model_id.len() - ":thinking".len()]
        } else {
            model_id
        };
        format!("{}/{}:streamRawPredict", self.base_url(), clean_id)
    }

    /// Build the non-streaming URL for the `rawPredict` endpoint.
    pub fn predict_url(&self, model_id: &str) -> String {
        let clean_id = if model_id.ends_with(":thinking") {
            &model_id[..model_id.len() - ":thinking".len()]
        } else {
            model_id
        };
        format!("{}/{}:rawPredict", self.base_url(), clean_id)
    }

    /// Create configuration from provider settings.
    ///
    /// Requires `vertex_project_id` and either `vertex_json_credentials`
    /// (a Google Cloud service account JSON) or `vertex_key_file`.
    ///
    /// The `access_token` field stores the raw credentials string. The
    /// [`AnthropicVertexHandler`](crate::handler::AnthropicVertexHandler)
    /// will attempt to parse it as a service account JSON for OAuth2 token
    /// management; if parsing fails, the string is used as a static token.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let project_id = settings
            .vertex_project_id
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_PROJECT_ID.to_string());

        let region = settings
            .vertex_region
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_REGION.to_string());

        // Store raw credentials — the handler will try to parse them as
        // service account JSON for OAuth2, or fall back to using as a raw token.
        let access_token = settings
            .vertex_json_credentials
            .clone()
            .or(settings.vertex_key_file.clone())?;

        let model_id = settings.api_model_id.clone();

        // Check if 1M context should be enabled
        let enable_1m_context = settings.vertex_1m_context.unwrap_or(false)
            && model_id
                .as_ref()
                .map(|id| VERTEX_1M_CONTEXT_MODEL_IDS.contains(&id.as_str()))
                .unwrap_or(false);

        Some(Self {
            project_id,
            region,
            access_token,
            model_id,
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
            enable_1m_context,
            use_extended_thinking: settings.anthropic_use_extended_thinking,
            max_thinking_tokens: settings.model_max_thinking_tokens,
        })
    }
}

// ---------------------------------------------------------------------------
// Anthropic Vertex AI model definitions
// ---------------------------------------------------------------------------

/// Returns the Anthropic Vertex AI models.
///
/// These are Claude models available through Vertex AI's Anthropic publisher
/// endpoint. They use the same Anthropic Messages API format but are served
/// through Google Cloud infrastructure.
pub fn anthropic_vertex_models() -> std::collections::HashMap<String, ModelInfo> {
    let mut m = std::collections::HashMap::new();

    m.insert(
        "claude-sonnet-4@20250514".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Sonnet 4 (Anthropic Vertex)".to_string()),
            tiers: Some(vec![roo_types::model::ModelTier {
                name: None,
                context_window: 1_000_000,
                input_price: Some(6.0),
                output_price: Some(22.5),
                cache_writes_price: Some(7.5),
                cache_reads_price: Some(0.6),
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-sonnet-4-5@20250929".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Sonnet 4.5 (Anthropic Vertex)".to_string()),
            tiers: Some(vec![roo_types::model::ModelTier {
                name: None,
                context_window: 1_000_000,
                input_price: Some(6.0),
                output_price: Some(22.5),
                cache_writes_price: Some(7.5),
                cache_reads_price: Some(0.6),
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-sonnet-4-6".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Sonnet 4.6 (Anthropic Vertex)".to_string()),
            tiers: Some(vec![roo_types::model::ModelTier {
                name: None,
                context_window: 1_000_000,
                input_price: Some(6.0),
                output_price: Some(22.5),
                cache_writes_price: Some(7.5),
                cache_reads_price: Some(0.6),
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-haiku-4-5@20251001".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.0),
            output_price: Some(5.0),
            cache_writes_price: Some(1.25),
            cache_reads_price: Some(0.1),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Haiku 4.5 (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-6".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(5.0),
            output_price: Some(25.0),
            cache_writes_price: Some(6.25),
            cache_reads_price: Some(0.5),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Opus 4.6 (Anthropic Vertex)".to_string()),
            tiers: Some(vec![roo_types::model::ModelTier {
                name: None,
                context_window: 1_000_000,
                input_price: Some(10.0),
                output_price: Some(37.5),
                cache_writes_price: Some(12.5),
                cache_reads_price: Some(1.0),
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-5@20251101".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(5.0),
            output_price: Some(25.0),
            cache_writes_price: Some(6.25),
            cache_reads_price: Some(0.5),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Opus 4.5 (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-1@20250805".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Opus 4.1 (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4@20250514".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            description: Some("Claude Opus 4 (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-7-sonnet@20250219:thinking".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            required_reasoning_budget: Some(true),
            description: Some("Claude 3.7 Sonnet Thinking (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-7-sonnet@20250219".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Claude 3.7 Sonnet (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-sonnet-v2@20241022".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Claude 3.5 Sonnet v2 (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-sonnet@20240620".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Claude 3.5 Sonnet (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-haiku@20241022".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(1.0),
            output_price: Some(5.0),
            cache_writes_price: Some(1.25),
            cache_reads_price: Some(0.1),
            description: Some("Claude 3.5 Haiku (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-opus@20240229".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            description: Some("Claude 3 Opus (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-haiku@20240307".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.25),
            output_price: Some(1.25),
            cache_writes_price: Some(0.3),
            cache_reads_price: Some(0.03),
            description: Some("Claude 3 Haiku (Anthropic Vertex)".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default Anthropic Vertex model ID.
pub fn anthropic_vertex_default_model_id() -> String {
    ANTHROPIC_VERTEX_DEFAULT_MODEL_ID.to_string()
}
