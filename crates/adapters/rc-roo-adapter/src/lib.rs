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
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, info, warn};

use claude_agent_protocol::adapter::AgentAdapter;
use claude_agent_protocol::events::UnifiedAgentEvent;
use claude_agent_protocol::permission::PermissionDecision;
use claude_agent_protocol::types::{AgentConfig, AgentInfo, AgentStatus, AgentType};

use roo_prompt::build_system_prompt;
use roo_provider::handler::{CreateMessageMetadata, Provider};
use roo_task::tool_dispatcher::{
    AutoApprovalChecker, AutoApprovalResult, ToolContext, ToolDispatcher, ToolExecutionResult,
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
// Collected stream data
// ---------------------------------------------------------------------------

struct CollectedToolCall {
    id: String,
    name: String,
    arguments: String,
}

// ---------------------------------------------------------------------------
// Pending permission
// ---------------------------------------------------------------------------

struct PendingApproval {
    response_tx: oneshot::Sender<bool>,
}

// ---------------------------------------------------------------------------
// Stream collection result
// ---------------------------------------------------------------------------

struct StreamCollectionResult {
    text: String,
    tool_calls: Vec<CollectedToolCall>,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
}

// ---------------------------------------------------------------------------
// RooInProcessAdapter
// ---------------------------------------------------------------------------

pub struct RooInProcessAdapter {
    info: AgentInfo,
    status: AgentStatus,
    cwd: PathBuf,
    model: Option<String>,
    api_key: Option<String>,
    provider_name: Option<String>,
    base_url: Option<String>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
    worker_handle: Option<tokio::task::JoinHandle<()>>,
    conversation_history: Arc<Mutex<Vec<ApiMessage>>>,
    pending_approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
    auto_approval: AutoApprovalChecker,
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
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            auto_approval: AutoApprovalChecker::new(true),
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

    fn build_dispatcher(cwd: &std::path::Path) -> ToolDispatcher {
        let registry = Arc::new(TerminalRegistry::new());
        let output_dir = cwd.join(".roo");
        default_dispatcher_with_terminal(registry, output_dir, "code")
    }

    pub fn set_auto_approval_enabled(&mut self, enabled: bool) {
        self.auto_approval = AutoApprovalChecker::new(enabled);
    }

    pub async fn resolve_roo_approval(
        &self,
        request_id: &str,
        approved: bool,
    ) -> anyhow::Result<()> {
        let mut pending = self.pending_approvals.lock().await;
        if let Some(entry) = pending.remove(request_id) {
            let _ = entry.response_tx.send(approved);
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "No pending Roo approval found for request_id: {}",
                request_id
            ))
        }
    }

    pub async fn clear_history(&self) {
        self.conversation_history.lock().await.clear();
    }
}

