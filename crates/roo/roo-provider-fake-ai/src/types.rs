//! Configuration types for the FakeAI provider.

use serde::{Deserialize, Serialize};

/// What to inject on each `create_message` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponsePattern {
    /// Return a fixed text string.
    #[serde(rename = "text")]
    Text { text: String },

    /// Return text streamed one chunk at a time with optional inter-chunk
    /// delay (milliseconds).
    #[serde(rename = "streaming")]
    Streaming {
        text: String,
        /// Delay between chunks in milliseconds.
        #[serde(default = "default_chunk_delay")]
        chunk_delay_ms: u64,
        /// Approximate characters per chunk.
        #[serde(default = "default_chunk_size")]
        chunk_size: usize,
    },

    /// Return a tool call response.
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },

    /// Return an error on the next call.
    #[serde(rename = "error")]
    Error { message: String },

    /// Return a sequence of patterns, advancing through them on each call.
    #[serde(rename = "sequence")]
    Sequence { patterns: Vec<ResponsePattern> },
}

fn default_chunk_delay() -> u64 {
    10
}

fn default_chunk_size() -> usize {
    20
}

impl Default for ResponsePattern {
    fn default() -> Self {
        Self::Text {
            text: "Hello from FakeAI".to_string(),
        }
    }
}

/// Configuration for the FakeAI handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FakeAiConfig {
    /// Unique identifier for this FakeAI instance.
    pub id: String,

    /// Model ID to report.
    #[serde(default = "default_model_id")]
    pub model_id: String,

    /// The response pattern to use.
    #[serde(default)]
    pub response_pattern: ResponsePattern,

    /// Fixed number to return from `count_tokens`.
    #[serde(default = "default_token_count")]
    pub fake_token_count: u64,

    /// Fixed string to return from `complete_prompt`.
    #[serde(default = "default_complete_response")]
    pub complete_response: String,

    /// Optional delay (ms) to inject before every operation to simulate
    /// network latency.
    #[serde(default)]
    pub delay_ms: u64,
}

fn default_model_id() -> String {
    "fake-ai-model".to_string()
}

fn default_token_count() -> u64 {
    42
}

fn default_complete_response() -> String {
    "fake completion".to_string()
}

impl Default for FakeAiConfig {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            model_id: default_model_id(),
            response_pattern: ResponsePattern::default(),
            fake_token_count: default_token_count(),
            complete_response: default_complete_response(),
            delay_ms: 0,
        }
    }
}