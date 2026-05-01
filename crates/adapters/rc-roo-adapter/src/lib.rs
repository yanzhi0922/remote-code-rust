//! # rc-roo-adapter — In-process Roo Code Agent Adapter
//!
//! Implements the [`AgentAdapter`] trait for the Roo Code agent, enabling
//! native in-process integration (no subprocess bridge required).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │  GUI (lib.rs)       │
//! │  send_prompt()      │
//! └────────┬────────────┘
//!          │ AgentAdapter::send_message()
//! ┌────────▼────────────┐
//! │  RooInProcessAdapter│
//! │  ┌───────────────┐  │
//! │  │ dyn Provider  │  │
//! │  │  (Anthropic/  │  │
//! │  │   OpenAI/...) │  │
//! │  └───────┬───────┘  │
//! │  ┌───────▼────────┐  │
//! │  │ ToolDispatcher │  │
//! │  │ (file_ops,     │  │
//! │  │  execute_cmd)  │  │
//! │  └───────────────┘  │
//! │  Agent Loop:        │
//! │  user → API → tools │
//! │  → results → API   │
//! │  → text response    │
//! └──────────┼───────────┘
//!            │ mpsc::Receiver<UnifiedAgentEvent>
//! ┌──────────▼───────────┐
//! │  GUI event loop      │
//! └──────────────────────┘
//! ```

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use claude_agent_protocol::adapter::AgentAdapter;
use claude_agent_protocol::events::UnifiedAgentEvent;
use claude_agent_protocol::permission::PermissionDecision;
use claude_agent_protocol::types::{AgentConfig, AgentInfo, AgentStatus, AgentType};

use roo_provider::handler::{CreateMessageMetadata, Provider};
use roo_task::tool_dispatcher::{
    ToolContext, ToolDispatcher, ToolExecutionResult,
    default_dispatcher_with_terminal,
};
use roo_terminal::TerminalRegistry;
use roo_tools::definition::{NativeToolsOptions, ToolDefinition, get_native_tools};
use roo_types::api::{
    ApiMessage, ApiStreamChunk, ContentBlock, MessageRole, ToolResultContent,
};

// ---------------------------------------------------------------------------
// Provider builder — mirrors roo-cli's build_handler()
// ---------------------------------------------------------------------------

