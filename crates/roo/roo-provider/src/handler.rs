//! Provider trait and factory function.
//!
//! Derived from `src/api/index.ts`.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::RwLock;

use async_trait::async_trait;
use futures::Stream;

use roo_types::api::{ApiMessage, ApiStreamChunk, ProviderName};
use roo_types::model::ModelInfo;
use roo_types::provider_settings::ProviderSettings;

use crate::error::{ProviderError, Result};

/// A stream of API response chunks.
pub type ApiStream = Pin<Box<dyn Stream<Item = Result<ApiStreamChunk>> + Send>>;

/// Metadata passed to `create_message`.
///
/// Source: `src/api/index.ts` — `ApiHandlerCreateMessageMetadata`
#[derive(Debug, Clone, Default)]
pub struct CreateMessageMetadata {
    pub task_id: Option<String>,
    pub mode: Option<String>,
    pub suppress_previous_response_id: Option<bool>,
    pub store: Option<bool>,
    pub tools: Option<Vec<serde_json::Value>>,
    pub tool_choice: Option<serde_json::Value>,
    pub parallel_tool_calls: Option<bool>,
    pub allowed_function_names: Option<Vec<String>>,
}

/// Core trait for API providers.
///
/// Source: `src/api/index.ts` — `ApiHandler` + `SingleCompletionHandler`
#[async_trait]
pub trait Provider: Send + Sync {
    /// Create a streaming message response.
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream>;

    /// Get the model ID and info.
    fn get_model(&self) -> (String, ModelInfo);