// ---------------------------------------------------------------------------
// AgentAdapter implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl AgentAdapter for RooInProcessAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        info!(working_dir = ?config.working_dir, "Starting Roo In-Process adapter");

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
        let (tx, rx) = mpsc::channel(256);

        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.cancel_token = Some(cancel_token.clone());

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

        {
            let mut history = self.conversation_history.lock().await;
            history.push(user_message.clone());
        }

        let system_prompt = self.build_system_prompt();
        let tools_json = Self::build_tools_json();
        let cwd = self.cwd.clone();
        let session_id_owned = session_id.to_string();

        let bg_handler = build_handler(
            self.provider_name
                .as_deref()
                .unwrap_or("anthropic"),
            self.api_key.as_deref(),
            self.base_url.as_deref(),
            self.model.as_deref(),
        )?;
        let bg_dispatcher = Self::build_dispatcher(&cwd);
        let history = Arc::clone(&self.conversation_history);
        let pending_approvals = Arc::clone(&self.pending_approvals);
        let auto_approval = self.auto_approval.clone();

        let worker = tokio::spawn(async move {
            run_agent_loop(
                bg_handler,
                bg_dispatcher,
                &tools_json,
                &system_prompt,
                history,
                pending_approvals,
                auto_approval,
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

        if let Some(ref token) = self.cancel_token {
            token.cancel();
        }

        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }

        Ok(())
    }

    async fn resolve_permission(
        &mut self,
        _session_id: &str,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        let approved = matches!(decision, PermissionDecision::Allow | PermissionDecision::AllowAll);
        self.resolve_roo_approval(request_id, approved).await
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping Roo In-Process adapter");

        if let Some(ref token) = self.cancel_token {
            token.cancel();
        }
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }

        self.conversation_history.lock().await.clear();
        self.pending_approvals.lock().await.clear();

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

#[allow(clippy::too_many_arguments)]
async fn run_agent_loop(
    handler: Box<dyn Provider>,
    dispatcher: ToolDispatcher,
    tools_json: &[serde_json::Value],
    system_prompt: &str,
    history: Arc<Mutex<Vec<ApiMessage>>>,
    pending_approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
    auto_approval: AutoApprovalChecker,
    cwd: &std::path::Path,
    session_id: &str,
    tx: mpsc::Sender<UnifiedAgentEvent>,
    cancel_token: tokio_util::sync::CancellationToken,
) {
    let mut caps = HashSet::new();
    caps.insert(claude_agent_protocol::types::AgentCapability::Streaming);
    caps.insert(claude_agent_protocol::types::AgentCapability::ToolUse);

    let _ = tx
        .send(UnifiedAgentEvent::Started(AgentInfo {
            name: "Roo In-Process".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            capabilities: caps,
            status: AgentStatus::Busy,
        }))
        .await;

    let _ = tx.send(UnifiedAgentEvent::Ready).await;

    let messages = history.lock().await.clone();

    let mut final_text = String::new();
    let mut total_tool_calls: Vec<claude_agent_protocol::events::ToolCallInfo> = Vec::new();
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut total_cache_read: u64 = 0;
    let mut total_cache_write: u64 = 0;
    let max_turns = 50;

    let mut loop_messages = messages;

    for _turn in 0..max_turns {
        if cancel_token.is_cancelled() {
            let _ = tx.send(UnifiedAgentEvent::Stopped).await;
            return;
        }

        let metadata = CreateMessageMetadata::default();
        let stream = match handler
            .create_message(
                system_prompt,
                loop_messages.clone(),
                Some(tools_json.to_vec()),
                metadata,
            )
            .await
        {
            Ok(s) => s,
            Err(e) => {
                let _ = tx
                    .send(UnifiedAgentEvent::Error {
                        session_id: session_id.to_string(),
                        message: format!("Provider error: {e}"),
                        recoverable: false,
                    })
                    .await;
                return;
            }
        };

        let collection = collect_stream_and_forward(stream, session_id, &tx, &cancel_token).await;

        total_input_tokens += collection.input_tokens;
        total_output_tokens += collection.output_tokens;
        total_cache_read += collection.cache_read_tokens;
        total_cache_write += collection.cache_write_tokens;

        let mut assistant_content: Vec<ContentBlock> = Vec::new();
        if !collection.text.is_empty() {
            assistant_content.push(ContentBlock::Text {
                text: collection.text.clone(),
            });
        }
        for tc in &collection.tool_calls {
            let input: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
            assistant_content.push(ContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input,
            });
        }

        let assistant_msg = ApiMessage {
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
        };
        loop_messages.push(assistant_msg.clone());
        {
            let mut h = history.lock().await;
            h.push(assistant_msg);
        }

        if collection.tool_calls.is_empty() {
            final_text = collection.text;
            break;
        }

        let mut tool_results: Vec<ContentBlock> = Vec::new();
        for tc in &collection.tool_calls {
            if cancel_token.is_cancelled() {
                let _ = tx.send(UnifiedAgentEvent::Stopped).await;
                return;
            }

            let params: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));

            let approval_result = auto_approval.check(&tc.name, &params);
            match approval_result {
                AutoApprovalResult::Denied { reason } => {
                    let denied_output = format!("Tool denied: {reason}");
                    let output_value = serde_json::Value::String(denied_output.clone());

                    total_tool_calls.push(claude_agent_protocol::events::ToolCallInfo {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: serde_json::from_str(&tc.arguments)
                            .unwrap_or(serde_json::json!({})),
                        output: output_value.clone(),
                    });

                    let _ = tx
                        .send(UnifiedAgentEvent::ToolCallCompleted {
                            session_id: session_id.to_string(),
                            tool_name: tc.name.clone(),
                            result: output_value,
                        })
                        .await;

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: tc.id.clone(),
                        content: vec![ToolResultContent::Text {
                            text: denied_output,
                        }],
                        is_error: Some(true),
                    });
                    continue;
                }
                AutoApprovalResult::RequiresApproval { reason } => {
                    let request_id = uuid::Uuid::new_v4().to_string();

                    let _ = tx
                        .send(UnifiedAgentEvent::PermissionRequest {
                            session_id: session_id.to_string(),
                            request_id: request_id.clone(),
                            tool_name: tc.name.clone(),
                            input: params.clone(),
                        })
                        .await;

                    let (approval_tx, approval_rx) = oneshot::channel();
                    {
                        let mut pending = pending_approvals.lock().await;
                        pending.insert(
                            request_id.clone(),
                            PendingApproval {
                                response_tx: approval_tx,
                            },
                        );
                    }

                    let _ = tx
                        .send(UnifiedAgentEvent::ToolCallStarted {
                            session_id: session_id.to_string(),
                            tool_name: format!("{} (pending approval: {reason})", tc.name),
                            tool_input: params.clone(),
                        })
                        .await;

                    let approved: bool = approval_rx.await.unwrap_or_default();

                    if cancel_token.is_cancelled() {
                        let _ = tx.send(UnifiedAgentEvent::Stopped).await;
                        return;
                    }

                    if !approved {
                        let denied_output = "User denied tool execution".to_string();
                        let output_value = serde_json::Value::String(denied_output.clone());

                        total_tool_calls.push(claude_agent_protocol::events::ToolCallInfo {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            input: serde_json::from_str(&tc.arguments)
                                .unwrap_or(serde_json::json!({})),
                            output: output_value.clone(),
                        });

                        let _ = tx
                            .send(UnifiedAgentEvent::ToolCallCompleted {
                                session_id: session_id.to_string(),
                                tool_name: tc.name.clone(),
                                result: output_value,
                            })
                            .await;

                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: tc.id.clone(),
                            content: vec![ToolResultContent::Text {
                                text: denied_output,
                            }],
                            is_error: Some(true),
                        });
                        continue;
                    }
                }
                AutoApprovalResult::Approved => {}
            }

            let _ = tx
                .send(UnifiedAgentEvent::ToolCallStarted {
                    session_id: session_id.to_string(),
                    tool_name: tc.name.clone(),
                    tool_input: params.clone(),
                })
                .await;

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

            let _ = tx
                .send(UnifiedAgentEvent::ToolCallCompleted {
                    session_id: session_id.to_string(),
                    tool_name: tc.name.clone(),
                    result: output_value,
                })
                .await;

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

        let tool_results_msg = ApiMessage {
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
        };
        loop_messages.push(tool_results_msg.clone());
        {
            let mut h = history.lock().await;
            h.push(tool_results_msg);
        }
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
                cache_read: total_cache_read,
                cache_write: total_cache_write,
            },
            cost: None,
        },
    })
    .await;

    debug!("Roo agent loop completed");
}

