//! MiniMax model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default MiniMax model ID.
pub const DEFAULT_MODEL_ID: &str = "MiniMax-M2.7";

/// Returns the supported MiniMax models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "MiniMax-M2.5".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2.5, the latest MiniMax model with enhanced coding and agentic \
                 capabilities, building on the strengths of the M2 series."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.5-highspeed".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.4),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2.5 highspeed: same performance as M2.5 but with faster response \
                 (approximately 100 tps vs 60 tps)."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.7".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.06),
            description: Some(
                "MiniMax M2.7, the latest MiniMax model with recursive self-improvement capabilities."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.7-highspeed".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.4),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.06),
            description: Some(
                "MiniMax M2.7 highspeed: same performance as M2.7 but with faster response \
                 (approximately 100 tps vs 60 tps)."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2, a model born for Agents and code, featuring Top-tier Coding \
                 Capabilities, Powerful Agentic Performance, and Ultimate Cost-Effectiveness & Speed."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2-Stable".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2 Stable (High Concurrency, Commercial Use), a model born for \
                 Agents and code."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.1".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2.1 builds on M2 with improved overall performance for agentic \
                 coding tasks and significantly faster response times."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.1-highspeed".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.4),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2.1 highspeed: same performance as M2.1 but with faster response \
                 (approximately 100 tps vs 60 tps)."
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
