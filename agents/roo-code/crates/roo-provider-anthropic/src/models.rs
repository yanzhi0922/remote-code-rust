//! Anthropic model definitions.

use std::collections::HashMap;
use roo_types::model::{ModelInfo, ModelTier};

/// Default Anthropic model ID.
pub const DEFAULT_MODEL_ID: &str = "claude-sonnet-4-5";

/// Returns the supported Anthropic models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "claude-sonnet-4-6".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Anthropic Claude Sonnet 4.6".to_string()),
            tiers: Some(vec![ModelTier {
                name: None,
                context_window: 1_000_000,
                input_price: Some(6.0),
                output_price: Some(22.5),
                cache_writes_price: Some(7.5),
                cache_reads_price: Some(0.6),
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-sonnet-4-5".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Anthropic Claude Sonnet 4.5".to_string()),
            tiers: Some(vec![ModelTier {
                name: None,
                context_window: 1_000_000,
                input_price: Some(6.0),
                output_price: Some(22.5),
                cache_writes_price: Some(7.5),
                cache_reads_price: Some(0.6),
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-sonnet-4-20250514".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Anthropic Claude Sonnet 4".to_string()),
            tiers: Some(vec![ModelTier {
                name: None,
                context_window: 1_000_000,
                input_price: Some(6.0),
                output_price: Some(22.5),
                cache_writes_price: Some(7.5),
                cache_reads_price: Some(0.6),
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-6".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(5.0),
            output_price: Some(25.0),
            cache_writes_price: Some(6.25),
            cache_reads_price: Some(0.5),
            supports_reasoning_budget: Some(true),
            description: Some("Anthropic Claude Opus 4.6".to_string()),
            tiers: Some(vec![ModelTier {
                name: None,
                context_window: 1_000_000,
                input_price: Some(10.0),
                output_price: Some(37.5),
                cache_writes_price: Some(12.5),
                cache_reads_price: Some(1.0),
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-5-20251101".to_string(),
        ModelInfo {
            max_tokens: Some(32_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(5.0),
            output_price: Some(25.0),
            cache_writes_price: Some(6.25),
            cache_reads_price: Some(0.5),
            supports_reasoning_budget: Some(true),
            description: Some("Anthropic Claude Opus 4.5".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-1-20250805".to_string(),
        ModelInfo {
            max_tokens: Some(32_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            supports_reasoning_budget: Some(true),
            description: Some("Anthropic Claude Opus 4.1".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-20250514".to_string(),
        ModelInfo {
            max_tokens: Some(32_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            supports_reasoning_budget: Some(true),
            description: Some("Anthropic Claude Opus 4".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-7-sonnet-20250219:thinking".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            required_reasoning_budget: Some(true),
            description: Some("Anthropic Claude 3.7 Sonnet (thinking)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-7-sonnet-20250219".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude 3.7 Sonnet".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-sonnet-20241022".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude 3.5 Sonnet (v2)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-haiku-20241022".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(1.0),
            output_price: Some(5.0),
            cache_writes_price: Some(1.25),
            cache_reads_price: Some(0.1),
            description: Some("Anthropic Claude 3.5 Haiku".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-opus-20240229".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            description: Some("Anthropic Claude 3 Opus".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-haiku-20240307".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.25),
            output_price: Some(1.25),
            cache_writes_price: Some(0.3),
            cache_reads_price: Some(0.03),
            description: Some("Anthropic Claude 3 Haiku".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-haiku-4-5-20251001".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.0),
            output_price: Some(5.0),
            cache_writes_price: Some(1.25),
            cache_reads_price: Some(0.1),
            supports_reasoning_budget: Some(true),
            description: Some(
                "Claude Haiku 4.5 delivers near-frontier intelligence at lightning speeds with \
                 extended thinking, vision, and multilingual support."
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
