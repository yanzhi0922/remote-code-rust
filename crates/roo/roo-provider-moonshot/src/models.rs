//! Moonshot / Kimi AI model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Moonshot model ID.
pub const DEFAULT_MODEL_ID: &str = "kimi-k2-0905-preview";

/// Returns the supported Moonshot models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "kimi-k2-0711-preview".to_string(),
        ModelInfo {
            max_tokens: Some(32_000),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.5),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.15),
            description: Some(
                "Kimi K2 is a state-of-the-art mixture-of-experts (MoE) language model \
                 with 32 billion activated parameters and 1 trillion total parameters."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "kimi-k2-0905-preview".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.5),
            cache_reads_price: Some(0.15),
            description: Some(
                "Kimi K2 model gets a new version update: Agentic coding: more accurate, \
                 better generalization across scaffolds. Frontend coding: improved aesthetics \
                 and functionalities on web, 3d, and other tasks. Context length: extended \
                 from 128k to 256k, providing better long-horizon support."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "kimi-k2-turbo-preview".to_string(),
        ModelInfo {
            max_tokens: Some(32_000),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(2.4),
            output_price: Some(10.0),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.6),
            description: Some(
                "Kimi K2 Turbo is a high-speed version of the state-of-the-art Kimi K2 \
                 mixture-of-experts (MoE) language model, with the same 32 billion activated \
                 parameters and 1 trillion total parameters, optimized for output speeds of \
                 up to 60 tokens per second, peaking at 100 tokens per second."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "kimi-k2-thinking".to_string(),
        ModelInfo {
            max_tokens: Some(16_000),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(0.6),
            output_price: Some(2.5),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.15),
            description: Some(
                "The kimi-k2-thinking model is a general-purpose agentic reasoning model \
                 developed by Moonshot AI. Thanks to its strength in deep reasoning and \
                 multi-turn tool use, it can solve even the hardest problems."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "kimi-k2.5".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(3.0),
            cache_reads_price: Some(0.1),
            description: Some(
                "Kimi K2.5 is the latest generation of Moonshot AI's Kimi series, \
                 featuring improved reasoning capabilities and enhanced performance across \
                 diverse tasks."
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
