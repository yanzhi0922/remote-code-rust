//! VS Code Language Model API model definitions.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default VS Code LM model ID.
pub const DEFAULT_MODEL_ID: &str = "claude-3.5-sonnet";

/// Returns the supported VS Code LM fallback models.
///
/// VS Code LM provides access to models registered in VS Code.
/// These are placeholder entries; actual models are discovered at runtime
/// via `vscode.lm.selectChatModels()`.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "claude-3.5-sonnet".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200000,
            supports_images: Some(true),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            description: Some("Claude 3.5 Sonnet via VS Code LM".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "vscode-lm-default".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 128000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            description: Some(
                "Default VS Code Language Model (discovered at runtime)".to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "copilot-gpt-4o".to_string(),
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 128000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            description: Some("GitHub Copilot GPT-4o via VS Code LM".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "copilot-claude-3.5-sonnet".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 200000,
            supports_images: Some(false),
            supports_prompt_cache: false,
            input_price: Some(0.0),
            output_price: Some(0.0),
            description: Some("GitHub Copilot Claude 3.5 Sonnet via VS Code LM".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
