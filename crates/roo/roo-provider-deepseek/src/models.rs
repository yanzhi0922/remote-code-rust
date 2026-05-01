//! DeepSeek model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default DeepSeek model ID.
pub const DEFAULT_MODEL_ID: &str = "deepseek-chat";

/// Returns the supported DeepSeek models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "deepseek-chat".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.28),
            output_price: Some(0.42),
            cache_writes_price: Some(0.28),
            cache_reads_price: Some(0.028),
            description: Some(
                "DeepSeek-V3.2 (Non-thinking Mode) achieves a significant breakthrough in \
                 inference speed over previous models. Supports JSON output, tool calls, \
                 chat prefix completion (beta), and FIM completion (beta)."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-reasoner".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(0.28),
            output_price: Some(0.42),
            cache_writes_price: Some(0.28),
            cache_reads_price: Some(0.028),
            description: Some(
                "DeepSeek-V3.2 (Thinking Mode) achieves performance comparable to OpenAI-o1 \
                 across math, code, and reasoning tasks. Supports Chain of Thought reasoning \
                 with up to 8K output tokens. Supports JSON output, tool calls, and chat \
                 prefix completion (beta)."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-chat-v3-0324".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131072,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.27),
            output_price: Some(1.10),
            cache_writes_price: Some(0.27),
            cache_reads_price: Some(0.07),
            description: Some("DeepSeek-V3-0324 chat model".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-r1-0528".to_string(),
        ModelInfo {
            max_tokens: Some(16384),
            context_window: 131072,
            supports_images: Some(false),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(0.55),
            output_price: Some(2.19),
            cache_writes_price: Some(0.55),
            cache_reads_price: Some(0.14),
            description: Some("DeepSeek-R1-0528 reasoning model".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