/// Build a boxed [`Provider`] based on the provider name string.
///
/// Supports all providers that the Roo CLI supports. The `provider_name`
/// should match the `ProviderName` serde value (e.g. `"anthropic"`, `"openai"`).
fn build_handler(
    provider_name: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model_id: Option<&str>,
) -> anyhow::Result<Box<dyn Provider>> {
    match provider_name {
        // ── Anthropic ──────────────────────────────────────────────────
        "anthropic" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for anthropic"))?;
            let cfg = roo_provider_anthropic::AnthropicConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_anthropic::AnthropicConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                use_extended_thinking: None,
                max_thinking_tokens: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_anthropic::AnthropicHandler::new(cfg)?))
        }

        // ── OpenAI (compatible) ────────────────────────────────────────
        "openai" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for openai"))?;
            let cfg = roo_provider_openai::OpenAiConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_openai::OpenAiConfig::DEFAULT_BASE_URL)
                    .to_string(),
                org_id: None,
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                reasoning_effort: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_openai::OpenAiHandler::new(cfg)?))
        }

        // ── OpenAI Native (Responses API) ──────────────────────────────
        "openai-native" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for openai-native"))?;
            let cfg = roo_provider_openai_native::OpenAiNativeConfig {
                api_key: api_key.to_string(),
                base_url: base_url.map(|s| s.to_string()),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                reasoning_effort: None,
                request_timeout: None,
                service_tier: None,
                enable_reasoning_summary: true,
            };
            Ok(Box::new(
                roo_provider_openai_native::OpenAiNativeHandler::new(cfg)?,
            ))
        }

        // ── OpenRouter ─────────────────────────────────────────────────
        "openrouter" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for openrouter"))?;
            let cfg = roo_provider_openrouter::OpenRouterConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_openrouter::OpenRouterConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_openrouter::OpenRouterHandler::new(cfg)?))
        }

        // ── DeepSeek ───────────────────────────────────────────────────
        "deepseek" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for deepseek"))?;
            let cfg = roo_provider_deepseek::DeepSeekConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_deepseek::DeepSeekConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_deepseek::DeepSeekHandler::new(cfg)?))
        }

        // ── Google Gemini ──────────────────────────────────────────────
        "gemini" | "google" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for gemini"))?;
            let cfg = roo_provider_google::GoogleConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_google::GoogleConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_google::GoogleHandler::new(cfg)?))
        }

        // ── Ollama ─────────────────────────────────────────────────────
        "ollama" => {
            let cfg = roo_provider_ollama::OllamaConfig {
                base_url: base_url
                    .unwrap_or(roo_provider_ollama::OllamaConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
                api_options: None,
            };
            Ok(Box::new(roo_provider_ollama::OllamaHandler::new(cfg)?))
        }

        // ── LM Studio ──────────────────────────────────────────────────
        "lmstudio" => {
            let cfg = roo_provider_lmstudio::LmStudioConfig {
                base_url: base_url
                    .unwrap_or(roo_provider_lmstudio::LmStudioConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
                speculative_decoding_enabled: false,
                draft_model_id: None,
            };
            Ok(Box::new(roo_provider_lmstudio::LmStudioHandler::new(cfg)?))
        }

        // ── xAI (Grok) ────────────────────────────────────────────────
        "xai" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for xai"))?;
            let cfg = roo_provider_xai::XaiConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_xai::XaiConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_xai::XaiHandler::new(cfg)?))
        }

        // ── Mistral ────────────────────────────────────────────────────
        "mistral" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for mistral"))?;
            let cfg = roo_provider_mistral::MistralConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_mistral::MistralConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_mistral::MistralHandler::new(cfg)?))
        }

        // ── Fireworks ──────────────────────────────────────────────────
        "fireworks" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for fireworks"))?;
            let cfg = roo_provider_fireworks::FireworksConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_fireworks::FireworksConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_fireworks::FireworksHandler::new(cfg)?))
        }

        // ── LiteLLM ────────────────────────────────────────────────────
        "litellm" => {
            let api_key = api_key
                .unwrap_or("dummy-key")
                .to_string();
            let cfg = roo_provider_litellm::LiteLlmConfig {
                api_key,
                base_url: base_url
                    .unwrap_or(roo_provider_litellm::LiteLlmConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                use_prompt_cache: false,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_litellm::LiteLlmHandler::new(cfg)?))
        }

        // ── Qwen ───────────────────────────────────────────────────────
        "qwen" | "qwen-code" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for qwen"))?;
            let cfg = roo_provider_qwen::QwenConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_qwen::QwenConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_qwen::QwenHandler::new(cfg)?))
        }

        // ── MiniMax ────────────────────────────────────────────────────
        "minimax" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for minimax"))?;
            let cfg = roo_provider_minimax::MiniMaxConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_minimax::MiniMaxConfig::DEFAULT_BASE_URL)
                    .to_string(),
                group_id: None,
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_minimax::MiniMaxHandler::new(cfg)?))
        }

        // ── Moonshot ───────────────────────────────────────────────────
        "moonshot" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for moonshot"))?;
            let cfg = roo_provider_moonshot::MoonshotConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_moonshot::MoonshotConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_moonshot::MoonshotHandler::new(cfg)?))
        }

        // ── ZAI ────────────────────────────────────────────────────────
        "zai" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for zai"))?;
            let cfg = roo_provider_zai::ZaiConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_zai::ZaiConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_zai::ZaiHandler::new(cfg)?))
        }

        // ── SambaNova ──────────────────────────────────────────────────
        "sambanova" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for sambanova"))?;
            let cfg = roo_provider_sambanova::SambaNovaConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_sambanova::SambaNovaConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_sambanova::SambaNovaHandler::new(cfg)?))
        }

        // ── Baseten ────────────────────────────────────────────────────
        "baseten" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for baseten"))?;
            let cfg = roo_provider_baseten::BasetenConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_baseten::BasetenConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_baseten::BasetenHandler::new(cfg)?))
        }

        // ── Poe ────────────────────────────────────────────────────────
        "poe" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for poe"))?;
            let cfg = roo_provider_poe::PoeConfig {
                api_key: api_key.to_string(),
                base_url: base_url.map(|s| s.to_string()),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                max_thinking_tokens: None,
                reasoning_effort: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_poe::PoeHandler::new(cfg)?))
        }

        // ── Requesty ───────────────────────────────────────────────────
        "requesty" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for requesty"))?;
            let cfg = roo_provider_requesty::RequestyConfig {
                api_key: api_key.to_string(),
                base_url: base_url
                    .unwrap_or(roo_provider_requesty::RequestyConfig::DEFAULT_BASE_URL)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_requesty::RequestyHandler::new(cfg)?))
        }

        // ── Unbound ────────────────────────────────────────────────────
        "unbound" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for unbound"))?;
            let cfg = roo_provider_unbound::UnboundConfig {
                api_key: api_key.to_string(),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_unbound::UnboundHandler::new(cfg)?))
        }

        // ── Vercel AI Gateway ──────────────────────────────────────────
        "vercel" | "vercel-ai-gateway" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for vercel"))?;
            let cfg = roo_provider_vercel::VercelConfig {
                api_key: api_key.to_string(),
                base_url: base_url.map(|s| s.to_string()),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_vercel::VercelHandler::new(cfg)?))
        }

        // ── Roo ────────────────────────────────────────────────────────
        "roo" => {
            let cfg = roo_provider_roo::RooConfig {
                api_key: api_key.map(|s| s.to_string()),
                base_url: base_url.map(|s| s.to_string())
                    .or_else(|| Some(roo_provider_roo::RooConfig::DEFAULT_BASE_URL.to_string())),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_roo::RooHandler::new(cfg)?))
        }

        // ── AWS Bedrock ────────────────────────────────────────────────
        "aws" | "bedrock" => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("api_key is required for bedrock (format: access_key:secret_key)"))?;
            let parts: Vec<&str> = api_key.splitn(2, ':').collect();
            let (access_key, secret_key) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                anyhow::bail!("api_key for bedrock must be in format access_key:secret_key");
            };
            let cfg = roo_provider_aws::AwsBedrockConfig {
                access_key,
                secret_key,
                session_token: None,
                region: base_url
                    .unwrap_or(roo_provider_aws::AwsBedrockConfig::DEFAULT_REGION)
                    .to_string(),
                model_id: model_id.map(|s| s.to_string()),
                use_cross_region_inference: false,
                endpoint_url: None,
                request_timeout: None,
                temperature: None,
            };
            Ok(Box::new(roo_provider_aws::AwsBedrockHandler::new(cfg)?))
        }

        _ => {
            anyhow::bail!(
                "Unsupported Roo provider: '{}'. Supported: anthropic, openai, openai-native, \
                 openrouter, deepseek, gemini, ollama, lmstudio, xai, mistral, fireworks, \
                 litellm, qwen, minimax, moonshot, zai, sambanova, baseten, poe, \
                 requesty, unbound, vercel, roo, bedrock",
                provider_name
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Collected stream data
// ---------------------------------------------------------------------------

/// Accumulated tool call from streaming.
struct CollectedToolCall {
    id: String,
    name: String,
    arguments: String,
}

// ---------------------------------------------------------------------------
// RooInProcessAdapter
// ---------------------------------------------------------------------------

/// In-process adapter for the Roo Code agent.
///
/// Wraps a Roo [`Provider`] and [`ToolDispatcher`] and runs the full
/// agent loop (user → API → tools → results → API → text) in a background
/// task, streaming [`UnifiedAgentEvent`]s through an mpsc channel.
pub struct RooInProcessAdapter {
    /// Static agent metadata.
    info: AgentInfo,
    /// Runtime status.
    status: AgentStatus,
    /// Working directory for tool execution.
    cwd: PathBuf,
    /// Model override.
    model: Option<String>,
    /// API key for the provider.
    api_key: Option<String>,
    /// Provider name (e.g. "anthropic", "openai").
    provider_name: Option<String>,
    /// Base URL override.
    base_url: Option<String>,
    /// Cancellation token for the running task.
    cancel_token: Option<tokio_util::sync::CancellationToken>,
    /// Background worker handle.
    worker_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RooInProcessAdapter {
    /// Create a new adapter in the **Starting** state.
    pub fn new() -> Self {
        let mut caps = HashSet::new();
        caps.insert(claude_agent_protocol::types::AgentCapability::Streaming);
        caps.insert(claude_agent_protocol::types::AgentCapability::ToolUse);
        caps.insert(claude_agent_protocol::types::AgentCapability::McpSupport);
        caps.insert(claude_agent_protocol::types::AgentCapability::Subtasks);
        caps.insert(claude_agent_protocol::types::AgentCapability::Permissions);

        Self {
            info: AgentInfo {
                name: "Roo In-Process".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities: caps,
                status: AgentStatus::Starting,
            },
            status: AgentStatus::Starting,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model: None,
            api_key: None,
            provider_name: None,
            base_url: None,
            cancel_token: None,
            worker_handle: None,
        }
    }

    /// Build the system prompt using Roo's prompt system.
    fn build_system_prompt(&self) -> String {
        // Use a basic system prompt for now. Full integration would use
        // roo_prompt::build_system_prompt() with mode and tool definitions.
        format!(
            "You are Roo, a skilled software engineer. You are working in: {}\n\
             Assist the user by reading, writing, and executing code as needed.\n\
             Use the available tools to accomplish the user's tasks.",
            self.cwd.display()
        )
    }

    /// Build tool definitions as JSON values.
    fn build_tools_json() -> Vec<serde_json::Value> {
        let options = NativeToolsOptions::default();
        let tools: Vec<ToolDefinition> = get_native_tools(options);
        tools
            .into_iter()
            .map(|td| {
                serde_json::json!({
                    "name": td.name,
                    "description": td.description,
                    "inputSchema": td.parameters,
                })
            })
            .collect()
    }

    /// Build a ToolDispatcher with terminal support.
    fn build_dispatcher(cwd: &PathBuf) -> ToolDispatcher {
        let registry = Arc::new(TerminalRegistry::new());
        let output_dir = cwd.join(".roo");
        default_dispatcher_with_terminal(registry, output_dir, "code")
    }
}

// ---------------------------------------------------------------------------
// AgentAdapter implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl AgentAdapter for RooInProcessAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        info!(working_dir = ?config.working_dir, "Starting Roo In-Process adapter");

        // Extract configuration
        if let Some(ref wd) = config.working_dir {
            self.cwd = wd.clone();
        }
        self.model = config.model.clone();
        self.api_key = config.api_key.clone();
        self.base_url = config.base_url.clone();

        // Extract provider name from config.provider or config.agent_type
        self.provider_name = config.provider.clone().or_else(|| {
            // Try to infer from agent_type serialization
            match config.agent_type {
                AgentType::RemoteRoo => Some("anthropic".to_string()),
                _ => None,
            }
        });

        let provider_name = self.provider_name.as_deref()
            .ok_or_else(|| anyhow::anyhow!("provider name is required for Roo adapter"))?;

        // Verify the provider handler can be built (but don't store it —
        // we'll create a fresh one per send_message call since dyn Provider
        // is not Clone).
        let _ = build_handler(
            provider_name,
            self.api_key.as_deref(),
            self.base_url.as_deref(),
            self.model.as_deref(),
        )?;

        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;

        info!(provider = %provider_name, "Roo In-Process adapter started successfully");
        Ok(())
    }

    async fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        // Create the event channel
        let (tx, rx) = mpsc::channel(256);

        // Create cancellation token
        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.cancel_token = Some(cancel_token.clone());

        // Build initial messages
        let user_message = ApiMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: message.to_string(),
            }],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        };

        // Clone/move everything into the background task
        let system_prompt = self.build_system_prompt();
        let tools_json = Self::build_tools_json();
        let cwd = self.cwd.clone();
        let session_id_owned = session_id.to_string();

        // Create a fresh handler and dispatcher for the background task
        let bg_handler = build_handler(
            self.provider_name.as_deref().unwrap_or("anthropic"),
            self.api_key.as_deref(),
            self.base_url.as_deref(),
            self.model.as_deref(),
        )?;
        let bg_dispatcher = Self::build_dispatcher(&cwd);

        let worker = tokio::spawn(async move {
            run_agent_loop(
                bg_handler,
                bg_dispatcher,
                &tools_json,
                &system_prompt,
                user_message,
                &cwd,
                &session_id_owned,
                tx,
                cancel_token,
            )
            .await;
        });

        self.worker_handle = Some(worker);

        Ok(rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> anyhow::Result<()> {
        info!("Cancelling Roo task");

        // Trigger cancellation
        if let Some(ref token) = self.cancel_token {
            token.cancel();
        }

        // Abort the worker task
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }

        Ok(())
    }

    async fn resolve_permission(
        &mut self,
        _session_id: &str,
        _request_id: &str,
        _decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        // Roo's permission handling is integrated into the tool dispatcher.
        // For now, this is a no-op. Full integration will wire this to
        // the tool approval flow.
        warn!("resolve_permission called on Roo adapter — not yet fully wired");
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping Roo In-Process adapter");

        // Cancel any running task
        if let Some(ref token) = self.cancel_token {
            token.cancel();
        }
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }

        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;

        Ok(())
    }

    fn is_alive(&self) -> bool {
        matches!(
            self.status,
            AgentStatus::Ready | AgentStatus::Busy
        )
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::RemoteRoo
    }
}