async fn collect_stream_and_forward(
    mut stream: roo_provider::handler::ApiStream,
    session_id: &str,
    tx: &mpsc::Sender<UnifiedAgentEvent>,
    cancel_token: &tokio_util::sync::CancellationToken,
) -> StreamCollectionResult {
    let mut collected_text = String::new();
    let mut tool_calls: Vec<CollectedToolCall> = Vec::new();
    let mut tool_call_index: HashMap<String, usize> = HashMap::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_write_tokens: u64 = 0;

    while let Some(chunk_result) = stream.next().await {
        if cancel_token.is_cancelled() {
            break;
        }

        match chunk_result {
            Ok(chunk) => match &chunk {
                ApiStreamChunk::Text { text } => {
                    let _ = tx
                        .send(UnifiedAgentEvent::MessageDelta {
                            session_id: session_id.to_string(),
                            delta: text.clone(),
                        })
                        .await;
                    collected_text.push_str(text);
                }
                ApiStreamChunk::Reasoning { text, .. } => {
                    let _ = tx
                        .send(UnifiedAgentEvent::MessageDelta {
                            session_id: session_id.to_string(),
                            delta: format!("[thinking] {}", text),
                        })
                        .await;
                }
                ApiStreamChunk::ToolCall {
                    id,
                    name,
                    arguments,
                } => {
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
                    input_tokens: it,
                    output_tokens: ot,
                    cache_read_tokens: crt,
                    cache_write_tokens: cwt,
                    ..
                } => {
                    input_tokens = *it;
                    output_tokens = *ot;
                    if let Some(crt) = crt {
                        cache_read_tokens = *crt;
                    }
                    if let Some(cwt) = cwt {
                        cache_write_tokens = *cwt;
                    }
                    let _ = tx
                        .send(UnifiedAgentEvent::ContextUsage {
                            session_id: session_id.to_string(),
                            used: (input_tokens + output_tokens) as usize,
                            total: 200_000,
                        })
                        .await;
                }
                _ => {}
            },
            Err(e) => {
                warn!(error = %e, "Stream error in Roo agent loop");
                break;
            }
        }
    }

    StreamCollectionResult {
        text: collected_text,
        tool_calls,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
    }
}
