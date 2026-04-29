//! ZAI (Zhipu/GLM) model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default ZAI model ID.
pub const DEFAULT_MODEL_ID: &str = "glm-4.6";

/// Returns the supported ZAI models (international).
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "glm-4.5".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.2),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.11),
            description: Some(
                "GLM-4.5 is Zhipu's latest featured model. Its comprehensive capabilities in \
                 reasoning, coding, and agent reach the state-of-the-art (SOTA) level among \
                 open-source models, with a context length of up to 128k."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.5-air".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.2),
            output_price: Some(1.1),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.03),
            description: Some(
                "GLM-4.5-Air is the lightweight version of GLM-4.5. It balances performance and \
                 cost-effectiveness, and can flexibly switch to hybrid thinking models."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.5-x".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(2.2),
            output_price: Some(8.9),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.45),
            description: Some(
                "GLM-4.5-X is a high-performance variant optimized for strong reasoning with \
                 ultra-fast responses."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.5-airx".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(1.1),
            output_price: Some(4.5),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.22),
            description: Some(
                "GLM-4.5-AirX is a lightweight, ultra-fast variant delivering strong performance \
                 with lower cost."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.5-flash".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.0),
            output_price: Some(0.0),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "GLM-4.5-Flash is a free, high-speed model excellent for reasoning, coding, \
                 and agentic tasks."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.5v".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(1.8),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.11),
            description: Some(
                "GLM-4.5V is Z.AI's multimodal visual reasoning model (image/video/text/file \
                 input), optimized for GUI tasks, grounding, and document/video understanding."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.6v".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(0.9),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.05),
            description: Some(
                "GLM-4.6V is an advanced multimodal vision model with improved performance and \
                 cost-efficiency for visual understanding tasks."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.6".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.2),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.11),
            description: Some(
                "GLM-4.6 is Zhipu's newest model with an extended context window of up to 200k \
                 tokens, providing enhanced capabilities for processing longer documents and conversations."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.7".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            supports_reasoning_effort: Some(serde_json::json!(["disable", "medium"])),
            reasoning_effort: None, // "medium" maps to ReasoningEffortExtended but "disable" is not in that enum
            preserve_reasoning: Some(true),
            input_price: Some(0.6),
            output_price: Some(2.2),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.11),
            description: Some(
                "GLM-4.7 is Zhipu's latest model with built-in thinking capabilities enabled by \
                 default. It provides enhanced reasoning for complex tasks while maintaining fast \
                 response times."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-5".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 202_752,
            supports_images: Some(false),
            supports_prompt_cache: true,
            supports_reasoning_budget: Some(true),
            supports_reasoning_effort: Some(serde_json::json!(["disable", "medium"])),
            reasoning_effort: None,
            preserve_reasoning: Some(true),
            input_price: Some(0.6),
            output_price: Some(2.2),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.11),
            description: Some(
                "GLM-5 is Zhipu's next-generation model with a 202k context window and built-in \
                 thinking capabilities. It delivers state-of-the-art reasoning, coding, and agentic \
                 performance."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.7-flash".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.0),
            output_price: Some(0.0),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "GLM-4.7-Flash is a free, high-speed variant of GLM-4.7 offering fast responses \
                 for reasoning and coding tasks."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.7-flashx".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 200_000,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.07),
            output_price: Some(0.4),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.01),
            description: Some(
                "GLM-4.7-FlashX is an ultra-fast variant of GLM-4.7 with exceptional speed and \
                 cost-effectiveness for high-throughput applications."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.6v-flash".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.0),
            output_price: Some(0.0),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "GLM-4.6V-Flash is a free, high-speed multimodal vision model for rapid image \
                 understanding and visual reasoning tasks."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4.6v-flashx".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(0.04),
            output_price: Some(0.4),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.004),
            description: Some(
                "GLM-4.6V-FlashX is an ultra-fast multimodal vision model optimized for \
                 high-speed visual processing at low cost."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "glm-4-32b-0414-128k".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 131_072,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.1),
            output_price: Some(0.1),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "GLM-4-32B is a 32 billion parameter model with 128k context length, optimized \
                 for efficiency."
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
