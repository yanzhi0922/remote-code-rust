//! OpenRouter model definitions.
//!
//! OpenRouter provides access to many models through a unified API.
//! We define a subset of popular models with their pricing.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default OpenRouter model ID.
pub const DEFAULT_MODEL_ID: &str = "anthropic/claude-sonnet-4.5";

/// Returns commonly used OpenRouter models with their pricing.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "anthropic/claude-sonnet-4.5".to_string(),
        ModelInfo {
            max_tokens: Some(64000),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Anthropic Claude Sonnet 4.5 via OpenRouter".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic/claude-sonnet-4".to_string(),
        ModelInfo {
            max_tokens: Some(16384),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude Sonnet 4 via OpenRouter".to_string()),
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
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude 3.5 Sonnet via OpenRouter".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "openai/gpt-4o".to_string(),
        ModelInfo {
            max_tokens: Some(16384),
            context_window: 128000,
            supports_images: Some(true),
            input_price: Some(2.5),
            output_price: Some(10.0),
            description: Some("OpenAI GPT-4o via OpenRouter".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "google/gemini-2.5-pro-preview".to_string(),
        ModelInfo {
            max_tokens: Some(65536),
            context_window: 1048576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.25),
            output_price: Some(10.0),
            description: Some("Google Gemini 2.5 Pro via OpenRouter".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek/deepseek-chat".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 65536,
            supports_prompt_cache: true,
            input_price: Some(0.27),
            output_price: Some(1.10),
            description: Some("DeepSeek Chat via OpenRouter".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta-llama/llama-3.3-70b-instruct".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131072,
            input_price: Some(0.39),
            output_price: Some(0.39),
            description: Some("Meta Llama 3.3 70B Instruct via OpenRouter".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
