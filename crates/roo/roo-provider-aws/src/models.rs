//! AWS Bedrock model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Bedrock model ID.
pub const DEFAULT_MODEL_ID: &str = "anthropic.claude-sonnet-4-5-20250929-v1:0";

/// Returns the supported AWS Bedrock models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    // --- Anthropic Claude on Bedrock ---
    m.insert(
        "anthropic.claude-sonnet-4-5-20250929-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude Sonnet 4.5 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-sonnet-4-6".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude Sonnet 4.6 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude Sonnet 4 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-opus-4-1-20250805-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            description: Some("Anthropic Claude Opus 4.1 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-opus-4-6-v1".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(5.0),
            output_price: Some(25.0),
            cache_writes_price: Some(6.25),
            cache_reads_price: Some(0.5),
            description: Some("Anthropic Claude Opus 4.6 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-opus-4-5-20251101-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(5.0),
            output_price: Some(25.0),
            cache_writes_price: Some(6.25),
            cache_reads_price: Some(0.5),
            description: Some("Anthropic Claude Opus 4.5 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-opus-4-20250514-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(15.0),
            output_price: Some(75.0),
            cache_writes_price: Some(18.75),
            cache_reads_price: Some(1.5),
            description: Some("Anthropic Claude Opus 4 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-3-7-sonnet-20250219-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude 3.7 Sonnet on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            description: Some("Anthropic Claude 3.5 Sonnet v2 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.8),
            output_price: Some(4.0),
            cache_writes_price: Some(1.0),
            cache_reads_price: Some(0.08),
            description: Some("Anthropic Claude 3.5 Haiku on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-haiku-4-5-20251001-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            input_price: Some(1.0),
            output_price: Some(5.0),
            cache_writes_price: Some(1.25),
            cache_reads_price: Some(0.1),
            description: Some("Anthropic Claude Haiku 4.5 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-3-5-sonnet-20240620-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(3.0),
            output_price: Some(15.0),
            description: Some("Anthropic Claude 3.5 Sonnet (v1) on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-3-opus-20240229-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(15.0),
            output_price: Some(75.0),
            description: Some("Anthropic Claude 3 Opus on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(3.0),
            output_price: Some(15.0),
            description: Some("Anthropic Claude 3 Sonnet on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 200_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.25),
            output_price: Some(1.25),
            description: Some("Anthropic Claude 3 Haiku on Bedrock".to_string()),
            ..Default::default()
        },
    );

    // --- Amazon Nova ---
    m.insert(
        "amazon.nova-pro-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(5000),
            context_window: 300_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.8),
            output_price: Some(3.2),
            cache_writes_price: Some(0.8),
            cache_reads_price: Some(0.2),
            description: Some("Amazon Nova Pro on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "amazon.nova-pro-latency-optimized-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(5000),
            context_window: 300_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(1.0),
            output_price: Some(4.0),
            cache_writes_price: Some(1.0),
            cache_reads_price: Some(0.25),
            description: Some(
                "Amazon Nova Pro with latency optimized inference".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "amazon.nova-lite-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(5000),
            context_window: 300_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.06),
            output_price: Some(0.24),
            cache_writes_price: Some(0.06),
            cache_reads_price: Some(0.015),
            description: Some("Amazon Nova Lite on Bedrock".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "amazon.nova-2-lite-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(65_535),
            context_window: 1_000_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.33),
            output_price: Some(2.75),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0825),
            description: Some(
                "Amazon Nova 2 Lite - Comparable to Claude Haiku 4.5".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "amazon.nova-micro-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(5000),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.035),
            output_price: Some(0.14),
            cache_writes_price: Some(0.035),
            cache_reads_price: Some(0.00875),
            description: Some("Amazon Nova Micro on Bedrock".to_string()),
            ..Default::default()
        },
    );

    // --- Amazon Nova (Cross-Region Inference) ---
    m.insert(
        "us.amazon.nova-pro-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(5000),
            context_window: 300_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.8),
            output_price: Some(3.2),
            cache_writes_price: Some(0.8),
            cache_reads_price: Some(0.2),
            description: Some(
                "Amazon Nova Pro (US Cross-Region Inference)".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "us.amazon.nova-lite-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(5000),
            context_window: 300_000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.06),
            output_price: Some(0.24),
            cache_writes_price: Some(0.06),
            cache_reads_price: Some(0.015),
            description: Some(
                "Amazon Nova Lite (US Cross-Region Inference)".to_string(),
            ),
            ..Default::default()
        },
    );

    // --- DeepSeek ---
    m.insert(
        "deepseek.r1-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(1.35),
            output_price: Some(5.4),
            description: Some("DeepSeek R1 on Bedrock".to_string()),
            ..Default::default()
        },
    );

    // --- OpenAI GPT-OSS ---
    m.insert(
        "openai.gpt-oss-20b-1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.5),
            output_price: Some(1.5),
            description: Some(
                "GPT-OSS 20B - Optimized for low latency and local/specialized use cases"
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "openai.gpt-oss-120b-1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(2.0),
            output_price: Some(6.0),
            description: Some(
                "GPT-OSS 120B - Production-ready, general-purpose, high-reasoning model"
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    // --- Meta Llama ---
    m.insert(
        "meta.llama3-3-70b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.72),
            output_price: Some(0.72),
            description: Some("Llama 3.3 Instruct (70B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-2-90b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.72),
            output_price: Some(0.72),
            description: Some("Llama 3.2 Instruct (90B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-2-11b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.16),
            output_price: Some(0.16),
            description: Some("Llama 3.2 Instruct (11B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-2-3b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.15),
            output_price: Some(0.15),
            description: Some("Llama 3.2 Instruct (3B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-2-1b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.1),
            output_price: Some(0.1),
            description: Some("Llama 3.2 Instruct (1B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-1-405b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(2.4),
            output_price: Some(2.4),
            description: Some("Llama 3.1 Instruct (405B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-1-70b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.72),
            output_price: Some(0.72),
            description: Some("Llama 3.1 Instruct (70B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-1-70b-instruct-latency-optimized-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.9),
            output_price: Some(0.9),
            description: Some(
                "Llama 3.1 Instruct (70B) (w/ latency optimized inference)".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-1-8b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 8_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.22),
            output_price: Some(0.22),
            description: Some("Llama 3.1 Instruct (8B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-70b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(2048),
            context_window: 8_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(2.65),
            output_price: Some(3.5),
            description: Some("Llama 3 Instruct (70B)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "meta.llama3-8b-instruct-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(2048),
            context_window: 4_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.3),
            output_price: Some(0.6),
            description: Some("Llama 3 Instruct (8B)".to_string()),
            ..Default::default()
        },
    );

    // --- Amazon Titan ---
    m.insert(
        "amazon.titan-text-lite-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 8_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.15),
            output_price: Some(0.2),
            description: Some("Amazon Titan Text Lite".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "amazon.titan-text-express-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 8_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.2),
            output_price: Some(0.6),
            description: Some("Amazon Titan Text Express".to_string()),
            ..Default::default()
        },
    );

    // --- Moonshot ---
    m.insert(
        "moonshot.kimi-k2-thinking".to_string(),
        ModelInfo {
            max_tokens: Some(32_000),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: false,
            preserve_reasoning: Some(true),
            input_price: Some(0.6),
            output_price: Some(2.5),
            description: Some(
                "Kimi K2 Thinking (1T parameter MoE model with 32B active parameters)".to_string(),
            ),
            ..Default::default()
        },
    );

    // --- MiniMax ---
    m.insert(
        "minimax.minimax-m2".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 196_608,
            supports_images: Some(false),
            supports_prompt_cache: false,
            preserve_reasoning: Some(true),
            input_price: Some(0.3),
            output_price: Some(1.2),
            description: Some(
                "MiniMax M2 (230B parameter MoE model with 10B active parameters)".to_string(),
            ),
            ..Default::default()
        },
    );

    // --- Qwen ---
    m.insert(
        "qwen.qwen3-next-80b-a3b".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.15),
            output_price: Some(1.2),
            description: Some(
                "Qwen3 Next 80B (MoE model with 3B active parameters)".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "qwen.qwen3-coder-480b-a35b-v1:0".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 262_144,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.45),
            output_price: Some(1.8),
            description: Some(
                "Qwen3 Coder 480B (MoE model with 35B active parameters)".to_string(),
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
