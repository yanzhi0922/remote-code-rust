//! Vercel AI Gateway model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Vercel AI Gateway model ID.
pub const DEFAULT_MODEL_ID: &str = "anthropic/claude-sonnet-4";

/// Returns the supported Vercel AI Gateway fallback models.
///
/// Vercel AI Gateway provides access to models from various providers.
/// These are fallback models used when dynamic fetching is not available.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "anthropic/claude-sonnet-4".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.30),
            description: Some("Claude Sonnet 4 via Vercel AI Gateway".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic/claude-3.5-sonnet".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            description: Some("Claude 3.5 Sonnet via Vercel AI Gateway".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "openai/gpt-4o".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 128000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(2.50),
            output_price: Some(10.0),
            description: Some("GPT-4o via Vercel AI Gateway".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "google/gemini-2.5-pro".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 1048576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.25),
            output_price: Some(10.0),
            description: Some("Gemini 2.5 Pro via Vercel AI Gateway".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
