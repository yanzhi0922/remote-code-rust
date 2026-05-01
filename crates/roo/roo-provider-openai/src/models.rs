//! OpenAI model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default OpenAI model ID.
pub const DEFAULT_MODEL_ID: &str = "gpt-4o";

/// Returns the supported OpenAI models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "gpt-4o".to_string(),
        ModelInfo {
            max_tokens: Some(16384),
            context_window: 128000,
            supports_images: Some(true),
            input_price: Some(2.5),
            output_price: Some(10.0),
            description: Some("OpenAI GPT-4o".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-4o-mini".to_string(),
        ModelInfo {
            max_tokens: Some(16384),
            context_window: 128000,
            supports_images: Some(true),
            input_price: Some(0.15),
            output_price: Some(0.60),
            description: Some("OpenAI GPT-4o Mini".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-4.1".to_string(),
        ModelInfo {
            max_tokens: Some(32768),
            context_window: 1047576,
            supports_images: Some(true),
            input_price: Some(2.0),
            output_price: Some(8.0),
            description: Some("OpenAI GPT-4.1".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-4.1-mini".to_string(),
        ModelInfo {
            max_tokens: Some(32768),
            context_window: 1047576,
            supports_images: Some(true),
            input_price: Some(0.40),
            output_price: Some(1.60),
            description: Some("OpenAI GPT-4.1 Mini".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "gpt-4.1-nano".to_string(),
        ModelInfo {
            max_tokens: Some(32768),
            context_window: 1047576,
            supports_images: Some(true),
            input_price: Some(0.10),
            output_price: Some(0.40),
            description: Some("OpenAI GPT-4.1 Nano".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "o3".to_string(),
        ModelInfo {
            max_tokens: Some(100000),
            context_window: 200000,
            supports_images: Some(true),
            input_price: Some(10.0),
            output_price: Some(40.0),
            description: Some("OpenAI o3 reasoning model".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "o4-mini".to_string(),
        ModelInfo {
            max_tokens: Some(100000),
            context_window: 200000,
            supports_images: Some(true),
            input_price: Some(1.10),
            output_price: Some(4.40),
            description: Some("OpenAI o4-mini reasoning model".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "o3-mini".to_string(),
        ModelInfo {
            max_tokens: Some(65536),
            context_window: 200000,
            input_price: Some(1.10),
            output_price: Some(4.40),
            description: Some("OpenAI o3-mini reasoning model".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
