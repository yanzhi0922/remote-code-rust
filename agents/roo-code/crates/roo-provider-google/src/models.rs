//! Google Gemini and Vertex AI model definitions.

use std::collections::HashMap;
use roo_types::model::{ModelInfo, ModelTier, ReasoningEffortExtended};

/// Default Gemini model ID.
pub const DEFAULT_MODEL_ID: &str = "gemini-3.1-pro-preview";

/// Returns the supported Google Gemini models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    // --- Gemini 3.x ---
    m.insert(
        "gemini-3.1-pro-preview".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            supports_temperature: Some(true),
            default_temperature: Some(1.0),
            input_price: Some(4.0),
            output_price: Some(18.0),
            cache_reads_price: Some(0.4),
            cache_writes_price: Some(4.5),
            description: Some("Google Gemini 3.1 Pro Preview".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 200_000,
                    input_price: Some(2.0),
                    output_price: Some(12.0),
                    cache_reads_price: Some(0.2),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: None,
                    context_window: u64::MAX,
                    input_price: Some(4.0),
                    output_price: Some(18.0),
                    cache_reads_price: Some(0.4),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-3.1-pro-preview-customtools".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            supports_temperature: Some(true),
            default_temperature: Some(1.0),
            input_price: Some(4.0),
            output_price: Some(18.0),
            cache_reads_price: Some(0.4),
            cache_writes_price: Some(4.5),
            description: Some("Google Gemini 3.1 Pro Preview (custom tools)".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 200_000,
                    input_price: Some(2.0),
                    output_price: Some(12.0),
                    cache_reads_price: Some(0.2),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: None,
                    context_window: u64::MAX,
                    input_price: Some(4.0),
                    output_price: Some(18.0),
                    cache_reads_price: Some(0.4),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-3-pro-preview".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            supports_temperature: Some(true),
            default_temperature: Some(1.0),
            input_price: Some(4.0),
            output_price: Some(18.0),
            cache_reads_price: Some(0.4),
            description: Some("Google Gemini 3 Pro Preview".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 200_000,
                    input_price: Some(2.0),
                    output_price: Some(12.0),
                    cache_reads_price: Some(0.2),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: None,
                    context_window: u64::MAX,
                    input_price: Some(4.0),
                    output_price: Some(18.0),
                    cache_reads_price: Some(0.4),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-3-flash-preview".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            supports_temperature: Some(true),
            default_temperature: Some(1.0),
            input_price: Some(0.5),
            output_price: Some(3.0),
            cache_reads_price: Some(0.05),
            description: Some("Google Gemini 3 Flash Preview".to_string()),
            ..Default::default()
        },
    );

    // --- Gemini 2.5 Pro ---
    m.insert(
        "gemini-2.5-pro".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.5),
            output_price: Some(15.0),
            cache_writes_price: Some(4.5),
            cache_reads_price: Some(0.625),
            max_thinking_tokens: Some(32_768),
            supports_reasoning_budget: Some(true),
            required_reasoning_budget: Some(true),
            description: Some("Google Gemini 2.5 Pro".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 200_000,
                    input_price: Some(1.25),
                    output_price: Some(10.0),
                    cache_reads_price: Some(0.31),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: None,
                    context_window: u64::MAX,
                    input_price: Some(2.5),
                    output_price: Some(15.0),
                    cache_reads_price: Some(0.625),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-pro-preview-06-05".to_string(),
        ModelInfo {
            max_tokens: Some(65_535),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.5),
            output_price: Some(15.0),
            cache_writes_price: Some(4.5),
            cache_reads_price: Some(0.625),
            max_thinking_tokens: Some(32_768),
            supports_reasoning_budget: Some(true),
            description: Some("Google Gemini 2.5 Pro Preview 06-05".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 200_000,
                    input_price: Some(1.25),
                    output_price: Some(10.0),
                    cache_reads_price: Some(0.31),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: None,
                    context_window: u64::MAX,
                    input_price: Some(2.5),
                    output_price: Some(15.0),
                    cache_reads_price: Some(0.625),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-pro-preview-05-06".to_string(),
        ModelInfo {
            max_tokens: Some(65_535),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.5),
            output_price: Some(15.0),
            cache_writes_price: Some(4.5),
            cache_reads_price: Some(0.625),
            description: Some("Google Gemini 2.5 Pro Preview 05-06".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 200_000,
                    input_price: Some(1.25),
                    output_price: Some(10.0),
                    cache_reads_price: Some(0.31),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: None,
                    context_window: u64::MAX,
                    input_price: Some(2.5),
                    output_price: Some(15.0),
                    cache_reads_price: Some(0.625),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-pro-preview-03-25".to_string(),
        ModelInfo {
            max_tokens: Some(65_535),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.5),
            output_price: Some(15.0),
            cache_writes_price: Some(4.5),
            cache_reads_price: Some(0.625),
            max_thinking_tokens: Some(32_768),
            supports_reasoning_budget: Some(true),
            description: Some("Google Gemini 2.5 Pro Preview 03-25".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 200_000,
                    input_price: Some(1.25),
                    output_price: Some(10.0),
                    cache_reads_price: Some(0.31),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: None,
                    context_window: u64::MAX,
                    input_price: Some(2.5),
                    output_price: Some(15.0),
                    cache_reads_price: Some(0.625),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    // --- Gemini 2.5 Flash ---
    m.insert(
        "gemini-flash-latest".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(2.5),
            cache_writes_price: Some(1.0),
            cache_reads_price: Some(0.075),
            max_thinking_tokens: Some(24_576),
            supports_reasoning_budget: Some(true),
            description: Some("Google Gemini Flash Latest".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-flash-preview-09-2025".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(2.5),
            cache_writes_price: Some(1.0),
            cache_reads_price: Some(0.075),
            max_thinking_tokens: Some(24_576),
            supports_reasoning_budget: Some(true),
            description: Some("Google Gemini 2.5 Flash Preview 09-2025".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-flash".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(2.5),
            cache_writes_price: Some(1.0),
            cache_reads_price: Some(0.075),
            max_thinking_tokens: Some(24_576),
            supports_reasoning_budget: Some(true),
            description: Some("Google Gemini 2.5 Flash".to_string()),
            ..Default::default()
        },
    );

    // --- Gemini 2.5 Flash Lite ---
    m.insert(
        "gemini-flash-lite-latest".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.1),
            output_price: Some(0.4),
            cache_writes_price: Some(1.0),
            cache_reads_price: Some(0.025),
            supports_reasoning_budget: Some(true),
            max_thinking_tokens: Some(24_576),
            description: Some("Google Gemini Flash Lite Latest".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-flash-lite-preview-09-2025".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.1),
            output_price: Some(0.4),
            cache_writes_price: Some(1.0),
            cache_reads_price: Some(0.025),
            supports_reasoning_budget: Some(true),
            max_thinking_tokens: Some(24_576),
            description: Some("Google Gemini 2.5 Flash Lite Preview 09-2025".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}

// ---------------------------------------------------------------------------
// Vertex AI models
// ---------------------------------------------------------------------------

/// Default Vertex AI model ID.
pub const VERTEX_DEFAULT_MODEL_ID: &str = "claude-sonnet-4-5@20250929";

/// Returns the supported Vertex AI models.
///
/// Source: `packages/types/src/providers/vertex.ts` — `vertexModels`
/// Includes Gemini, Claude, and third-party models available on Vertex AI.
pub fn vertex_models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    // --- Gemini 3.x ---
    m.insert(
        "gemini-3.1-pro-preview".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            supports_temperature: Some(true),
            default_temperature: Some(1.0),
            input_price: Some(4.0),
            output_price: Some(18.0),
            cache_reads_price: Some(0.4),
            cache_writes_price: Some(4.5),
            description: Some("Gemini 3.1 Pro Preview (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-3-pro-preview".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            supports_temperature: Some(true),
            default_temperature: Some(1.0),
            input_price: Some(4.0),
            output_price: Some(18.0),
            cache_reads_price: Some(0.4),
            description: Some("Gemini 3 Pro Preview (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-3-flash-preview".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            supports_temperature: Some(true),
            default_temperature: Some(1.0),
            input_price: Some(0.5),
            output_price: Some(3.0),
            cache_reads_price: Some(0.05),
            description: Some("Gemini 3 Flash Preview (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    // --- Gemini 2.5 ---
    m.insert(
        "gemini-2.5-flash-preview-05-20:thinking".to_string(),
        ModelInfo {
            max_tokens: Some(65_535),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.15),
            output_price: Some(3.5),
            max_thinking_tokens: Some(24_576),
            supports_reasoning_budget: Some(true),
            required_reasoning_budget: Some(true),
            description: Some("Gemini 2.5 Flash Preview 05-20 Thinking (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-flash-preview-05-20".to_string(),
        ModelInfo {
            max_tokens: Some(65_535),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.15),
            output_price: Some(0.6),
            description: Some("Gemini 2.5 Flash Preview 05-20 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-flash".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(2.5),
            cache_reads_price: Some(0.075),
            cache_writes_price: Some(1.0),
            max_thinking_tokens: Some(24_576),
            supports_reasoning_budget: Some(true),
            description: Some("Gemini 2.5 Flash (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-pro-preview-05-06".to_string(),
        ModelInfo {
            max_tokens: Some(65_535),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.5),
            output_price: Some(15.0),
            description: Some("Gemini 2.5 Pro Preview 05-06 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-pro-preview-06-05".to_string(),
        ModelInfo {
            max_tokens: Some(65_535),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.5),
            output_price: Some(15.0),
            max_thinking_tokens: Some(32_768),
            supports_reasoning_budget: Some(true),
            description: Some("Gemini 2.5 Pro Preview 06-05 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-pro".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.5),
            output_price: Some(15.0),
            max_thinking_tokens: Some(32_768),
            supports_reasoning_budget: Some(true),
            required_reasoning_budget: Some(true),
            description: Some("Gemini 2.5 Pro (Vertex AI)".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 200_000,
                    input_price: Some(1.25),
                    output_price: Some(10.0),
                    cache_reads_price: Some(0.31),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: None,
                    context_window: u64::MAX,
                    input_price: Some(2.5),
                    output_price: Some(15.0),
                    cache_reads_price: Some(0.625),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.5-flash-lite-preview-06-17".to_string(),
        ModelInfo {
            max_tokens: Some(64_000),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.1),
            output_price: Some(0.4),
            cache_reads_price: Some(0.025),
            cache_writes_price: Some(1.0),
            max_thinking_tokens: Some(24_576),
            supports_reasoning_budget: Some(true),
            description: Some("Gemini 2.5 Flash Lite Preview 06-17 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    // --- Gemini 2.0 ---
    m.insert(
        "gemini-2.0-flash-001".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.15),
            output_price: Some(0.6),
            description: Some("Gemini 2.0 Flash 001 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.0-flash-lite-001".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 1_048_576,
            supports_images: Some(true),
            input_price: Some(0.075),
            output_price: Some(0.3),
            description: Some("Gemini 2.0 Flash Lite 001 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-2.0-pro-exp-02-05".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 2_097_152,
            supports_images: Some(true),
            input_price: Some(0.0),
            output_price: Some(0.0),
            description: Some("Gemini 2.0 Pro Exp 02-05 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    // --- Gemini 1.5 ---
    m.insert(
        "gemini-1.5-flash-002".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 1_048_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.075),
            output_price: Some(0.3),
            description: Some("Gemini 1.5 Flash 002 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gemini-1.5-pro-002".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 2_097_152,
            supports_images: Some(true),
            input_price: Some(1.25),
            output_price: Some(5.0),
            description: Some("Gemini 1.5 Pro 002 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    // --- Claude models on Vertex AI ---
    m.insert(
        "claude-sonnet-4@20250514".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Sonnet 4 (Vertex AI)".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 1_000_000,
                    input_price: Some(6.0),
                    output_price: Some(22.5),
                    cache_writes_price: Some(7.5),
                    cache_reads_price: Some(0.6),
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-sonnet-4-5@20250929".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Sonnet 4.5 (Vertex AI)".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 1_000_000,
                    input_price: Some(6.0),
                    output_price: Some(22.5),
                    cache_writes_price: Some(7.5),
                    cache_reads_price: Some(0.6),
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-sonnet-4-6".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Sonnet 4.6 (Vertex AI)".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 1_000_000,
                    input_price: Some(6.0),
                    output_price: Some(22.5),
                    cache_writes_price: Some(7.5),
                    cache_reads_price: Some(0.6),
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-haiku-4-5@20251001".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.0),
            output_price: Some(5.0),
            cache_writes_price: Some(1.25),
            cache_reads_price: Some(0.1),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Haiku 4.5 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-6".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(5.0),
            output_price: Some(25.0),
            cache_writes_price: Some(6.25),
            cache_reads_price: Some(0.5),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Opus 4.6 (Vertex AI)".to_string()),
            tiers: Some(vec![
                ModelTier {
                    name: None,
                    context_window: 1_000_000,
                    input_price: Some(10.0),
                    output_price: Some(37.5),
                    cache_writes_price: Some(12.5),
                    cache_reads_price: Some(1.0),
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-5@20251101".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(5.0),
            output_price: Some(25.0),
            cache_writes_price: Some(6.25),
            cache_reads_price: Some(0.5),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Opus 4.5 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4-1@20250805".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            supports_reasoning_budget: Some(true),
            description: Some("Claude Opus 4.1 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-opus-4@20250514".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            description: Some("Claude Opus 4 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-7-sonnet@20250219:thinking".to_string(),
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
            required_reasoning_budget: Some(true),
            description: Some("Claude 3.7 Sonnet Thinking (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-7-sonnet@20250219".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Claude 3.7 Sonnet (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-sonnet-v2@20241022".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Claude 3.5 Sonnet v2 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-sonnet@20240620".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Claude 3.5 Sonnet (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-haiku@20241022".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(1.0),
            output_price: Some(5.0),
            cache_writes_price: Some(1.25),
            cache_reads_price: Some(0.1),
            description: Some("Claude 3.5 Haiku (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-opus@20240229".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            description: Some("Claude 3 Opus (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-haiku@20240307".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.25),
            output_price: Some(1.25),
            cache_writes_price: Some(0.3),
            cache_reads_price: Some(0.03),
            description: Some("Claude 3 Haiku (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    // --- Third-party models on Vertex AI ---
    m.insert(
        "llama-4-maverick-17b-128e-instruct-maas".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_072,
            supports_images: Some(false),
            input_price: Some(0.35),
            output_price: Some(1.15),
            description: Some("Meta Llama 4 Maverick 17B Instruct (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-r1-0528-maas".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 163_840,
            supports_images: Some(false),
            input_price: Some(1.35),
            output_price: Some(5.4),
            description: Some("DeepSeek R1 0528 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-v3.1-maas".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 163_840,
            supports_images: Some(false),
            input_price: Some(0.6),
            output_price: Some(1.7),
            description: Some("DeepSeek V3.1 (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "qwen3-coder-480b-a35b-instruct-maas".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 262_144,
            supports_images: Some(false),
            input_price: Some(1.0),
            output_price: Some(4.0),
            description: Some("Qwen3 Coder 480B A35B Instruct (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "qwen3-235b-a22b-instruct-2507-maas".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 262_144,
            supports_images: Some(false),
            input_price: Some(0.25),
            output_price: Some(1.0),
            description: Some("Qwen3 235B A22B Instruct (Vertex AI)".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default Vertex AI model ID.
pub fn vertex_default_model_id() -> String {
    VERTEX_DEFAULT_MODEL_ID.to_string()
}
