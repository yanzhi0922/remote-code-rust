//! Baseten model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Baseten model ID.
pub const DEFAULT_MODEL_ID: &str = "zai-org/GLM-4.6";

/// Returns the supported Baseten models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "moonshotai/Kimi-K2-Thinking".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 262_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.6),
            output_price: Some(2.5),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Kimi K2 Thinking - A model with enhanced reasoning capabilities from Kimi K2"
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "zai-org/GLM-4.6".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.6),
            output_price: Some(2.2),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Frontier open model with advanced agentic, reasoning and coding capabilities"
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-ai/DeepSeek-R1".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 163_840,
            supports_images: Some(false),
            supports_prompt_cache: false,
            supports_reasoning_budget: Some(true),
            input_price: Some(2.55),
            output_price: Some(5.95),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "DeepSeek's first-generation reasoning model".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-ai/DeepSeek-R1-0528".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 163_840,
            supports_images: Some(false),
            supports_prompt_cache: false,
            supports_reasoning_budget: Some(true),
            input_price: Some(2.55),
            output_price: Some(5.95),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "The latest revision of DeepSeek's first-generation reasoning model".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-ai/DeepSeek-V3-0324".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 163_840,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.77),
            output_price: Some(0.77),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Fast general-purpose LLM with enhanced reasoning capabilities".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-ai/DeepSeek-V3.1".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 163_840,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.5),
            output_price: Some(1.5),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Extremely capable general-purpose LLM with hybrid reasoning capabilities and \
                 advanced tool calling"
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-ai/DeepSeek-V3.2".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 163_840,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.3),
            output_price: Some(0.45),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "DeepSeek's hybrid reasoning model with efficient long context scaling with \
                 GPT-5 level performance"
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "openai/gpt-oss-120b".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_072,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.1),
            output_price: Some(0.5),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Extremely capable general-purpose LLM with strong, controllable reasoning capabilities"
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "Qwen/Qwen3-235B-A22B-Instruct-2507".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.22),
            output_price: Some(0.8),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Mixture-of-experts LLM with math and reasoning capabilities".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "Qwen/Qwen3-Coder-480B-A35B-Instruct".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.38),
            output_price: Some(1.53),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Mixture-of-experts LLM with advanced coding and reasoning capabilities"
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "moonshotai/Kimi-K2-Instruct-0905".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 262_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.6),
            output_price: Some(2.5),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "State of the art language model for agentic and coding tasks. September Update."
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
