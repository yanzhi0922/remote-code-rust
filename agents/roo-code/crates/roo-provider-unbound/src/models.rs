//! Unbound model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Unbound model ID.
pub const DEFAULT_MODEL_ID: &str = "anthropic/claude-sonnet-4-5";

/// Returns the supported Unbound fallback models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "anthropic/claude-sonnet-4-5".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: true,
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.30),
            description: Some("Claude Sonnet 4.5 via Unbound".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "default".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 128000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            description: Some("Unbound default model".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
