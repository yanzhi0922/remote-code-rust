//! xAI/Grok model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default xAI model ID.
pub const DEFAULT_MODEL_ID: &str = "grok-4.20";

/// Returns the supported xAI models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "grok-4.20".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 2_000_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.0),
            output_price: Some(6.0),
            cache_writes_price: Some(0.5),
            cache_reads_price: Some(0.5),
            description: Some(
                "xAI's flagship Grok 4.20 model with 2M context and reasoning support via \
                 Responses API."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "grok-code-fast-1".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 256_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.2),
            output_price: Some(1.5),
            cache_writes_price: Some(0.02),
            cache_reads_price: Some(0.02),
            description: Some(
                "xAI's Grok Code Fast model with 256K context window.".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "grok-4-1-fast-reasoning".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 2_000_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.2),
            output_price: Some(0.5),
            cache_writes_price: Some(0.05),
            cache_reads_price: Some(0.05),
            description: Some(
                "xAI's Grok 4.1 Fast model with 2M context window, optimized for \
                 high-performance agentic tool calling with reasoning."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "grok-4-1-fast-non-reasoning".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 2_000_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.2),
            output_price: Some(0.5),
            cache_writes_price: Some(0.05),
            cache_reads_price: Some(0.05),
            description: Some(
                "xAI's Grok 4.1 Fast model with 2M context window, optimized for \
                 high-performance agentic tool calling."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "grok-4-fast-reasoning".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 2_000_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.2),
            output_price: Some(0.5),
            cache_writes_price: Some(0.05),
            cache_reads_price: Some(0.05),
            description: Some(
                "xAI's Grok 4 Fast model with 2M context window, optimized for \
                 high-performance agentic tool calling with reasoning."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "grok-4-fast-non-reasoning".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 2_000_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.2),
            output_price: Some(0.5),
            cache_writes_price: Some(0.05),
            cache_reads_price: Some(0.05),
            description: Some(
                "xAI's Grok 4 Fast model with 2M context window, optimized for \
                 high-performance agentic tool calling."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "grok-4-0709".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 256_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(0.75),
            cache_reads_price: Some(0.75),
            description: Some(
                "xAI's Grok-4 model with 256K context window.".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "grok-3-mini".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(0.3),
            output_price: Some(0.5),
            cache_writes_price: Some(0.07),
            cache_reads_price: Some(0.07),
            description: Some(
                "xAI's Grok-3 mini model with 128K context window.".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "grok-3".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(0.75),
            cache_reads_price: Some(0.75),
            description: Some(
                "xAI's Grok-3 model with 128K context window.".to_string(),
            ),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
