//! Ollama model definitions.
//!
//! Ollama supports many local models. We define popular defaults.

use std::collections::HashMap;
use roo_types::model::ModelInfo;

/// Default Ollama model ID.
pub const DEFAULT_MODEL_ID: &str = "devstral:24b";

/// Returns commonly used Ollama models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "llama3.2".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131072,
            supports_images: Some(true),
            description: Some("Meta Llama 3.2 via Ollama".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "llama3.1".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131072,
            description: Some("Meta Llama 3.1 via Ollama".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "codellama".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 16384,
            description: Some("Code Llama via Ollama".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "mistral".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 32768,
            description: Some("Mistral via Ollama".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "deepseek-coder-v2".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131072,
            description: Some("DeepSeek Coder V2 via Ollama".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "devstral:24b".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131072,
            supports_images: Some(false),
            description: Some("Devstral 24B via Ollama".to_string()),
            ..Default::default()
        },
    );

    m.insert(
        "qwen2.5-coder".to_string(),
        ModelInfo {
            max_tokens: Some(8192),
            context_window: 131072,
            supports_images: Some(false),
            description: Some("Qwen 2.5 Coder via Ollama".to_string()),
            ..Default::default()
        },
    );

    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}
