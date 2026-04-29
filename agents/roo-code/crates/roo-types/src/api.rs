//! API stream type definitions.
//!
//! Derived from `src/api/transform/stream.ts`.
//! Defines all 11 API stream chunk types used for streaming responses.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ApiStreamChunk — the 11 stream chunk types
// ---------------------------------------------------------------------------

/// All possible chunk types in an API stream response.
///
/// Source: `src/api/transform/stream.ts` — `ApiStreamChunk`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ApiStreamChunk {
    /// Plain text content.
    #[serde(rename = "text")]
    Text { text: String },

    /// Token usage information.
    #[serde(rename = "usage")]
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_write_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_read_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total_cost: Option<f64>,
    },

    /// Reasoning/thinking content.
    /// For Anthropic extended thinking, may include a signature field
    /// required for passing thinking blocks back to the API during tool use.
    #[serde(rename = "reasoning")]
    Reasoning {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },

    /// Signals completion of a thinking block with its verification signature.
    /// Used by Anthropic extended thinking to pass the signature needed for
    /// tool use continuations and caching.
    #[serde(rename = "thinking_complete")]
    ThinkingComplete { signature: String },

    /// Grounding sources (for Gemini models).
    #[serde(rename = "grounding")]
    Grounding { sources: Vec<GroundingSource> },

    /// Complete tool call with id, name, and arguments.
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },

    /// Start of a tool call.
    #[serde(rename = "tool_call_start")]
    ToolCallStart {
        id: String,
        name: String,
    },

    /// Delta content for a tool call.
    #[serde(rename = "tool_call_delta")]
    ToolCallDelta {
        id: String,
        delta: String,
    },

    /// End of a tool call.
    #[serde(rename = "tool_call_end")]
    ToolCallEnd { id: String },

    /// Raw tool call chunk from the API stream.
    /// Providers emit this simple format; NativeToolCallParser handles all
    /// state management (tracking, buffering, emitting start/delta/end events).
    #[serde(rename = "tool_call_partial")]
    ToolCallPartial {
        /// Sequential index of the tool call in the response.
        index: u64,
        /// Tool call ID (present on the first chunk for each tool call).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Function name (present on the first chunk for each tool call).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Incremental JSON arguments string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arguments: Option<String>,
    },

    /// Error during streaming.
    #[serde(rename = "error")]
    Error {
        error: String,
        message: String,
    },
}

// ---------------------------------------------------------------------------
// GroundingSource
// ---------------------------------------------------------------------------

/// A grounding source from search-augmented models.
///
/// Source: `src/api/transform/stream.ts` — `GroundingSource`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingSource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

// ---------------------------------------------------------------------------
// ApiStreamError
// ---------------------------------------------------------------------------

/// A structured error in the API stream.
///
/// Source: `src/api/transform/stream.ts` — `ApiStreamError`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStreamError {
    pub error: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Content block types (Anthropic-style)
// ---------------------------------------------------------------------------

/// Anthropic-style content block types.
///
/// These are used in the API conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "image")]
    Image {
        source: ImageSource,
    },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Vec<ToolResultContent>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },

    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        signature: String,
    },

    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        data: String,
    },
}

/// Image source data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImageSource {
    #[serde(rename = "base64")]
    Base64 {
        media_type: String,
        data: String,
    },
    #[serde(rename = "url")]
    Url {
        url: String,
    },
}

/// Content within a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
}

// ---------------------------------------------------------------------------
// ApiMessage
// ---------------------------------------------------------------------------

/// A message in the API conversation history.
///
/// Source: Used throughout `src/core/task/Task.ts`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: MessageRole,
    pub content: Vec<ContentBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// Timestamp for ordering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<f64>,

    // --- Truncation fields (sliding window) ---

    /// If set, this message was truncated by the sliding window with this truncation ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_parent: Option<String>,
    /// Whether this message is a truncation marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_truncation_marker: Option<bool>,
    /// The truncation ID for a truncation marker message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_id: Option<String>,

    // --- Condensation fields ---

    /// If set, this message was condensed with this condense ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condense_parent: Option<String>,
    /// Whether this message is a conversation summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_summary: Option<bool>,
    /// The condense ID for a summary message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condense_id: Option<String>,

    // --- Reasoning details (OpenRouter/Gemini format) ---

    /// Reasoning details array for models that report structured reasoning
    /// (e.g., OpenRouter format for Gemini 3).
    /// Source: `src/core/task/Task.ts` — `reasoning_details` on ApiMessage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_details: Option<Vec<serde_json::Value>>,
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

// ---------------------------------------------------------------------------
// ProviderName
// ---------------------------------------------------------------------------

/// All supported AI provider names.
///
/// Source: `packages/types/src/provider-settings.ts` — `providerNames`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderName {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    Openai,
    #[serde(rename = "openai-native")]
    OpenaiNative,
    #[serde(rename = "openai-codex")]
    OpenaiCodex,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "gemini-cli")]
    GeminiCli,
    #[serde(rename = "vertex")]
    Vertex,
    #[serde(rename = "bedrock")]
    Bedrock,
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "lmstudio")]
    LmStudio,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "xai")]
    Xai,
    #[serde(rename = "minimax")]
    MiniMax,
    #[serde(rename = "moonshot")]
    Moonshot,
    #[serde(rename = "qwen-code")]
    QwenCode,
    #[serde(rename = "zai")]
    Zai,
    #[serde(rename = "mistral")]
    Mistral,
    #[serde(rename = "fireworks")]
    Fireworks,
    #[serde(rename = "sambanova")]
    SambaNova,
    #[serde(rename = "baseten")]
    Baseten,
    #[serde(rename = "vscode-lm")]
    VscodeLm,
    #[serde(rename = "poe")]
    Poe,
    #[serde(rename = "litellm")]
    LiteLlm,
    #[serde(rename = "requesty")]
    Requesty,
    #[serde(rename = "unbound")]
    Unbound,
    #[serde(rename = "roo")]
    Roo,
    #[serde(rename = "vercel-ai-gateway")]
    VercelAiGateway,
    #[serde(rename = "fake-ai")]
    FakeAi,
}

impl ProviderName {
    /// Returns the string identifier used in settings.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::OpenaiNative => "openai-native",
            Self::OpenaiCodex => "openai-codex",
            Self::Gemini => "gemini",
            Self::GeminiCli => "gemini-cli",
            Self::Vertex => "vertex",
            Self::Bedrock => "bedrock",
            Self::OpenRouter => "openrouter",
            Self::Ollama => "ollama",
            Self::LmStudio => "lmstudio",
            Self::DeepSeek => "deepseek",
            Self::Xai => "xai",
            Self::MiniMax => "minimax",
            Self::Moonshot => "moonshot",
            Self::QwenCode => "qwen-code",
            Self::Zai => "zai",
            Self::Mistral => "mistral",
            Self::Fireworks => "fireworks",
            Self::SambaNova => "sambanova",
            Self::Baseten => "baseten",
            Self::VscodeLm => "vscode-lm",
            Self::Poe => "poe",
            Self::LiteLlm => "litellm",
            Self::Requesty => "requesty",
            Self::Unbound => "unbound",
            Self::Roo => "roo",
            Self::VercelAiGateway => "vercel-ai-gateway",
            Self::FakeAi => "fake-ai",
        }
    }
}
