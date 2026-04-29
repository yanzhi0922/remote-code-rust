//! VS Code Language Model API-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Configuration for the VS Code Language Model API provider.
///
/// VS Code LM provides access to language models registered in VS Code
/// through the Language Model API. This is a VS Code-specific provider
/// that uses `vscode.lm.selectChatModels()` to discover and use models.
/// Source: `src/api/providers/vscode-lm.ts`
#[derive(Debug, Clone)]
pub struct VscodeLmConfig {
    /// Model selector for VS Code Language Model API.
    /// Specifies vendor, family, version, and id constraints.
    pub model_selector: Option<serde_json::Value>,
    /// Model ID override.
    pub model_id: Option<String>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
}

impl VscodeLmConfig {
    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        // VS Code LM is always available if we're running in VS Code
        Some(Self {
            model_selector: settings.vs_code_lm_model_selector.clone(),
            model_id: settings.api_model_id.clone(),
            temperature: settings.model_temperature.flatten(),
            request_timeout: settings.request_timeout,
        })
    }
}

/// VS Code Language Model Chat Message role.
#[derive(Debug, Clone, PartialEq)]
pub enum VscodeLmMessageRole {
    /// User message.
    User,
    /// Assistant message.
    Assistant,
}

/// VS Code Language Model tool call information.
#[derive(Debug, Clone)]
pub struct VscodeLmToolCall {
    /// Tool call ID.
    pub id: String,
    /// Function name.
    pub name: String,
    /// Function arguments as JSON string.
    pub arguments: String,
}

/// VS Code Language Model response part.
#[derive(Debug, Clone)]
pub enum VscodeLmResponsePart {
    /// Text response.
    Text(String),
    /// Tool call response.
    ToolCall(VscodeLmToolCall),
}
