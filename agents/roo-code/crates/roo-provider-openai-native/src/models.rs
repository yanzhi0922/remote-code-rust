//! Model definitions for OpenAI Native and OpenAI Codex providers.
//!
//! Derived from `packages/types/src/providers/openai.ts` and
//! `packages/types/src/providers/openai-codex.ts`.

use std::collections::HashMap;
use roo_types::model::{ModelInfo, ModelTier, ReasoningEffortExtended, ServiceTier};

// ---------------------------------------------------------------------------
// OpenAI Native models
// ---------------------------------------------------------------------------

/// Default model ID for the OpenAI Native provider.
pub const OPENAI_NATIVE_DEFAULT_MODEL_ID: &str = "gpt-5.1-codex-max";

/// Default temperature for OpenAI Native models.
pub const OPENAI_NATIVE_DEFAULT_TEMPERATURE: f64 = 0.0;

/// Returns the supported OpenAI Native models.
///
/// Source: `packages/types/src/providers/openai.ts` — `openAiNativeModels`
pub fn openai_native_models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    // --- GPT-5.x family ---

    m.insert(
        "gpt-5.1-codex-max".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            prompt_cache_retention: Some("24h".to_string()),
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Xhigh),
            input_price: Some(1.25),
            output_price: Some(10.0),
            cache_reads_price: Some(0.125),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 400_000,
                input_price: Some(2.5),
                output_price: Some(20.0),
                cache_reads_price: Some(0.25),
                cache_writes_price: None,
            }]),
            description: Some("GPT-5.1 Codex Max: Our most intelligent coding model optimized for long-horizon, agentic coding tasks".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.4".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 1_050_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::None),
            input_price: Some(2.5),
            output_price: Some(15.0),
            cache_reads_price: Some(0.25),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 1_050_000,
                    input_price: Some(1.25),
                    output_price: Some(7.5),
                    cache_reads_price: Some(0.125),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 1_050_000,
                    input_price: Some(5.0),
                    output_price: Some(30.0),
                    cache_reads_price: Some(0.5),
                    cache_writes_price: None,
                },
            ]),
            description: Some("GPT-5.4: Our most capable model for professional work".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.4-mini".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::None),
            input_price: Some(0.75),
            output_price: Some(4.5),
            cache_reads_price: Some(0.075),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 400_000,
                    input_price: Some(0.375),
                    output_price: Some(2.25),
                    cache_reads_price: Some(0.0375),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 400_000,
                    input_price: Some(1.5),
                    output_price: Some(9.0),
                    cache_reads_price: Some(0.15),
                    cache_writes_price: None,
                },
            ]),
            description: Some("GPT-5.4 Mini: A faster, lower-cost GPT-5.4 model for coding and agentic workflows".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.4-nano".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::None),
            input_price: Some(0.2),
            output_price: Some(1.25),
            cache_reads_price: Some(0.02),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Flex),
                context_window: 400_000,
                input_price: Some(0.1),
                output_price: Some(0.625),
                cache_reads_price: Some(0.01),
                cache_writes_price: None,
            }]),
            description: Some("GPT-5.4 Nano: The smallest GPT-5.4 model for high-volume, low-latency tasks".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.2".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            prompt_cache_retention: Some("24h".to_string()),
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(1.75),
            output_price: Some(14.0),
            cache_reads_price: Some(0.175),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 400_000,
                    input_price: Some(0.875),
                    output_price: Some(7.0),
                    cache_reads_price: Some(0.0875),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 400_000,
                    input_price: Some(3.5),
                    output_price: Some(28.0),
                    cache_reads_price: Some(0.35),
                    cache_writes_price: None,
                },
            ]),
            description: Some("GPT-5.2: Our flagship model for coding and agentic tasks across industries".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.2-codex".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            prompt_cache_retention: Some("24h".to_string()),
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(1.75),
            output_price: Some(14.0),
            cache_reads_price: Some(0.175),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 400_000,
                input_price: Some(3.5),
                output_price: Some(28.0),
                cache_reads_price: Some(0.35),
                cache_writes_price: None,
            }]),
            description: Some("GPT-5.2 Codex: Our most intelligent coding model optimized for long-horizon, agentic coding tasks".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.3-codex".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            prompt_cache_retention: Some("24h".to_string()),
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(1.75),
            output_price: Some(14.0),
            cache_reads_price: Some(0.175),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 400_000,
                input_price: Some(3.5),
                output_price: Some(28.0),
                cache_reads_price: Some(0.35),
                cache_writes_price: None,
            }]),
            description: Some("GPT-5.3 Codex: Our most intelligent coding model optimized for long-horizon, agentic coding tasks".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.2-chat-latest".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.75),
            output_price: Some(14.0),
            cache_reads_price: Some(0.175),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.2 Chat: Optimized for conversational AI and chat use cases".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.3-chat-latest".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.75),
            output_price: Some(14.0),
            cache_reads_price: Some(0.175),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.3 Chat: Optimized for conversational AI and chat use cases".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.1".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            prompt_cache_retention: Some("24h".to_string()),
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(1.25),
            output_price: Some(10.0),
            cache_reads_price: Some(0.125),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 400_000,
                    input_price: Some(0.625),
                    output_price: Some(5.0),
                    cache_reads_price: Some(0.0625),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 400_000,
                    input_price: Some(2.5),
                    output_price: Some(20.0),
                    cache_reads_price: Some(0.25),
                    cache_writes_price: None,
                },
            ]),
            description: Some("GPT-5.1: The best model for coding and agentic tasks across domains".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.1-codex".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            prompt_cache_retention: Some("24h".to_string()),
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(1.25),
            output_price: Some(10.0),
            cache_reads_price: Some(0.125),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 400_000,
                input_price: Some(2.5),
                output_price: Some(20.0),
                cache_reads_price: Some(0.25),
                cache_writes_price: None,
            }]),
            description: Some("GPT-5.1 Codex: A version of GPT-5.1 optimized for agentic coding in Codex".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.1-codex-mini".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            prompt_cache_retention: Some("24h".to_string()),
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.25),
            output_price: Some(2.0),
            cache_reads_price: Some(0.025),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.1 Codex mini: A version of GPT-5.1 optimized for agentic coding in Codex".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(1.25),
            output_price: Some(10.0),
            cache_reads_price: Some(0.125),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 400_000,
                    input_price: Some(0.625),
                    output_price: Some(5.0),
                    cache_reads_price: Some(0.0625),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 400_000,
                    input_price: Some(2.5),
                    output_price: Some(20.0),
                    cache_reads_price: Some(0.25),
                    cache_writes_price: None,
                },
            ]),
            description: Some("GPT-5: The best model for coding and agentic tasks across domains".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5-mini".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.25),
            output_price: Some(2.0),
            cache_reads_price: Some(0.025),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 400_000,
                    input_price: Some(0.125),
                    output_price: Some(1.0),
                    cache_reads_price: Some(0.0125),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 400_000,
                    input_price: Some(0.45),
                    output_price: Some(3.6),
                    cache_reads_price: Some(0.045),
                    cache_writes_price: None,
                },
            ]),
            description: Some("GPT-5 Mini: A faster, more cost-efficient version of GPT-5 for well-defined tasks".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5-codex".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(1.25),
            output_price: Some(10.0),
            cache_reads_price: Some(0.125),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 400_000,
                input_price: Some(2.5),
                output_price: Some(20.0),
                cache_reads_price: Some(0.25),
                cache_writes_price: None,
            }]),
            description: Some("GPT-5-Codex: A version of GPT-5 optimized for agentic coding in Codex".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5-nano".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.05),
            output_price: Some(0.4),
            cache_reads_price: Some(0.005),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Flex),
                context_window: 400_000,
                input_price: Some(0.025),
                output_price: Some(0.2),
                cache_reads_price: Some(0.0025),
                cache_writes_price: None,
            }]),
            description: Some("GPT-5 Nano: Fastest, most cost-efficient version of GPT-5".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5-chat-latest".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.25),
            output_price: Some(10.0),
            cache_reads_price: Some(0.125),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5 Chat: Optimized for conversational AI and non-reasoning tasks".to_string()),
            ..Default::default()
        },
    );

    // --- GPT-4.1 family ---

    m.insert(
        "gpt-4.1".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 1_047_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.0),
            output_price: Some(8.0),
            cache_reads_price: Some(0.5),
            supports_temperature: Some(true),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 1_047_576,
                input_price: Some(3.5),
                output_price: Some(14.0),
                cache_reads_price: Some(0.875),
                cache_writes_price: None,
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-4.1-mini".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 1_047_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.4),
            output_price: Some(1.6),
            cache_reads_price: Some(0.1),
            supports_temperature: Some(true),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 1_047_576,
                input_price: Some(0.7),
                output_price: Some(2.8),
                cache_reads_price: Some(0.175),
                cache_writes_price: None,
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-4.1-nano".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 1_047_576,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.1),
            output_price: Some(0.4),
            cache_reads_price: Some(0.025),
            supports_temperature: Some(true),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 1_047_576,
                input_price: Some(0.2),
                output_price: Some(0.8),
                cache_reads_price: Some(0.05),
                cache_writes_price: None,
            }]),
            ..Default::default()
        },
    );

    // --- o-family reasoning models ---

    m.insert(
        "o3".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.0),
            output_price: Some(8.0),
            cache_reads_price: Some(0.5),
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            supports_temperature: Some(false),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 200_000,
                    input_price: Some(1.0),
                    output_price: Some(4.0),
                    cache_reads_price: Some(0.25),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 200_000,
                    input_price: Some(3.5),
                    output_price: Some(14.0),
                    cache_reads_price: Some(0.875),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "o3-high".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.0),
            output_price: Some(8.0),
            cache_reads_price: Some(0.5),
            reasoning_effort: Some(ReasoningEffortExtended::High),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o3-low".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.0),
            output_price: Some(8.0),
            cache_reads_price: Some(0.5),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o4-mini".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.1),
            output_price: Some(4.4),
            cache_reads_price: Some(0.275),
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            supports_temperature: Some(false),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 200_000,
                    input_price: Some(0.55),
                    output_price: Some(2.2),
                    cache_reads_price: Some(0.138),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 200_000,
                    input_price: Some(2.0),
                    output_price: Some(8.0),
                    cache_reads_price: Some(0.5),
                    cache_writes_price: None,
                },
            ]),
            ..Default::default()
        },
    );

    m.insert(
        "o4-mini-high".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.1),
            output_price: Some(4.4),
            cache_reads_price: Some(0.275),
            reasoning_effort: Some(ReasoningEffortExtended::High),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o4-mini-low".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.1),
            output_price: Some(4.4),
            cache_reads_price: Some(0.275),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o3-mini".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(1.1),
            output_price: Some(4.4),
            cache_reads_price: Some(0.55),
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o3-mini-high".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(1.1),
            output_price: Some(4.4),
            cache_reads_price: Some(0.55),
            reasoning_effort: Some(ReasoningEffortExtended::High),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o3-mini-low".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(1.1),
            output_price: Some(4.4),
            cache_reads_price: Some(0.55),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o1".to_string(),
        ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(60.0),
            cache_reads_price: Some(7.5),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o1-preview".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(15.0),
            output_price: Some(60.0),
            cache_reads_price: Some(7.5),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    m.insert(
        "o1-mini".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(1.1),
            output_price: Some(4.4),
            cache_reads_price: Some(0.55),
            supports_temperature: Some(false),
            ..Default::default()
        },
    );

    // --- GPT-4o family ---

    m.insert(
        "gpt-4o".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(2.5),
            output_price: Some(10.0),
            cache_reads_price: Some(1.25),
            supports_temperature: Some(true),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 128_000,
                input_price: Some(4.25),
                output_price: Some(17.0),
                cache_reads_price: Some(2.125),
                cache_writes_price: None,
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-4o-mini".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.15),
            output_price: Some(0.6),
            cache_reads_price: Some(0.075),
            supports_temperature: Some(true),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Priority),
                context_window: 128_000,
                input_price: Some(0.25),
                output_price: Some(1.0),
                cache_reads_price: Some(0.125),
                cache_writes_price: None,
            }]),
            ..Default::default()
        },
    );

    m.insert(
        "codex-mini-latest".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(1.5),
            output_price: Some(6.0),
            cache_reads_price: Some(0.375),
            supports_temperature: Some(false),
            description: Some(
                "Codex Mini: Cloud-based software engineering agent powered by codex-1, a version \
                 of o3 optimized for coding tasks. Trained with reinforcement learning to generate \
                 human-style code, adhere to instructions, and iteratively run tests."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    // --- Dated clones (snapshots) ---

    m.insert(
        "gpt-5-2025-08-07".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(1.25),
            output_price: Some(10.0),
            cache_reads_price: Some(0.125),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 400_000,
                    input_price: Some(0.625),
                    output_price: Some(5.0),
                    cache_reads_price: Some(0.0625),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 400_000,
                    input_price: Some(2.5),
                    output_price: Some(20.0),
                    cache_reads_price: Some(0.25),
                    cache_writes_price: None,
                },
            ]),
            description: Some("GPT-5: The best model for coding and agentic tasks across domains".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5-mini-2025-08-07".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.25),
            output_price: Some(2.0),
            cache_reads_price: Some(0.025),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![
                ModelTier {
                    name: Some(ServiceTier::Flex),
                    context_window: 400_000,
                    input_price: Some(0.125),
                    output_price: Some(1.0),
                    cache_reads_price: Some(0.0125),
                    cache_writes_price: None,
                },
                ModelTier {
                    name: Some(ServiceTier::Priority),
                    context_window: 400_000,
                    input_price: Some(0.45),
                    output_price: Some(3.6),
                    cache_reads_price: Some(0.045),
                    cache_writes_price: None,
                },
            ]),
            description: Some("GPT-5 Mini: A faster, more cost-efficient version of GPT-5 for well-defined tasks".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5-nano-2025-08-07".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.05),
            output_price: Some(0.4),
            cache_reads_price: Some(0.005),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            tiers: Some(vec![ModelTier {
                name: Some(ServiceTier::Flex),
                context_window: 400_000,
                input_price: Some(0.025),
                output_price: Some(0.2),
                cache_reads_price: Some(0.0025),
                cache_writes_price: None,
            }]),
            description: Some("GPT-5 Nano: Fastest, most cost-efficient version of GPT-5".to_string()),
            ..Default::default()
        },
    );

    m
}

