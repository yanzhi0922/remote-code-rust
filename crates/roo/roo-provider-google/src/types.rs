//! Google Gemini-specific configuration and response types.

use serde::Deserialize;
use roo_types::provider_settings::ProviderSettings;

/// Configuration for the Google Gemini provider.
#[derive(Debug, Clone)]
pub struct GoogleConfig {
    /// API key for Google Gemini.
    pub api_key: String,
    /// Base URL for the Gemini API.
    pub base_url: String,
    /// Model ID to use.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl GoogleConfig {
    /// Default Google Gemini API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://generativelanguage.googleapis.com/v1beta";

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let api_key = settings
            .google_api_key
            .clone()
            .or(settings.api_key.clone())?;
        let base_url = settings
            .google_gemini_base_url
            .clone()
            .or(settings.gemini_base_url.clone())
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

// ---------------------------------------------------------------------------
// Gemini SSE response types (for parsing streaming responses)
// ---------------------------------------------------------------------------

/// A chunk from the Gemini streaming API.
#[derive(Debug, Deserialize)]
pub struct GeminiStreamResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    pub usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GeminiCandidate {
    pub content: Option<GeminiResponseContent>,
    pub finish_reason: Option<String>,
    pub grounding_metadata: Option<GeminiGroundingMetadata>,
}

/// Response content from Gemini (uses flat part structure).
#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct GeminiResponseContent {
    pub parts: Option<Vec<GeminiResponsePart>>,
    pub role: Option<String>,
}

/// A single part in a Gemini response.
///
/// In the Gemini API, `thought` is a boolean flag indicating whether
/// this part is a thinking/reasoning part. The text content of the
/// thought is in the `text` field. `thought_signature` is used for
/// Gemini 3 models to validate thought signatures during tool calling.
#[derive(Debug, Deserialize, Clone)]
pub struct GeminiResponsePart {
    pub text: Option<String>,
    pub function_call: Option<GeminiFunctionCall>,
    /// Whether this part is a thinking/reasoning part (boolean in the API).
    /// Deserialized from either a boolean or a string for compatibility.
    pub thought: Option<GeminiThoughtValue>,
    /// Thought signature for Gemini 3 models (required for tool calling round-trips).
    pub thought_signature: Option<String>,
}

/// Wrapper for the `thought` field which can be a boolean or string in the Gemini API.
/// The TS source checks `part.thought` as a boolean, but some API versions may send it
/// as a string. We handle both cases.
#[derive(Debug, Clone)]
pub enum GeminiThoughtValue {
    Bool(bool),
    String(String),
}

impl<'de> serde::de::Deserialize<'de> for GeminiThoughtValue {
    fn deserialize<D: serde::de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{self, Visitor};
        use std::fmt;

        struct ThoughtValueVisitor;

        impl<'de> Visitor<'de> for ThoughtValueVisitor {
            type Value = GeminiThoughtValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a boolean or a string")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(GeminiThoughtValue::Bool(v))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(GeminiThoughtValue::String(v.to_string()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(GeminiThoughtValue::String(v))
            }
        }

        deserializer.deserialize_any(ThoughtValueVisitor)
    }
}

impl GeminiThoughtValue {
    /// Returns true if the thought value indicates this is a thinking part.
    pub fn is_thinking(&self) -> bool {
        match self {
            GeminiThoughtValue::Bool(b) => *b,
            GeminiThoughtValue::String(s) => !s.is_empty(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct GeminiUsageMetadata {
    pub prompt_token_count: Option<u64>,
    pub candidates_token_count: Option<u64>,
    pub cached_content_token_count: Option<u64>,
    /// Token count for thinking/reasoning (thoughtsTokenCount in TS).
    pub thoughts_token_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GeminiGroundingMetadata {
    pub grounding_chunks: Option<Vec<GeminiGroundingChunk>>,
    pub search_entry_point: Option<GeminiSearchEntryPoint>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiGroundingChunk {
    pub web: Option<GeminiWebChunk>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiWebChunk {
    pub uri: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GeminiSearchEntryPoint {
    pub rendered_content: Option<String>,
}

// ---------------------------------------------------------------------------
// Vertex AI configuration
// ---------------------------------------------------------------------------

/// Configuration for the Vertex AI provider.
///
/// Vertex AI uses the same Gemini API format as Google Gemini,
/// but with a different endpoint and authentication mechanism.
#[derive(Debug, Clone)]
pub struct VertexConfig {
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
}

impl VertexConfig {
    /// Default Vertex AI region.
    pub const DEFAULT_REGION: &'static str = "us-east5";

    /// Create configuration from provider settings.
    ///
    /// Requires `vertex_project_id` and either `vertex_json_credentials`
    /// (a Google Cloud service account JSON) or `vertex_key_file`.
    ///
    /// The `access_token` field stores the raw credentials string. The
    /// [`VertexHandler`](crate::handler::VertexHandler) will attempt to
    /// parse it as a service account JSON for OAuth2 token management;
    /// if parsing fails, the string is used as a static access token.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let project_id = settings.vertex_project_id.clone()?;

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

        Some(Self {
            project_id,
            region,
            access_token,
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }

    /// Build the Vertex AI base URL from region.
    pub fn base_url(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1",
            self.region
        )
    }
}
