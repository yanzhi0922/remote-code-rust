//! LiteLLM model definitions.
//!
//! These are static fallback models. In production, models are fetched
//! dynamically from the LiteLLM server's `/v1/model/info` endpoint.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default LiteLLM model ID.
pub const DEFAULT_MODEL_ID: &str = "claude-3-7-sonnet-20250219";

/// Returns the supported LiteLLM fallback models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "gpt-4o".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 128000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(2.50),
            output_price: Some(10.0),
            description: Some("GPT-4o via LiteLLM proxy".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-sonnet-20241022".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.30),
            description: Some("Claude 3.5 Sonnet via LiteLLM proxy".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-chat".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 65536,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.27),
            output_price: Some(1.10),
            description: Some("DeepSeek-V3 via LiteLLM proxy".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-7-sonnet-20250219".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.30),
            description: Some("Claude 3.7 Sonnet via LiteLLM proxy".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-pro".to_string(),
        ModelInfo {
            max_tokens: Some(16384),
            context_window: 1048576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.25),
            output_price: Some(10.0),
            description: Some("Gemini 2.5 Pro via LiteLLM proxy".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
