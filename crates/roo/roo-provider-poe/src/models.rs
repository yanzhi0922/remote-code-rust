//! Poe model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Poe model ID.
pub const DEFAULT_MODEL_ID: &str = "gpt-4o";

/// Returns the supported Poe fallback models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "gpt-4o".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 128000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            description: Some("GPT-4o via Poe (subscription)".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "claude-3-5-sonnet-20241022".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            description: Some("Claude 3.5 Sonnet via Poe (subscription)".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
