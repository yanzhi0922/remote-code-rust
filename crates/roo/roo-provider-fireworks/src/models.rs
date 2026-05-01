//! Fireworks AI model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Fireworks model ID.
pub const DEFAULT_MODEL_ID: &str = "accounts/fireworks/models/kimi-k2-instruct-0905";

/// Returns the supported Fireworks models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "accounts/fireworks/models/kimi-k2-instruct-0905".to_string(),
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
                 and functionalities on web, 3d, and other tasks. Context length: extended from \
                 128k to 256k, providing better long-horizon support."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/kimi-k2-instruct".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.6),
            output_price: Some(2.5),
            description: Some(
                "Kimi K2 is a state-of-the-art mixture-of-experts (MoE) language model \
                 with 32 billion activated parameters and 1 trillion total parameters. Trained \
                 with the Muon optimizer, Kimi K2 achieves exceptional performance across frontier \
                 knowledge, reasoning, and coding tasks while being meticulously optimized for \
                 agentic capabilities."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/kimi-k2-thinking".to_string(),
        ModelInfo {
            max_tokens: Some(16_000),
            context_window: 256_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            supports_temperature: Some(true),
            default_temperature: Some(1.0),
            supports_reasoning_budget: Some(true),
            preserve_reasoning: Some(true),
            input_price: Some(0.6),
            output_price: Some(2.5),
            cache_reads_price: Some(0.15),
            description: Some(
                "The kimi-k2-thinking model is a general-purpose agentic reasoning model \
                 developed by Moonshot AI. Thanks to its strength in deep reasoning and multi-turn \
                 tool use, it can solve even the hardest problems."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/kimi-k2p5".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 262_144,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(3.0),
            cache_reads_price: Some(0.1),
            description: Some(
                "Kimi K2.5 is Moonshot AI's flagship agentic model and a new SOTA open model. \
                 It unifies vision and text, thinking and non-thinking modes, and single-agent and \
                 multi-agent execution into one model."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/minimax-m2".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.3),
            output_price: Some(1.2),
            description: Some(
                "MiniMax M2 is a high-performance language model with 204.8K context window, \
                 optimized for long-context understanding and generation tasks."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/minimax-m2p1".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.3),
            output_price: Some(1.2),
            description: Some(
                "MiniMax M2.1 is an upgraded version of M2 with improved performance on complex \
                 reasoning, coding, and long-context understanding tasks."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/qwen3-235b-a22b-instruct-2507".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 256_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.22),
            output_price: Some(0.88),
            description: Some(
                "Latest Qwen3 thinking model, competitive against the best closed source models \
                 in Jul 2025."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/qwen3-coder-480b-a35b-instruct".to_string(),
        ModelInfo {
            max_tokens: Some(32_768),
            context_window: 256_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.45),
            output_price: Some(1.8),
            description: Some(
                "Qwen3's most agentic code model to date.".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/deepseek-r1-0528".to_string(),
        ModelInfo {
            max_tokens: Some(20_480),
            context_window: 160_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(3.0),
            output_price: Some(8.0),
            description: Some(
                "05/28 updated checkpoint of Deepseek R1. Its overall performance is now \
                 approaching that of leading models, such as O3 and Gemini 2.5 Pro. Compared to \
                 the previous version, the upgraded model shows significant improvements in \
                 handling complex reasoning tasks, and this version also offers a reduced \
                 hallucination rate, enhanced support for function calling, and better experience \
                 for vibe coding."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/deepseek-v3".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.9),
            output_price: Some(0.9),
            description: Some(
                "A strong Mixture-of-Experts (MoE) language model with 671B total parameters \
                 with 37B activated for each token from Deepseek."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/deepseek-v3p1".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 163_840,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.56),
            output_price: Some(1.68),
            description: Some(
                "DeepSeek v3.1 is an improved version of the v3 model with enhanced performance, \
                 better reasoning capabilities, and improved code generation. This \
                 Mixture-of-Experts (MoE) model maintains the same 671B total parameters with 37B \
                 activated per token."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/deepseek-v3p2".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 163_840,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.56),
            output_price: Some(1.68),
            description: Some(
                "DeepSeek V3.2 is the latest iteration of the V3 model family with enhanced \
                 reasoning capabilities, improved code generation, and better instruction following."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/glm-4p5".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.55),
            output_price: Some(2.19),
            description: Some(
                "Z.ai GLM-4.5 with 355B total parameters and 32B active parameters. Features \
                 unified reasoning, coding, and intelligent agent capabilities."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/glm-4p5-air".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.55),
            output_price: Some(2.19),
            description: Some(
                "Z.ai GLM-4.5-Air with 106B total parameters and 12B active parameters. Features \
                 unified reasoning, coding, and intelligent agent capabilities."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/glm-4p6".to_string(),
        ModelInfo {
            max_tokens: Some(25_344),
            context_window: 198_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.55),
            output_price: Some(2.19),
            description: Some(
                "Z.ai GLM-4.6 is an advanced coding model with exceptional performance on complex \
                 programming tasks. Features improved reasoning capabilities and enhanced code \
                 generation quality, making it ideal for software development workflows."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/glm-4p7".to_string(),
        ModelInfo {
            max_tokens: Some(25_344),
            context_window: 198_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.55),
            output_price: Some(2.19),
            description: Some(
                "Z.ai GLM-4.7 is the latest coding model with exceptional performance on complex \
                 programming tasks. Features improved reasoning capabilities and enhanced code \
                 generation quality."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/gpt-oss-20b".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.07),
            output_price: Some(0.3),
            description: Some(
                "OpenAI gpt-oss-20b: Compact model for local/edge deployments. Optimized for \
                 low-latency and resource-constrained environments with chain-of-thought output, \
                 adjustable reasoning, and agentic workflows."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/gpt-oss-120b".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 128_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.15),
            output_price: Some(0.6),
            description: Some(
                "OpenAI gpt-oss-120b: Production-grade, general-purpose model that fits on a \
                 single H100 GPU. Features complex reasoning, configurable effort, full \
                 chain-of-thought transparency, and supports function calling, tool use, and \
                 structured outputs."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/llama-v3p3-70b-instruct".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.9),
            output_price: Some(0.9),
            description: Some(
                "Meta Llama 3.3 70B Instruct is a highly capable instruction-tuned model with \
                 strong reasoning, coding, and general task performance."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/llama4-maverick-instruct-basic".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.22),
            output_price: Some(0.88),
            description: Some(
                "Llama 4 Maverick is Meta's latest multimodal model with vision capabilities, \
                 optimized for instruction following and coding tasks."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "accounts/fireworks/models/llama4-scout-instruct-basic".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.15),
            output_price: Some(0.6),
            description: Some(
                "Llama 4 Scout is a smaller, faster variant of Llama 4 with multimodal \
                 capabilities, ideal for quick iterations and cost-effective deployments."
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