// ---------------------------------------------------------------------------
// Agent loop — runs in background task
// ---------------------------------------------------------------------------

/// Run the full agent loop: user → API → tools → results → API → text.
///
/// This mirrors the CLI's `run_single()` but sends events through `tx`
/// instead of printing to stdout.
async fn run_agent_loop(
    handler: Box<dyn Provider>,
    dispatcher: ToolDispatcher,
    tools_json: &[serde_json::Value],
    system_prompt: &str,
    initial_message: ApiMessage,
    cwd: &PathBuf,
    session_id: &str,
    tx: mpsc::Sender<UnifiedAgentEvent>,
    cancel_token: tokio_util::sync::CancellationToken,
) {
    let mut caps = HashSet::new();
    caps.insert(claude_agent_protocol::types::AgentCapability::Streaming);
    caps.insert(claude_agent_protocol::types::AgentCapability::ToolUse);

    // Emit Started event
    let _ = tx.send(UnifiedAgentEvent::Started(AgentInfo {
        name: "Roo In-Process".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        capabilities: caps,
        status: AgentStatus::Busy,
    })).await;

    let _ = tx.send(UnifiedAgentEvent::Ready).await;

    let mut messages = vec![initial_message];
    let mut final_text = String::new();
    let mut total_tool_calls: Vec<claude_agent_protocol::events::ToolCallInfo> = Vec::new();
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let max_turns = 50; // Safety limit

    for _turn in 0..max_turns {
        // Check cancellation
        if cancel_token.is_cancelled() {
            let _ = tx.send(UnifiedAgentEvent::Stopped).await;
            return;
        }

        // Call the provider
        let metadata = CreateMessageMetadata::default();
        let stream = match handler
            .create_message(system_prompt, messages.clone(), Some(tools_json.to_vec()), metadata)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(UnifiedAgentEvent::Error {
                    session_id: session_id.to_string(),
                    message: format!("Provider error: {e}"),
                    recoverable: false,
                }).await;
                return;
            }
        };

        // Collect the stream, forwarding text deltas
        let (text, tool_calls) = collect_stream_and_forward(
            stream,
            session_id,
            &tx,
            &cancel_token,
        ).await;

        // Update token tracking (approximate)
        total_input_tokens += text.len() as u64 / 4; // rough estimate
        total_output_tokens += text.len() as u64 / 4;

        // Build assistant message content blocks
        let mut assistant_content: Vec<ContentBlock> = Vec::new();
        if !text.is_empty() {
            assistant_content.push(ContentBlock::Text {
                text: text.clone(),
            });
        }
        for tc in &tool_calls {
            let input: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
            assistant_content.push(ContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input,
            });
        }

        messages.push(ApiMessage {
            role: MessageRole::Assistant,
            content: assistant_content,
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        });

        // If no tool calls, we're done
        if tool_calls.is_empty() {
            final_text = text;
            break;
        }

        // Execute tool calls
        let mut tool_results: Vec<ContentBlock> = Vec::new();
        for tc in &tool_calls {
            // Check cancellation
            if cancel_token.is_cancelled() {
                let _ = tx.send(UnifiedAgentEvent::Stopped).await;
                return;
            }

            // Parse tool input
            let params: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));

            // Emit tool started event
            let _ = tx.send(UnifiedAgentEvent::ToolCallStarted {
                session_id: session_id.to_string(),
                tool_name: tc.name.clone(),
                tool_input: params.clone(),
            }).await;

            let context = ToolContext::new(cwd, session_id);
            let result = dispatcher.dispatch(&tc.name, params, &context).await;

            let (output_text, is_error) = match result {
                ToolExecutionResult { text, is_error, .. } => (text, is_error),
            };

            let output_value = serde_json::Value::String(output_text.clone());

            // Record tool call info
            total_tool_calls.push(claude_agent_protocol::events::ToolCallInfo {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({})),
                output: output_value.clone(),
            });

            // Emit tool completed event
            let _ = tx.send(UnifiedAgentEvent::ToolCallCompleted {
                session_id: session_id.to_string(),
                tool_name: tc.name.clone(),
                result: output_value,
            }).await;

            tool_results.push(ContentBlock::ToolResult {
                tool_use_id: tc.id.clone(),
                content: vec![ToolResultContent::Text {
                    text: if is_error {
                        format!("Error: {}", output_text)
                    } else {
                        output_text
                    },
                }],
                is_error: if is_error { Some(true) } else { None },
            });
        }

        // Push tool results as user message
        messages.push(ApiMessage {
            role: MessageRole::User,
            content: tool_results,
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        });
    }

    // Emit completion
    let _ = tx.send(UnifiedAgentEvent::Completed {
        session_id: session_id.to_string(),
        result: claude_agent_protocol::events::AgentResult {
            response_text: final_text,
            tool_calls: total_tool_calls,
            usage: claude_agent_protocol::events::UsageInfo {
                input_tokens: total_input_tokens,
                output_tokens: total_output_tokens,
                cache_read: 0,
                cache_write: 0,
            },
            cost: None,
        },
    }).await;

    debug!("Roo agent loop completed");
}