// ---------------------------------------------------------------------------
// OpenAI Codex models
// ---------------------------------------------------------------------------

/// Default model ID for the OpenAI Codex provider.
pub const OPENAI_CODEX_DEFAULT_MODEL_ID: &str = "gpt-5.3-codex";

/// Returns the supported OpenAI Codex models.
///
/// Source: `packages/types/src/providers/openai-codex.ts` — `openAiCodexModels`
///
/// All Codex models are subscription-based (zero per-token cost).
pub fn openai_codex_models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "gpt-5.1-codex-max".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Xhigh),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.1 Codex Max: Maximum capability coding model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.3-codex".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.3 Codex: OpenAI's flagship coding model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.3-codex-spark".to_string(),
        ModelInfo {
            max_tokens: Some(8_192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.3 Codex Spark: Fast, text-only coding model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.2-codex".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.2 Codex: OpenAI's flagship coding model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.1-codex".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.1 Codex: GPT-5.1 optimized for agentic coding via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.1-codex-mini".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.1 Codex Mini: Faster version for coding tasks via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5-codex".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5 Codex: GPT-5 optimized for agentic coding via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5-codex-mini".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5 Codex Mini: Faster coding model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.1".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.1: General GPT-5.1 model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["minimal", "low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5: General GPT-5 model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.4".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 1_050_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::None),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.4: Most capable model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.4-mini".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::None),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_verbosity: Some(true),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.4 Mini: Lower-cost GPT-5.4 model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-5.2".to_string(),
        ModelInfo {
            max_tokens: Some(128_000),
            context_window: 400_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_effort: Some(serde_json::json!(["none", "low", "medium", "high", "xhigh"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            input_price: Some(0.0),
            output_price: Some(0.0),
            supports_temperature: Some(false),
            included_tools: Some(vec!["apply_patch".to_string()]),
            excluded_tools: Some(vec!["apply_diff".to_string(), "write_to_file".to_string()]),
            description: Some("GPT-5.2: Latest GPT model via ChatGPT subscription".to_string()),
            ..Default::default()
        },
    );

    m
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_default_model_exists() {
        let models = openai_native_models();
        assert!(
            models.contains_key(OPENAI_NATIVE_DEFAULT_MODEL_ID),
            "Default native model '{}' should exist",
            OPENAI_NATIVE_DEFAULT_MODEL_ID
        );
    }

    #[test]
    fn test_codex_default_model_exists() {
        let models = openai_codex_models();
        assert!(
            models.contains_key(OPENAI_CODEX_DEFAULT_MODEL_ID),
            "Default codex model '{}' should exist",
            OPENAI_CODEX_DEFAULT_MODEL_ID
        );
    }

    #[test]
    fn test_native_models_have_pricing() {
        for (id, info) in openai_native_models() {
            assert!(
                info.input_price.is_some(),
                "Native model '{}' missing input_price",
                id
            );
            assert!(
                info.output_price.is_some(),
                "Native model '{}' missing output_price",
                id
            );
        }
    }

    #[test]
    fn test_codex_models_are_free() {
        for (id, info) in openai_codex_models() {
            assert_eq!(
                info.input_price,
                Some(0.0),
                "Codex model '{}' should be free",
                id
            );
            assert_eq!(
                info.output_price,
                Some(0.0),
                "Codex model '{}' should be free",
                id
            );
        }
    }

    #[test]
    fn test_native_models_have_context_window() {
        for (id, info) in openai_native_models() {
            assert!(
                info.context_window > 0,
                "Native model '{}' should have context_window > 0",
                id
            );
        }
    }

    #[test]
    fn test_codex_models_have_context_window() {
        for (id, info) in openai_codex_models() {
            assert!(
                info.context_window > 0,
                "Codex model '{}' should have context_window > 0",
                id
            );
        }
    }

    #[test]
    fn test_native_models_count() {
        let models = openai_native_models();
        assert!(
            models.len() >= 36,
            "Should have at least 36 OpenAI Native models, got {}",
            models.len()
        );
    }

    #[test]
    fn test_codex_models_count() {
        let models = openai_codex_models();
        assert!(
            models.len() >= 12,
            "Should have at least 12 OpenAI Codex models, got {}",
            models.len()
        );
    }

    #[test]
    fn test_gpt51_codex_max_has_xhigh_reasoning() {
        let models = openai_native_models();
        let model = models.get("gpt-5.1-codex-max").unwrap();
        assert_eq!(model.reasoning_effort, Some(ReasoningEffortExtended::Xhigh));
    }

    #[test]
    fn test_gpt54_has_priority_tier() {
        let models = openai_native_models();
        let model = models.get("gpt-5.4").unwrap();
        let tiers = model.tiers.as_ref().unwrap();
        assert!(tiers.len() >= 2);
        assert_eq!(tiers[1].name, Some(ServiceTier::Priority));
    }

    #[test]
    fn test_codex_spark_no_images() {
        let models = openai_codex_models();
        let model = models.get("gpt-5.3-codex-spark").unwrap();
        assert_eq!(model.supports_images, Some(false));
    }
}
