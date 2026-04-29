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

    /// Count tokens for content blocks.
    ///
    /// Default implementation uses a simple heuristic of ~4 characters per token.
    /// Individual providers should override this with accurate counting when available.
    async fn count_tokens(&self, content: &[roo_types::api::ContentBlock]) -> Result<u64> {
        let total_chars: usize = content
            .iter()
            .map(|block| match block {
                roo_types::api::ContentBlock::Text { text } => text.len(),
                roo_types::api::ContentBlock::ToolUse { input, .. } => {
                    // Estimate JSON input size
                    serde_json::to_string(input).map(|s| s.len()).unwrap_or(0)
                }
                roo_types::api::ContentBlock::ToolResult { content, .. } => content
                    .iter()
                    .map(|c| match c {
                        roo_types::api::ToolResultContent::Text { text } => text.len(),
                        roo_types::api::ToolResultContent::Image { .. } => 256, // rough estimate for image tokens
                    })
                    .sum(),
                roo_types::api::ContentBlock::Image { source } => {
                    // Rough estimate: images typically use 85-170 tokens depending on detail
                    match source {
                        roo_types::api::ImageSource::Base64 { data, .. } => {
                            // Estimate based on base64 data length
                            (data.len() / 100).max(85).min(1000)
                        }
                        roo_types::api::ImageSource::Url { .. } => 256,
                    }
                }
                roo_types::api::ContentBlock::Thinking { thinking, .. } => thinking.len(),
                roo_types::api::ContentBlock::RedactedThinking { data } => data.len() / 4,
            })
            .sum();
        // ~4 characters per token is a reasonable default for most tokenizers
        Ok((total_chars as u64).div_ceil(4))
    }

    /// Complete a simple prompt (non-streaming).
    async fn complete_prompt(&self, prompt: &str) -> Result<String>;

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
    let mut registry = PROVIDER_REGISTRY.write().unwrap();
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

    let registry = PROVIDER_REGISTRY.read().unwrap();
    let map = registry
        .as_ref()
        .ok_or_else(|| ProviderError::Other("No providers registered — call register_provider() first".to_string()))?;

    let factory = map.get(&provider_name).ok_or_else(|| {
        ProviderError::Other(format!(
            "Provider '{}' is not registered — add the provider crate dependency and call register_provider() during startup",
            provider_name.as_str()
        ))
    })?;

    factory(configuration)
}