/// Collect a provider stream, forwarding text deltas as events.
async fn collect_stream_and_forward(
    mut stream: roo_provider::handler::ApiStream,
    session_id: &str,
    tx: &mpsc::Sender<UnifiedAgentEvent>,
    cancel_token: &tokio_util::sync::CancellationToken,
) -> (String, Vec<CollectedToolCall>) {
    let mut collected_text = String::new();
    let mut tool_calls: Vec<CollectedToolCall> = Vec::new();
    let mut tool_call_index: HashMap<String, usize> = HashMap::new();

    while let Some(chunk_result) = stream.next().await {
        // Check cancellation
        if cancel_token.is_cancelled() {
            break;
        }

        match chunk_result {
            Ok(chunk) => match &chunk {
                ApiStreamChunk::Text { text } => {
                    let _ = tx.send(UnifiedAgentEvent::MessageDelta {
                        session_id: session_id.to_string(),
                        delta: text.clone(),
                    }).await;
                    collected_text.push_str(text);
                }
                ApiStreamChunk::Reasoning { text, .. } => {
                    // Forward reasoning as message delta (dimmed)
                    let _ = tx.send(UnifiedAgentEvent::MessageDelta {
                        session_id: session_id.to_string(),
                        delta: format!("[thinking] {}", text),
                    }).await;
                }
                // Complete tool call (some providers emit this instead of Start/Delta/End)
                ApiStreamChunk::ToolCall { id, name, arguments } => {
                    tool_calls.push(CollectedToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                    });
                }
                ApiStreamChunk::ToolCallStart { id, name } => {
                    let idx = tool_calls.len();
                    tool_calls.push(CollectedToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: String::new(),
                    });
                    tool_call_index.insert(id.clone(), idx);
                }
                ApiStreamChunk::ToolCallDelta { id, delta } => {
                    if let Some(&idx) = tool_call_index.get(id) {
                        tool_calls[idx].arguments.push_str(delta);
                    }
                }
                ApiStreamChunk::Usage {
                    input_tokens,
                    output_tokens,
                    ..
                } => {
                    let _ = tx.send(UnifiedAgentEvent::ContextUsage {
                        session_id: session_id.to_string(),
                        used: (*input_tokens + *output_tokens) as usize,
                        total: 200_000, // approximate context window
                    }).await;
                }
                _ => {
                    // Ignore other chunk types (ToolCallEnd, ThinkingComplete,
                    // ToolCallPartial, Grounding, Error, etc.)
                }
            },
            Err(e) => {
                warn!(error = %e, "Stream error in Roo agent loop");
                break;
            }
        }
    }

    (collected_text, tool_calls)
}
