//! Mistral AI model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Mistral model ID.
pub const DEFAULT_MODEL_ID: &str = "codestral-latest";

/// Returns the supported Mistral models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "magistral-medium-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 128_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(2.0),
            output_price: Some(5.0),
            description: Some("Magistral Medium - Mistral's reasoning model.".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "devstral-medium-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.4),
            output_price: Some(2.0),
            description: Some("Devstral Medium - Mistral's agentic coding model.".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "mistral-medium-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.4),
            output_price: Some(2.0),
            description: Some("Mistral Medium - balanced performance and cost.".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "codestral-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 256_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.3),
            output_price: Some(0.9),
            description: Some("Codestral - Mistral's code generation model.".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "mistral-large-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(2.0),
            output_price: Some(6.0),
            description: Some("Mistral Large - top-tier reasoning model.".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "ministral-8b-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.1),
            output_price: Some(0.1),
            description: Some("MiniStral 8B - lightweight model.".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "ministral-3b-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.04),
            output_price: Some(0.04),
            description: Some("MiniStral 3B - ultra-lightweight model.".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "mistral-small-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 32_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.2),
            output_price: Some(0.6),
            description: Some("Mistral Small - cost-effective model.".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "pixtral-large-latest".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131_000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(2.0),
            output_price: Some(6.0),
            description: Some("Pixtral Large - multimodal vision model.".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
