//! Qwen / 通义千问 model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Qwen model ID.
pub const DEFAULT_MODEL_ID: &str = "qwen3-coder-plus";

/// Returns the supported Qwen models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "qwen3-coder-plus".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_000_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Qwen3 Coder Plus - High-performance coding model with 1M context window for \
                 large codebases."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "qwen3-coder-flash".to_string(),
        ModelInfo {
            max_tokens: Some(65_536),
            context_window: 1_000_000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            cache_writes_price: Some(0.0),
            cache_reads_price: Some(0.0),
            description: Some(
                "Qwen3 Coder Flash - Fast coding model with 1M context window optimized for speed."
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
