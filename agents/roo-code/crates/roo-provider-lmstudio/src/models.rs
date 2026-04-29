//! LM Studio model definitions.
//!
//! LM Studio uses dynamic model lists fetched from `/v1/models`.
//! This module provides default model info for when the model list
//! is not available.

use roo_types::model::ModelInfo;

/// Default LM Studio model ID.
pub const DEFAULT_MODEL_ID: &str = "mistralai/devstral-small-2505";

/// Default temperature for LM Studio models.
pub const DEFAULT_TEMPERATURE: f64 = 0.0;

/// Returns the default model info for LM Studio.
///
/// Since LM Studio models are loaded dynamically, this provides
/// sane defaults when the model list is not available.
pub fn default_model_info() -> ModelInfo {
    ModelInfo {
        max_tokens: Some(8192),
        context_window: 200_000,
        supports_images: Some(true),
        supports_prompt_cache: true,
        input_price: Some(0.0),
        output_price: Some(0.0),
        cache_writes_price: Some(0.0),
        cache_reads_price: Some(0.0),
        description: Some("LM Studio hosted models".to_string()),
        ..Default::default()
    }
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
