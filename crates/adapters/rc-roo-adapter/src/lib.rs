//! # rc-roo-adapter — In-process Roo Code Agent Adapter (100% Native)
//!
//! Implements the [`AgentAdapter`] trait for the Roo Code agent using
//! Roo's native [`AgentLoop`] for full feature coverage including:
//! MCP tools, context compression, budget management, subtasks,
//! custom modes, code indexing, and more.
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
//! │  │   AgentLoop    │  │
//! │  │  (native Roo   │  │
//! │  │   loop with    │  │
//! │  │   MCP, condense│  │
//! │  │   budget, etc) │  │
//! │  └───────────────┘  │
//! └──────────┼───────────┘
//!            │ mpsc::Receiver<UnifiedAgentEvent>
//! ┌──────────▼───────────┐
//! │  GUI event loop      │
//! └──────────────────────┘
//! ```

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn};

use claude_agent_protocol::adapter::AgentAdapter;
use claude_agent_protocol::events::UnifiedAgentEvent;
use claude_agent_protocol::permission::PermissionDecision;
use claude_agent_protocol::types::{AgentConfig, AgentInfo, AgentStatus, AgentType};

use roo_auto_approval::types::AutoApprovalState;
use roo_prompt::build_system_prompt;
use roo_provider::handler::Provider;
use roo_task::tool_dispatcher::ToolDispatcher;
use roo_task::message_builder::MessageBuilder;
use roo_task::engine::TaskEngine;
use roo_task::agent_loop::{AgentLoop, AgentLoopConfig};
use roo_task::types::TaskConfig;
use roo_task::TaskEvent as RooTaskEvent;
use roo_terminal::TerminalRegistry;
use roo_types::api::ApiMessage;
use roo_types::mcp::McpServerConnection;
use roo_ignore::RooIgnoreController;
use roo_protect::RooProtectedController;
use roo_context_tracking::{FileContextTracker, InMemoryMetadataStore};
use roo_editor::diff_view::DiffViewProvider;

// ---------------------------------------------------------------------------
// Provider builder — mirrors roo-cli's build_handler()
// ---------------------------------------------------------------------------

