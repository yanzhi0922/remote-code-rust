//! VS Code Language Model API provider handler.
//!
//! This provider uses the VS Code Language Model API to interact with
//! language models registered in VS Code (e.g., GitHub Copilot models).
//!
//! **Note**: In a pure Rust standalone context, this handler provides a
//! structural implementation. The actual VS Code LM integration requires
//! the VS Code extension host runtime. When used in a VS Code extension
//! context (via LSP/JSON-RPC bridge), the model calls are forwarded to
//! the VS Code Language Model API.
//!
//! Source: `src/api/providers/vscode-lm.ts`

use async_trait::async_trait;
use roo_provider::{ApiStream, CreateMessageMetadata, Provider};
use roo_types::api::ProviderName;
use roo_types::model::ModelInfo;

use crate::models;
use crate::types::VscodeLmConfig;

/// VS Code Language Model API provider handler.
///
/// Provides access to language models registered in VS Code through
/// the Language Model API. This handler is designed to work with a
/// VS Code extension host bridge for actual model inference.
pub struct VscodeLmHandler {
    /// Configuration for the VS Code LM provider.
    config: VscodeLmConfig,
    /// The resolved model ID.
    model_id: String,
    /// The model info for the resolved model.
    model_info: ModelInfo,
}

impl VscodeLmHandler {
    /// Create a new VS Code LM handler from configuration.
    pub fn new(config: VscodeLmConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config
            .model_id
            .clone()
            .unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(4096),
                context_window: 128000,
                supports_prompt_cache: false,
                input_price: Some(0.0),
                output_price: Some(0.0),
                description: Some("VS Code LM model (discovered at runtime)".to_string()),
                ..Default::default()
            });

        Ok(Self {
            config,
            model_id,
            model_info,
        })
    }

    /// Create a new VS Code LM handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config =
            VscodeLmConfig::from_settings(settings).unwrap_or_else(|| VscodeLmConfig {
                model_selector: None,
                model_id: None,
                temperature: None,
                request_timeout: None,
            });
        Self::new(config)
    }

    /// Get the model selector for VS Code Language Model API.
    pub fn model_selector(&self) -> Option<&serde_json::Value> {
        self.config.model_selector.as_ref()
    }

    /// Get the temperature setting.
    pub fn temperature(&self) -> Option<f64> {
        self.config.temperature
    }
}

#[async_trait]
impl Provider for VscodeLmHandler {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<roo_types::api::ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream, roo_provider::ProviderError> {
        // In a VS Code extension context, this would forward to the VS Code LM API.
        // In standalone mode, we return an error indicating VS Code runtime is needed.
        let _ = (system_prompt, messages, tools, metadata);
        Err(roo_provider::ProviderError::Other(
            "VS Code Language Model API requires VS Code extension host runtime. \
             This handler should be used through the VS Code extension bridge."
                .to_string(),
        ))
    }

    fn get_model(&self) -> (String, ModelInfo) {
        (self.model_id.clone(), self.model_info.clone())
    }

    async fn count_tokens(
        &self,
        content: &[roo_types::api::ContentBlock],
    ) -> Result<u64, roo_provider::ProviderError> {
        // VS Code LM provides its own token counting via
        // `vscode.LanguageModelChat.countTokens()`.
        // Fallback: estimate ~4 chars per token.
        let total_chars: usize = content
            .iter()
            .map(|block| match block {
                roo_types::api::ContentBlock::Text { text } => text.len(),
                _ => 0,
            })
            .sum();
        Ok((total_chars as u64) / 4)
    }

    async fn complete_prompt(
        &self,
        prompt: &str,
    ) -> Result<String, roo_provider::ProviderError> {
        let _ = prompt;
        Err(roo_provider::ProviderError::Other(
            "VS Code Language Model API requires VS Code extension host runtime.".to_string(),
        ))
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::VscodeLm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models;

    #[test]
    fn test_default_model_exists() {
        let all_models = models::models();
        assert!(
            all_models.contains_key(models::DEFAULT_MODEL_ID),
            "Default model '{}' should exist in models map",
            models::DEFAULT_MODEL_ID
        );
    }

    #[test]
    fn test_all_models_have_required_fields() {
        for (id, info) in models::models() {
            assert!(info.max_tokens.is_some(), "Model '{}' missing max_tokens", id);
            assert!(
                info.input_price.is_some(),
                "Model '{}' missing input_price",
                id
            );
            assert!(
                info.output_price.is_some(),
                "Model '{}' missing output_price",
                id
            );
        }
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: Some("copilot-gpt-4o".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "copilot-gpt-4o");
    }

    #[test]
    fn test_handler_from_settings() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let handler = VscodeLmHandler::from_settings(&settings);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::VscodeLm);
    }

    #[test]
    fn test_fallback_model_info() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: Some("unknown-vscode-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        let (_, info) = handler.get_model();
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_temperature_config() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: None,
            temperature: Some(0.5),
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        assert_eq!(handler.temperature(), Some(0.5));
    }

    #[test]
    fn test_models_count() {
        let all_models = models::models();
        assert_eq!(all_models.len(), 4);
    }

    #[test]
    fn test_all_vscode_lm_models_are_free() {
        for (id, info) in models::models() {
            assert_eq!(
                info.input_price,
                Some(0.0),
                "VS Code LM model '{}' should be free (input_price = 0.0)",
                id
            );
            assert_eq!(
                info.output_price,
                Some(0.0),
                "VS Code LM model '{}' should be free (output_price = 0.0)",
                id
            );
        }
    }

    #[test]
    fn test_model_selector() {
        let selector = serde_json::json!({
            "vendor": "copilot",
            "family": "gpt-4"
        });
        let config = VscodeLmConfig {
            model_selector: Some(selector.clone()),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        assert!(handler.model_selector().is_some());
        assert_eq!(handler.model_selector().unwrap(), &selector);
    }

    #[test]
    fn test_create_message_returns_error_without_runtime() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(handler.create_message(
            "test",
            vec![],
            None,
            CreateMessageMetadata::default(),
        ));
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_prompt_returns_error_without_runtime() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(handler.complete_prompt("test prompt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_count_tokens_estimates() {
        let config = VscodeLmConfig {
            model_selector: None,
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = VscodeLmHandler::new(config).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let content = vec![roo_types::api::ContentBlock::Text {
            text: "Hello, world! This is a test.".to_string(),
        }];
        let count = rt.block_on(handler.count_tokens(&content)).unwrap();
        // ~30 chars / 4 = ~7 tokens
        assert!(count > 0);
        assert!(count < 20);
    }
}
