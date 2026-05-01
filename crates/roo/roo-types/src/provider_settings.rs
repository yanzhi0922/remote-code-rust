//! Provider settings type definitions.
//!
//! Derived from `packages/types/src/provider-settings.ts` (662 lines).
//! Defines configuration for all AI providers.

use serde::{Deserialize, Serialize};

use crate::api::ProviderName;
use crate::model::{ModelInfo, ReasoningEffortSetting, ServiceTier, VerbosityLevel};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default consecutive mistake limit before the task is paused.
pub const DEFAULT_CONSECUTIVE_MISTAKE_LIMIT: u32 = 3;

// ---------------------------------------------------------------------------
// DynamicProvider / LocalProvider / etc.
// ---------------------------------------------------------------------------

/// Providers that require external API calls to get the model list.
pub const DYNAMIC_PROVIDERS: &[ProviderName] = &[
    ProviderName::OpenRouter,
    ProviderName::VercelAiGateway,
    ProviderName::LiteLlm,
    ProviderName::Requesty,
    ProviderName::Roo,
    ProviderName::Unbound,
    ProviderName::Poe,
];

/// Providers that require localhost API calls to get the model list.
pub const LOCAL_PROVIDERS: &[ProviderName] = &[ProviderName::Ollama, ProviderName::LmStudio];

/// Providers that require internal VSCode API calls.
pub const INTERNAL_PROVIDERS: &[ProviderName] = &[ProviderName::VscodeLm];

/// Providers that are completely configurable within settings.
pub const CUSTOM_PROVIDERS: &[ProviderName] = &[ProviderName::Openai];

/// Providers that do not make external inference calls.
pub const FAUX_PROVIDERS: &[ProviderName] = &[ProviderName::FakeAi];

// ---------------------------------------------------------------------------
// ZaiApiLine
// ---------------------------------------------------------------------------

/// ZAI API line selection.
///
/// Source: `packages/types/src/provider-settings.ts` — `zaiApiLineSchema`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZaiApiLine {
    InternationalCoding,
    ChinaCoding,
    InternationalApi,
    ChinaApi,
}

// ---------------------------------------------------------------------------
// ProviderSettingsEntry
// ---------------------------------------------------------------------------

/// A summary entry for a provider configuration profile.
///
/// Source: `packages/types/src/provider-settings.ts` — `providerSettingsEntrySchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsEntry {
    pub id: String,
    pub name: String,
    pub api_provider: Option<ProviderName>,
    pub model_id: Option<String>,
}

// ---------------------------------------------------------------------------
// AwsBedrockServiceTier
// ---------------------------------------------------------------------------

/// AWS Bedrock service tier selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AwsBedrockServiceTier {
    #[serde(rename = "STANDARD")]
    Standard,
    #[serde(rename = "FLEX")]
    Flex,
    #[serde(rename = "PRIORITY")]
    Priority,
}

// ---------------------------------------------------------------------------
// ProviderSettings
// ---------------------------------------------------------------------------

