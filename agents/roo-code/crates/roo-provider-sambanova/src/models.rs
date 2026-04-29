//! SambaNova model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default SambaNova model ID.
pub const DEFAULT_MODEL_ID: &str = "Meta-Llama-3.3-70B-Instruct";

/// Returns the supported SambaNova models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "Meta-Llama-3.1-8B-Instruct".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 16384,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.1),
            output_price: Some(0.2),
            description: Some(
                "Meta Llama 3.1 8B Instruct model with 16K context window."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "Meta-Llama-3.3-70B-Instruct".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.6),
            output_price: Some(1.2),
            description: Some(
                "Meta Llama 3.3 70B Instruct model with 128K context window."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "DeepSeek-R1".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 32768,
            supports_images: Some(false),
            supports_prompt_cache: false,
            supports_reasoning_budget: Some(true),
            input_price: Some(5.0),
            output_price: Some(7.0),
            description: Some(
                "DeepSeek R1 reasoning model with 32K context window."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "DeepSeek-V3-0324".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 32768,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(3.0),
            output_price: Some(4.5),
            description: Some(
                "DeepSeek V3 model with 32K context window."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "DeepSeek-V3.1".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 32768,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(3.0),
            output_price: Some(4.5),
            description: Some(
                "DeepSeek V3.1 model with 32K context window."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "Llama-4-Maverick-17B-128E-Instruct".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.63),
            output_price: Some(1.8),
            description: Some(
                "Meta Llama 4 Maverick 17B 128E Instruct model with 128K context window."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "Qwen3-32B".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 8192,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.4),
            output_price: Some(0.8),
            description: Some(
                "Alibaba Qwen 3 32B model with 8K context window."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-oss-120b".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.22),
            output_price: Some(0.59),
            description: Some(
                "OpenAI gpt oss 120b model with 128k context window."
                    .to_string(),
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