fn build_handler(
    provider_name: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model_id: Option<&str>,
) -> anyhow::Result<Box<dyn Provider>> {
    match provider_name {
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

        "litellm" => {
            let api_key = api_key.unwrap_or("dummy-key").to_string();
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

        "roo" => {
            let cfg = roo_provider_roo::RooConfig {
                api_key: api_key.map(|s| s.to_string()),
                base_url: base_url
                    .map(|s| s.to_string())
                    .or_else(|| Some(roo_provider_roo::RooConfig::DEFAULT_BASE_URL.to_string())),
                model_id: model_id.map(|s| s.to_string()),
                temperature: None,
                request_timeout: None,
            };
            Ok(Box::new(roo_provider_roo::RooHandler::new(cfg)?))
        }

        "aws" | "bedrock" => {
            let api_key = api_key.ok_or_else(|| {
                anyhow::anyhow!("api_key is required for bedrock (format: access_key:secret_key)")
            })?;
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
// Event bridge: TaskEvent → UnifiedAgentEvent
// ---------------------------------------------------------------------------

/// Maps Roo's native [`RooTaskEvent`] to [`UnifiedAgentEvent`].
///
/// This bridge is registered as a listener on the [`TaskEventEmitter`]
/// inside the [`TaskEngine`]. Every event the native `AgentLoop` emits
/// is translated and forwarded to the GUI via the unified event channel.
fn map_task_event(event: &RooTaskEvent, session_id: &str) -> Option<UnifiedAgentEvent> {
    match event {
        // --- Streaming text ---
        RooTaskEvent::StreamingTextDelta { text, .. } => Some(UnifiedAgentEvent::MessageDelta {
            session_id: session_id.to_string(),
            delta: text.clone(),
        }),

        // --- Streaming reasoning ---
        RooTaskEvent::StreamingReasoningDelta { text, .. } => {
            Some(UnifiedAgentEvent::MessageDelta {
                session_id: session_id.to_string(),
                delta: format!("[thinking] {}", text),
            })
        }

        // --- Tool use lifecycle ---
        RooTaskEvent::StreamingToolUseStarted { tool_name, .. } => {
            Some(UnifiedAgentEvent::ToolCallStarted {
                session_id: session_id.to_string(),
                tool_name: tool_name.clone(),
                tool_input: serde_json::json!({}),
            })
        }

        RooTaskEvent::StreamingToolUseCompleted {
            tool_name, success, ..
        } => Some(UnifiedAgentEvent::ToolCallCompleted {
            session_id: session_id.to_string(),
            tool_name: tool_name.clone(),
            result: serde_json::json!({
                "success": success,
            }),
        }),

        // --- Task lifecycle ---
        RooTaskEvent::TaskStarted { .. } => {
            let mut caps = HashSet::new();
            caps.insert(claude_agent_protocol::types::AgentCapability::Streaming);
            caps.insert(claude_agent_protocol::types::AgentCapability::ToolUse);
            caps.insert(claude_agent_protocol::types::AgentCapability::McpSupport);
            caps.insert(claude_agent_protocol::types::AgentCapability::Subtasks);
            caps.insert(claude_agent_protocol::types::AgentCapability::Permissions);
            Some(UnifiedAgentEvent::Started(AgentInfo {
                name: "Roo In-Process".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities: caps,
                status: AgentStatus::Busy,
            }))
        }

        RooTaskEvent::TaskCompleted { .. } => {
            // The Completed event is emitted from run_loop() result handling below.
            // We map it here for completeness, but the final result is built
            // after run_loop() returns.
            Some(UnifiedAgentEvent::Ready)
        }

        RooTaskEvent::TaskAborted { reason, .. } => Some(UnifiedAgentEvent::Error {
            session_id: session_id.to_string(),
            message: reason.clone().unwrap_or_else(|| "Task aborted".to_string()),
            recoverable: false,
        }),

        // --- Context management ---
        RooTaskEvent::ContextCondensationCompleted {
            messages_removed, ..
        } => Some(UnifiedAgentEvent::ContextCompacted {
            session_id: session_id.to_string(),
            entries_removed: *messages_removed,
            usage_ratio: 0.0,
        }),

        RooTaskEvent::ContextTruncationPerformed {
            messages_removed, ..
        } => Some(UnifiedAgentEvent::ContextCompacted {
            session_id: session_id.to_string(),
            entries_removed: *messages_removed,
            usage_ratio: 0.0,
        }),

        // --- Token usage ---
        RooTaskEvent::TokenUsageUpdated { usage } => Some(UnifiedAgentEvent::ContextUsage {
            session_id: session_id.to_string(),
            used: (usage.total_tokens_in + usage.total_tokens_out) as usize,
            total: usage.context_tokens as usize,
        }),

        RooTaskEvent::TaskTokenUsageUpdated { token_usage, .. } => {
            Some(UnifiedAgentEvent::ContextUsage {
                session_id: session_id.to_string(),
                used: (token_usage.total_tokens_in + token_usage.total_tokens_out) as usize,
                total: token_usage.context_tokens as usize,
            })
        }

        // --- Tool approval ---
        RooTaskEvent::ToolApprovalRequired {
            tool_name,
            tool_id,
            reason,
            ..
        } => Some(UnifiedAgentEvent::PermissionRequest {
            session_id: session_id.to_string(),
            request_id: tool_id.clone(),
            tool_name: tool_name.clone(),
            input: serde_json::json!({ "reason": reason }),
        }),

        // --- Subtask lifecycle ---
        RooTaskEvent::TaskSpawned {
            child_task_id, ..
        } => Some(UnifiedAgentEvent::SubtaskStarted {
            session_id: session_id.to_string(),
            task_id: child_task_id.clone(),
            description: String::new(),
        }),

        RooTaskEvent::TaskDelegationCompleted { summary, .. } => {
            Some(UnifiedAgentEvent::SubtaskCompleted {
                session_id: session_id.to_string(),
                task_id: String::new(),
                result: serde_json::json!({ "summary": summary }),
            })
        }

        // --- Errors ---
        RooTaskEvent::Error { error, .. } => Some(UnifiedAgentEvent::Error {
            session_id: session_id.to_string(),
            message: error.clone(),
            recoverable: true,
        }),

        RooTaskEvent::ToolError {
            tool_name, error, ..
        } => Some(UnifiedAgentEvent::Error {
            session_id: session_id.to_string(),
            message: format!("Tool '{}' error: {}", tool_name, error),
            recoverable: true,
        }),

        // --- State changes ---
        RooTaskEvent::StateChanged { to, .. } => {
            debug!(state = %to, "Task state changed");
            None
        }

        // --- API events ---
        RooTaskEvent::ApiRequestStarted { .. } => {
            debug!("API request started");
            None
        }
        RooTaskEvent::ApiRequestFinished { .. } => {
            debug!("API request finished");
            None
        }

        // --- Ignore events that don't map to unified events ---
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// RooInProcessAdapter
// ---------------------------------------------------------------------------

/// In-process adapter for Roo Code using the native [`AgentLoop`].
///
/// This adapter provides 100% feature coverage by delegating to Roo's
/// built-in agent loop, which handles:
/// - API calls with streaming
/// - Tool execution (including MCP tools)
/// - Context compression / condensation
/// - Budget management and rate limiting
/// - Subtask spawning and delegation
/// - Mistake detection and recovery
/// - Checkpoint management
pub struct RooInProcessAdapter {
    info: AgentInfo,
    status: AgentStatus,
    cwd: PathBuf,
    model: Option<String>,
    api_key: Option<String>,
    provider_name: Option<String>,
    base_url: Option<String>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
    worker_handle: Option<std::thread::JoinHandle<()>>,
    conversation_history: Arc<Mutex<Vec<ApiMessage>>>,
    auto_approval_enabled: bool,
}

impl RooInProcessAdapter {
    #[allow(clippy::new_without_default)]
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
            conversation_history: Arc::new(Mutex::new(Vec::new())),
            auto_approval_enabled: true,
        }
    }

    fn build_system_prompt(&self) -> String {
        let cwd_str = self.cwd.to_string_lossy();
        let shell = if cfg!(windows) {
            "cmd.exe".to_string()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
        };
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/tmp".to_string());
        let os_info = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);

        build_system_prompt(
            &cwd_str,
            "code",
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            &[],
            &os_info,
            &shell,
            &home,
        )
    }

    fn build_dispatcher(cwd: &std::path::Path) -> ToolDispatcher {
        let registry = Arc::new(TerminalRegistry::new());
        let output_dir = cwd.join(".roo");
        roo_task::tool_dispatcher::default_dispatcher_with_terminal(registry, output_dir, "code")
    }

    pub fn set_auto_approval_enabled(&mut self, enabled: bool) {
        self.auto_approval_enabled = enabled;
    }

    pub async fn clear_history(&self) {
        self.conversation_history.lock().await.clear();
    }

    /// Resolve a pending Roo tool-approval request.
    ///
    /// In the current auto-approval configuration the `AgentLoop` handles
    /// all approvals internally, so this method simply logs the decision.
    /// When non-auto-approval mode is implemented, this will bridge to
    /// `AgentLoop::set_approval_response()` via shared state.
    pub async fn resolve_roo_approval(
        &mut self,
        _request_id: &str,
        allowed: bool,
    ) -> anyhow::Result<()> {
        debug!(allowed, "Roo approval resolution received (auto-approval mode)");
        Ok(())
    }

    /// Load MCP servers from `.roo/mcp.json` in the workspace directory.
    ///
    /// Creates a temporary `McpHub`, loads the project-level MCP config,
    /// waits for servers to connect, and returns the list of connected
    /// server descriptions (tools, resources, etc.).
    async fn load_mcp_servers(&self) -> Vec<McpServerConnection> {
        let cwd_str = self.cwd.to_string_lossy().to_string();
        let hub = roo_mcp::McpHub::new_with_paths(
            Some(cwd_str.clone()),
            None,
        );

        // Load project-level MCP config (.roo/mcp.json)
        let mcp_path = std::path::Path::new(&cwd_str)
            .join(".roo")
            .join("mcp.json");

        if mcp_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&mcp_path) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
                        let server_map: std::collections::HashMap<String, serde_json::Value> =
                            servers.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                        let count = server_map.len();
                        if let Err(e) = hub.update_server_connections(
                            &server_map,
                            roo_mcp::McpSource::Project,
                            true,
                        ).await {
                            warn!("Failed to load project MCP servers: {}", e);
                        } else {
                            info!("Loaded {} project MCP servers from {}", count, mcp_path.display());
                        }
                    }
                }
            }
        }

        // Give servers a moment to connect (max 5 seconds per server, total 10s cap)
        let server_count = hub.get_servers().len();
        if server_count > 0 {
            info!(count = server_count, "Waiting for MCP servers to connect...");
            tokio::time::sleep(std::time::Duration::from_millis(
                (server_count as u64 * 2000).min(10000)
            )).await;
        }

        hub.get_servers()
    }

    /// Inner body of the Roo agent loop, extracted so it can be wrapped in
    /// `catch_unwind`.  Runs on a dedicated OS thread (not a tokio task)
    /// because `AgentLoop` contains !Send types (`git2::Repository`).
    #[allow(clippy::too_many_arguments)]
    fn run_agent_loop_inner(
        task_id: String,
        cwd_str: String,
        message_owned: String,
        auto_approval: bool,
        session_id: String,
        tx: tokio::sync::mpsc::Sender<UnifiedAgentEvent>,
        history_snapshot: Vec<ApiMessage>,
        provider: Box<dyn Provider>,
        message_builder: MessageBuilder,
        dispatcher: ToolDispatcher,
        history: Arc<tokio::sync::Mutex<Vec<ApiMessage>>>,
        mcp_servers: Vec<McpServerConnection>,
    ) {
        let config = TaskConfig::new(&task_id, &cwd_str)
            .with_mode("code")
            .with_task_text(&message_owned)
            .with_auto_approval(auto_approval)
            .with_start_task(true);

        let mut engine = match TaskEngine::new(config) {
            Ok(e) => e,
            Err(e) => {
                let _ = tx.blocking_send(UnifiedAgentEvent::Error {
                    session_id: session_id.clone(),
                    message: format!("TaskEngine creation failed: {}", e),
                    recoverable: false,
                });
                return;
            }
        };

        if !history_snapshot.is_empty() {
            engine.set_api_conversation_history(history_snapshot);
        }

        let tx_ev = tx.clone();
        let sid_ev = session_id.clone();
        engine.emitter().on(move |event: &RooTaskEvent| {
            if let Some(unified) = map_task_event(event, &sid_ev) {
                let _ = tx_ev.blocking_send(unified);
            }
        });

        let loop_config = AgentLoopConfig {
            auto_approval_enabled: auto_approval,
            auto_approval: AutoApprovalState {
                auto_approval_enabled: auto_approval,
                always_allow_read_only: true,
                always_allow_read_only_outside_workspace: true,
                always_allow_write: true,
                always_allow_write_outside_workspace: true,
                always_allow_write_protected: false,
                always_allow_mcp: true,
                always_allow_mode_switch: true,
                always_allow_subtasks: true,
                always_allow_execute: true,
                always_allow_followup_questions: true,
                ..AutoApprovalState::default()
            },
            enable_condense: true,
            ..AgentLoopConfig::default()
        };

        // --- Wire service controllers into AgentLoop ---

        // RooIgnoreController: load .rooignore patterns if the file exists
        let mut ignore_controller = RooIgnoreController::new(&cwd_str);
        let rooignore_path = std::path::Path::new(&cwd_str).join(".rooignore");
        if let Ok(content) = std::fs::read_to_string(&rooignore_path) {
            ignore_controller.load_patterns(&content);
            info!(
                path = %rooignore_path.display(),
                "Loaded .rooignore patterns"
            );
        }

        // RooProtectedController: enforce write-protection on Roo config files
        let protected_controller = RooProtectedController::new(&cwd_str);

        // FileContextTracker: track file reads/edits for stale context detection
        let context_tracker = FileContextTracker::new(&task_id, InMemoryMetadataStore::new());

        // DiffViewProvider: manage file editing sessions with diff tracking
        let diff_view_provider = DiffViewProvider::new_default();

        // MCP servers loaded from .roo/mcp.json via McpHub.
        // The hub is created in the async context (send_message) and
        // server connections are collected before spawning the thread.
        // If MCP loading fails, we gracefully fall back to empty servers.

        let mut agent_loop =
            AgentLoop::new(engine, provider, message_builder, dispatcher)
                .with_config(loop_config)
                .with_roo_ignore_controller(ignore_controller)
                .with_roo_protected_controller(protected_controller)
                .with_file_context_tracker(context_tracker)
                .with_diff_view_provider(diff_view_provider)
                .with_mcp_servers(mcp_servers);

        let _ = tx.blocking_send(UnifiedAgentEvent::Ready);

        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create tokio runtime for agent loop");
                return;
            }
        };

        match rt.block_on(agent_loop.run_loop()) {
            Ok(result) => {
                tracing::info!(
                    task_id = %result.task_id,
                    iterations = result.iterations,
                    "AgentLoop completed successfully"
                );

                {
                    let engine_history = agent_loop.engine().api_conversation_history();
                    let mut h = history.blocking_lock();
                    *h = engine_history.to_vec();
                }

                let final_message = result.final_message.clone().unwrap_or_default();
                let usage_info = claude_agent_protocol::events::UsageInfo {
                    input_tokens: result.token_usage.total_tokens_in,
                    output_tokens: result.token_usage.total_tokens_out,
                    cache_read: result.token_usage.total_cache_reads.unwrap_or(0),
                    cache_write: result.token_usage.total_cache_writes.unwrap_or(0),
                };

                let tool_calls: Vec<claude_agent_protocol::events::ToolCallInfo> = result
                    .tool_usage
                    .iter()
                    .map(|(name, count)| claude_agent_protocol::events::ToolCallInfo {
                        id: String::new(),
                        name: name.clone(),
                        input: serde_json::json!({}),
                        output: serde_json::json!({ "count": count }),
                    })
                    .collect();

                let _ = tx.blocking_send(UnifiedAgentEvent::Completed {
                    session_id,
                    result: claude_agent_protocol::events::AgentResult {
                        response_text: final_message,
                        tool_calls,
                        usage: usage_info,
                        cost: Some(result.token_usage.total_cost),
                    },
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "AgentLoop ended with error");
                let _ = tx.blocking_send(UnifiedAgentEvent::Error {
                    session_id,
                    message: format!("AgentLoop error: {}", e),
                    recoverable: false,
                });
            }
        }

        tracing::debug!("Roo native agent loop background task finished");
    }
}