    /// Count tokens for content blocks using real BPE tokenization.
    ///
    /// Uses tiktoken o200k_base encoding with 1.5x fudge factor, matching the
    /// TypeScript reference's `tiktoken` utility exactly.
    ///
    /// Source: `src/utils/tiktoken.ts` — `tiktoken`
    async fn count_tokens(&self, content: &[roo_types::api::ContentBlock]) -> Result<u64> {
        use std::sync::LazyLock;
        use roo_types::api::{ContentBlock, ImageSource, ToolResultContent};

        const TOKEN_FUDGE_FACTOR: f64 = 1.5;
        const DEFAULT_IMAGE_TOKENS: u64 = 300;

        static BPE: LazyLock<Option<tiktoken_rs::CoreBPE>> = LazyLock::new(|| {
            tiktoken_rs::o200k_base()
                .ok()
                .or_else(|| tiktoken_rs::cl100k_base().ok())
        });

        fn count_text(text: &str) -> u64 {
            if let Some(bpe) = BPE.as_ref() {
                bpe.encode_with_special_tokens(text).len() as u64
            } else {
                (text.len() as u64).div_ceil(4)
            }
        }

        fn serialize_tool_use(name: &str, input: &serde_json::Value) -> String {
            let mut parts = vec![format!("Tool: {name}")];
            if !input.is_null() {
                if let Ok(s) = serde_json::to_string(input) {
                    parts.push(format!("Arguments: {s}"));
                }
            }
            parts.join("\n")
        }

        fn serialize_tool_result(
            tool_use_id: &str,
            content: &[ToolResultContent],
            is_error: Option<bool>,
        ) -> String {
            let mut parts = vec![format!("Tool Result ({tool_use_id})")];
            if is_error.unwrap_or(false) {
                parts.push("[Error]".to_string());
            }
            for item in content {
                match item {
                    ToolResultContent::Text { text } => parts.push(text.clone()),
                    ToolResultContent::Image { .. } => parts.push("[Image content]".to_string()),
                }
            }
            parts.join("\n")
        }

        if content.is_empty() {
            return Ok(0);
        }

        let mut total_tokens: u64 = 0;
        for block in content {
            let block_tokens = match block {
                ContentBlock::Text { text } if text.is_empty() => 0,
                ContentBlock::Text { text } => count_text(text),
                ContentBlock::ToolUse { name, input, .. } => {
                    count_text(&serialize_tool_use(name, input))
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => count_text(&serialize_tool_result(tool_use_id, content, *is_error)),
                ContentBlock::Image { source } => match source {
                    ImageSource::Base64 { data, .. } => {
                        (data.len() as f64).sqrt().ceil() as u64
                    }
                    ImageSource::Url { .. } => DEFAULT_IMAGE_TOKENS,
                },
                ContentBlock::Thinking { thinking, .. } if thinking.is_empty() => 0,
                ContentBlock::Thinking { thinking, .. } => count_text(thinking),
                ContentBlock::RedactedThinking { data } => {
                    (data.len() as f64 / 4.0).ceil() as u64
                }
            };
            total_tokens += block_tokens;
        }

        Ok((total_tokens as f64 * TOKEN_FUDGE_FACTOR).ceil() as u64)
    }

    /// Complete a simple prompt (non-streaming).
    async fn complete_prompt(&self, prompt: &str) -> Result<String>;

    /// Generate an image using this provider.
    ///
    /// Default implementation returns an error. Providers that support image
    /// generation (OpenRouter, Roo) should override this.
    ///
    /// Source: `src/api/providers/openrouter.ts` — `generateImage()`
    async fn generate_image(
        &self,
        _prompt: &str,
        _model: &str,
        _input_image: Option<&str>,
    ) -> Result<crate::image_generation::ImageGenerationResult> {
        Err(ProviderError::Other(
            "Image generation is not supported by this provider".to_string(),
        ))
    }

    /// Get the provider name.
    fn provider_name(&self) -> ProviderName;
}

/// Type alias for a provider factory function.
///
/// Each provider crate registers one of these via [`register_provider`].
pub type ProviderFactoryFn =
    fn(&ProviderSettings) -> std::result::Result<Box<dyn Provider>, ProviderError>;

/// Global provider registry (lazy-initialized).
static PROVIDER_REGISTRY: RwLock<Option<HashMap<ProviderName, ProviderFactoryFn>>> =
    RwLock::new(None);

/// Register a provider factory function.
///
/// Call this during application startup (before any [`build_api_handler`] call)
/// to make a provider available through the factory.
///
/// # Example
///
/// ```rust,ignore
/// use roo_provider::{register_provider, Provider, ProviderError};
/// use roo_types::api::ProviderName;
/// use roo_types::provider_settings::ProviderSettings;
///
/// fn my_factory(settings: &ProviderSettings) -> Result<Box<dyn Provider>, ProviderError> {
///     // ... construct provider ...
/// }
///
/// roo_provider::register_provider(ProviderName::Anthropic, my_factory);
/// ```
pub fn register_provider(name: ProviderName, factory: ProviderFactoryFn) {
    let mut registry = PROVIDER_REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    let map = registry.get_or_insert_with(HashMap::new);
    map.insert(name, factory);
}

/// Register all built-in providers.
///
/// This is a convenience function that calls [`register_provider`] for each
/// known provider crate. Individual provider crates should each expose an
/// `init()` function that registers themselves.
///
/// # Note
///
/// This function is intentionally a no-op in the `roo-provider` crate itself,
/// because `roo-provider` cannot depend on individual provider crates (that
/// would create circular dependencies). The actual registration happens in
/// the application crate (e.g. `roo-server`) which depends on all providers.
pub fn register_default_providers() {
    // No-op here — registration is done by the application crate.
    // See `roo-server::register_providers()`.
}

/// Builds an API handler for the given configuration.
///
/// Source: `src/api/index.ts` — `buildApiHandler`
///
/// Looks up the provider in the global registry and delegates construction
/// to the registered factory function.
///
/// # Errors
///
/// Returns [`ProviderError::Other`] if:
/// - No `api_provider` is specified in the configuration
/// - No factory has been registered for the requested provider
pub fn build_api_handler(
    configuration: &ProviderSettings,
) -> std::result::Result<Box<dyn Provider>, ProviderError> {
    let provider_name = configuration
        .api_provider
        .ok_or_else(|| ProviderError::Other("No API provider specified".to_string()))?;

    let registry = PROVIDER_REGISTRY.read().unwrap_or_else(|e| e.into_inner());
    let map = registry.as_ref().ok_or_else(|| {
        ProviderError::Other("No providers registered — call register_provider() first".to_string())
    })?;

    let factory = map.get(&provider_name).ok_or_else(|| {
        ProviderError::Other(format!(
            "Provider '{}' is not registered — add the provider crate dependency and call register_provider() during startup",
            provider_name.as_str()
        ))
    })?;

    factory(configuration)
}