/// Configuration for an AI provider.
///
/// Source: `packages/types/src/provider-settings.ts` — `providerSettingsSchema`
/// This is a flattened union of all provider-specific schemas.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    // --- Core ---
    pub api_provider: Option<ProviderName>,

    // --- Base provider settings (shared by all) ---
    pub include_max_tokens: Option<bool>,
    pub todo_list_enabled: Option<bool>,
    pub model_temperature: Option<Option<f64>>, // nullish → Option<Option<f64>>
    pub rate_limit_seconds: Option<u64>,
    pub consecutive_mistake_limit: Option<u32>,

    // --- Model reasoning ---
    pub enable_reasoning_effort: Option<bool>,
    pub reasoning_effort: Option<ReasoningEffortSetting>,
    pub model_max_tokens: Option<u64>,
    pub model_max_thinking_tokens: Option<u64>,

    // --- Model verbosity ---
    pub verbosity: Option<VerbosityLevel>,

    // --- Common model ID (used by many providers) ---
    pub api_model_id: Option<String>,

    // --- Anthropic-specific ---
    pub api_key: Option<String>,
    pub anthropic_base_url: Option<String>,
    pub anthropic_use_auth_token: Option<bool>,
    pub anthropic_use_extended_thinking: Option<bool>,
    pub anthropic_beta_1m_context: Option<bool>,

    // --- OpenRouter-specific ---
    pub open_router_api_key: Option<String>,
    pub open_router_model_id: Option<String>,
    pub open_router_base_url: Option<String>,
    pub open_router_specific_provider: Option<String>,

    // --- AWS Bedrock-specific ---
    pub aws_access_key: Option<String>,
    pub aws_secret_key: Option<String>,
    pub aws_session_token: Option<String>,
    pub aws_region: Option<String>,
    pub aws_use_cross_region_inference: Option<bool>,
    pub aws_use_global_inference: Option<bool>,
    pub aws_use_prompt_cache: Option<bool>,
    pub aws_profile: Option<String>,
    pub aws_use_profile: Option<bool>,
    pub aws_api_key: Option<String>,
    pub aws_use_api_key: Option<bool>,
    pub aws_custom_arn: Option<String>,
    pub aws_model_context_window: Option<u64>,
    pub aws_bedrock_endpoint_enabled: Option<bool>,
    pub aws_bedrock_endpoint: Option<String>,
    pub aws_bedrock_custom_model_id: Option<String>,
    pub aws_bedrock_1m_context: Option<bool>,
    pub aws_bedrock_service_tier: Option<AwsBedrockServiceTier>,

    // --- Vertex-specific ---
    pub vertex_key_file: Option<String>,
    pub vertex_json_credentials: Option<String>,
    pub vertex_project_id: Option<String>,
    pub vertex_region: Option<String>,
    pub vertex_1m_context: Option<bool>,

    // --- OpenAI-specific ---
    pub open_ai_base_url: Option<String>,
    pub open_ai_api_key: Option<String>,
    pub open_ai_r1_format_enabled: Option<bool>,
    pub open_ai_model_id: Option<String>,
    pub open_ai_custom_model_info: Option<Box<ModelInfo>>,
    pub open_ai_use_azure: Option<bool>,
    pub azure_api_version: Option<String>,
    pub open_ai_streaming_enabled: Option<bool>,
    pub open_ai_host_header: Option<String>,
    pub open_ai_headers: Option<std::collections::HashMap<String, String>>,
    pub open_ai_org_id: Option<String>,
    pub open_ai_use_legacy_completion: Option<bool>,

    // --- Ollama-specific ---
    pub ollama_model_id: Option<String>,
    pub ollama_base_url: Option<String>,
    pub ollama_api_key: Option<String>,
    pub ollama_num_ctx: Option<u64>,
    pub ollama_api_options: Option<serde_json::Value>,

    // --- VS Code LM-specific ---
    pub vs_code_lm_model_selector: Option<serde_json::Value>,

    // --- LM Studio-specific ---
    pub lm_studio_model_id: Option<String>,
    pub lm_studio_base_url: Option<String>,
    pub lm_studio_draft_model_id: Option<String>,
    pub lm_studio_speculative_decoding_enabled: Option<bool>,

    // --- Google Gemini-specific ---
    pub gemini_api_key: Option<String>,
    pub google_gemini_base_url: Option<String>,
    pub google_api_key: Option<String>,
    pub gemini_base_url: Option<String>,

    // --- Gemini CLI-specific ---
    pub gemini_cli_oauth_path: Option<String>,
    pub gemini_cli_project_id: Option<String>,

    // --- OpenAI Native-specific ---
    pub open_ai_native_api_key: Option<String>,
    pub open_ai_native_base_url: Option<String>,
    pub open_ai_native_model_max_tokens: Option<u64>,
    pub open_ai_native_reasoning_effort: Option<String>,
    pub open_ai_native_service_tier: Option<ServiceTier>,

    // --- OpenAI Codex-specific ---
    pub open_ai_codex_base_url: Option<String>,

    // --- Mistral-specific ---
    pub mistral_api_key: Option<String>,
    pub mistral_base_url: Option<String>,
    pub mistral_codestral_url: Option<String>,

    // --- DeepSeek-specific ---
    pub deep_seek_base_url: Option<String>,
    pub deep_seek_api_key: Option<String>,

    // --- Poe-specific ---
    pub poe_api_key: Option<String>,
    pub poe_base_url: Option<String>,
    pub poe_model_id: Option<String>,

    // --- Moonshot-specific ---
    pub moonshot_base_url: Option<String>,
    pub moonshot_api_key: Option<String>,

    // --- MiniMax-specific ---
    pub minimax_base_url: Option<String>,
    pub minimax_api_key: Option<String>,

    // --- Requesty-specific ---
    pub requesty_base_url: Option<String>,
    pub requesty_api_key: Option<String>,
    pub requesty_model_id: Option<String>,

    // --- Unbound-specific ---
    pub unbound_api_key: Option<String>,
    pub unbound_model_id: Option<String>,
    pub unbound_base_url: Option<String>,

    // --- xAI-specific ---
    pub xai_base_url: Option<String>,
    pub xai_api_key: Option<String>,

    // --- LiteLLM-specific ---
    pub litellm_base_url: Option<String>,
    pub litellm_api_key: Option<String>,
    pub litellm_model_id: Option<String>,
    pub litellm_use_prompt_cache: Option<bool>,

    // --- SambaNova-specific ---
    pub samba_nova_base_url: Option<String>,
    pub samba_nova_api_key: Option<String>,

    // --- ZAI-specific ---
    pub zai_base_url: Option<String>,
    pub zai_api_key: Option<String>,
    pub zai_api_line: Option<ZaiApiLine>,

    // --- Fireworks-specific ---
    pub fireworks_base_url: Option<String>,
    pub fireworks_api_key: Option<String>,

    // --- Qwen Code-specific ---
    pub qwen_code_oauth_path: Option<String>,
    pub qwen_base_url: Option<String>,
    pub qwen_api_key: Option<String>,

    // --- Baseten-specific ---
    pub baseten_base_url: Option<String>,
    pub baseten_model_id: Option<String>,
    pub baseten_api_key: Option<String>,

    // --- Roo-specific ---
    pub roo_api_key: Option<String>,
    pub roo_base_url: Option<String>,

    // --- Vercel AI Gateway-specific ---
    pub vercel_ai_gateway_api_key: Option<String>,
    pub vercel_ai_gateway_model_id: Option<String>,
    pub vercel_base_url: Option<String>,
    pub vercel_api_key: Option<String>,
    pub vercel_model_id: Option<String>,

    // --- Fake AI ---
    pub fake_ai: Option<serde_json::Value>,

    // --- Router ---
    pub router_model_id: Option<String>,

    // --- Request settings ---
    pub include_developer_docs: Option<bool>,
    pub model_reasoning_effort: Option<String>,
    pub model_top_p: Option<f64>,
    pub request_timeout: Option<u64>,
}

// ---------------------------------------------------------------------------
// ProviderSettingsWithId
// ---------------------------------------------------------------------------

/// Provider settings with an optional ID.
///
/// Source: `packages/types/src/provider-settings.ts` — `providerSettingsWithIdSchema`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsWithId {
    #[serde(flatten)]
    pub settings: ProviderSettings,
    pub id: Option<String>,
}

// ---------------------------------------------------------------------------
// Model ID helpers
// ---------------------------------------------------------------------------

/// Keys that can hold a model ID in provider settings.
pub const MODEL_ID_KEYS: &[&str] = &[
    "apiModelId",
    "openRouterModelId",
    "openAiModelId",
    "ollamaModelId",
    "lmStudioModelId",
    "lmStudioDraftModelId",
    "requestyModelId",
    "unboundModelId",
    "litellmModelId",
    "vercelAiGatewayModelId",
];

/// Anthropic-style API providers.
pub const ANTHROPIC_STYLE_PROVIDERS: &[ProviderName] =
    &[ProviderName::Anthropic, ProviderName::Bedrock, ProviderName::MiniMax];