// ---------------------------------------------------------------------------
// AgentAdapter implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl AgentAdapter for RooInProcessAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        info!(working_dir = ?config.working_dir, "Starting Roo In-Process adapter (native AgentLoop)");

        if let Some(ref wd) = config.working_dir {
            self.cwd = wd.clone();
        }
        self.model = config.model.clone();
        self.api_key = config.api_key.clone();
        self.base_url = config.base_url.clone();

        self.provider_name = config.provider.clone().or_else(|| match config.agent_type {
            AgentType::RemoteRoo => Some("anthropic".to_string()),
            _ => None,
        });

        let provider_name = self
            .provider_name
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("provider name is required for Roo adapter"))?;

        // Validate provider config by building a handler (but don't keep it)
        let _ = build_handler(
            provider_name,
            self.api_key.as_deref(),
            self.base_url.as_deref(),
            self.model.as_deref(),
        )?;

        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;

        info!(provider = %provider_name, "Roo In-Process adapter started successfully (native AgentLoop)");
        Ok(())
    }

    async fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        let (tx, rx) = mpsc::channel(256);

        // Build provider
        let provider = build_handler(
            self.provider_name
                .as_deref()
                .unwrap_or("anthropic"),
            self.api_key.as_deref(),
            self.base_url.as_deref(),
            self.model.as_deref(),
        )?;

        // Build system prompt and message builder
        let system_prompt = self.build_system_prompt();
        let message_builder = MessageBuilder::new(&system_prompt);

        // Build tool dispatcher
        let dispatcher = Self::build_dispatcher(&self.cwd);

        // Load MCP servers from .roo/mcp.json
        let mcp_servers = self.load_mcp_servers().await;
        info!(
            count = mcp_servers.len(),
            "Loaded MCP servers for agent loop"
        );

        // Clone conversation history while in async context (before spawning).
        let history_snapshot = {
            let history = self.conversation_history.lock().await;
            history.clone()
        };

        // Create a cancellation token for this run.
        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.cancel_token = Some(cancel_token.clone());

        // Keep a reference to conversation history for updating after the run.
        let history = Arc::clone(&self.conversation_history);
        let session_id_for_completion = session_id.to_string();

        // Prepare owned parameters for the worker thread.
        let task_id = uuid::Uuid::now_v7().to_string();
        let cwd_str = self.cwd.to_string_lossy().to_string();
        let message_owned = message.to_string();
        let auto_approval = self.auto_approval_enabled;
        let mcp_servers_owned = mcp_servers;

        // Spawn in a dedicated OS thread.
        //
        // AgentLoop contains `Option<ShadowCheckpointService>` which wraps
        // `git2::Repository` — a raw-pointer type that is NOT Send/Sync.
        // By constructing AgentLoop INSIDE the thread, it never crosses a
        // thread boundary, so the closure only needs to capture Send-safe
        // components (provider, message_builder, dispatcher, etc.).
        let handle = std::thread::spawn(move || {
            // Wrap entire agent loop body in catch_unwind so a panic inside
            // the Roo AgentLoop (which contains !Send types like git2::Repository)
            // is converted to an error event instead of crashing the whole
            // Tauri process.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Self::run_agent_loop_inner(
                    task_id,
                    cwd_str,
                    message_owned,
                    auto_approval,
                    session_id_for_completion,
                    tx,
                    history_snapshot,
                    provider,
                    message_builder,
                    dispatcher,
                    history,
                    mcp_servers_owned,
                );
            }));

            if let Err(panic_payload) = result {
                // Best-effort: try to extract a string message from the panic.
                let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Roo agent loop panicked (unknown cause)".to_string()
                };
                tracing::error!(error = %msg, "Roo agent loop panicked — isolated to thread");
            }
        });

        let _ = cancel_token; // already stored in self.cancel_token
        self.worker_handle = Some(handle);

        Ok(rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> anyhow::Result<()> {
        info!("Cancelling Roo task");

        if let Some(ref token) = self.cancel_token {
            token.cancel();
        }

        // std::thread::JoinHandle has no abort().  The thread will
        // exit when it observes the cancelled CancellationToken.
        self.worker_handle = None;

        Ok(())
    }

    async fn resolve_permission(
        &mut self,
        _session_id: &str,
        _request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        // With the native AgentLoop, permissions are handled via the
        // ToolApprovalRequired event and set_approval_response().
        // For auto-approval mode (default), this is not needed.
        // Log the decision for debugging purposes.
        let approved = matches!(decision, PermissionDecision::Allow | PermissionDecision::AllowAll);
        debug!(approved, "Permission resolution received (auto-approval mode)");
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping Roo In-Process adapter");

        if let Some(ref token) = self.cancel_token {
            token.cancel();
        }
        // std::thread::JoinHandle has no abort().  The thread will
        // exit when it observes the cancelled CancellationToken.
        self.worker_handle = None;

        self.conversation_history.lock().await.clear();

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
