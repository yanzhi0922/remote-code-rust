//! Codex in-process adapter.
//!
//! [`CodexInProcessAdapter`] wraps the Codex `AppServerClient` (either in-process
//! or remote) and implements the [`AgentAdapter`] trait from `rc-agent-protocol`.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │  CodexInProcessAdapter                       │
//! │  ┌──────────────┐  ┌───────────────────────┐ │
//! │  │ request_handle│  │ event_pump (bg task)  │ │
//! │  │ (Clone)       │  │ owns AppServerClient  │ │
//! │  │               │  │ loops next_event()    │ │
//! │  │ - request()   │  │ maps via event_mapper │ │
//! │  │ - resolve()   │  │ forwards to event_tx  │ │
//! │  │ - reject()    │  └───────────┬───────────┘ │
//! │  └──────┬───────┘              │             │
//! │         │          ┌───────────▼───────────┐ │
//! │         │          │ Arc<Mutex<Option<tx>>> │ │
//! │         │          │ (shared event router)  │ │
//! │         │          └───────────┬───────────┘ │
//! │  send_message() installs new rx│             │
//! │  cancel() sends TurnInterrupt  │             │
//! │  resolve_permission() resolves │             │
//! └──────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use rc_codex_adapter::CodexInProcessAdapter;
//!
//! // All-in-one: creates an isolated Codex runtime and wraps it.
//! let mut adapter = CodexInProcessAdapter::start_in_process(
//!     working_dir,
//!     Some("gpt-4o".to_string()),
//! ).await?;
//!
//! // Start the adapter (spawns event pump).
//! let config = AgentConfig { .. };
//! adapter.start(&config).await?;
//!
//! // Send a message and receive streaming events.
//! let mut rx = adapter.send_message("session-id", "Hello!").await?;
//! while let Some(event) = rx.recv().await {
//!     println!("{:?}", event);
//! }
//! ```

mod event_mapper;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};

use anyhow::Context;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use claude_agent_protocol::adapter::AgentAdapter;
use claude_agent_protocol::events::UnifiedAgentEvent;
use claude_agent_protocol::permission::PermissionDecision;
use claude_agent_protocol::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

use codex_app_server_client::{AppServerClient, AppServerEvent, AppServerRequestHandle};
use codex_app_server_protocol::{
    AppsListParams, AppsListResponse, AskForApproval, CancelLoginAccountParams,
    CancelLoginAccountResponse, ChatgptAuthTokensRefreshResponse, ClientRequest,
    CommandExecResponse,
    CollaborationModeListParams, CollaborationModeListResponse, CommandExecParams,
    CommandExecResizeParams, CommandExecResizeResponse, CommandExecTerminateParams,
    CommandExecTerminateResponse, CommandExecWriteParams, CommandExecWriteResponse,
    ConfigBatchWriteParams, ConfigReadParams, ConfigReadResponse, ConfigRequirementsReadResponse,
    ConfigValueWriteParams, ConfigWriteResponse, DeviceKeyCreateParams, DeviceKeyCreateResponse,
    DeviceKeyPublicParams, DeviceKeyPublicResponse, DeviceKeySignParams, DeviceKeySignResponse,
    DynamicToolCallOutputContentItem, DynamicToolCallResponse,
    ExperimentalFeatureEnablementSetParams, ExperimentalFeatureEnablementSetResponse,
    ExperimentalFeatureListParams, ExperimentalFeatureListResponse,
    ExternalAgentConfigDetectParams, ExternalAgentConfigDetectResponse,
    ExternalAgentConfigImportParams, ExternalAgentConfigImportResponse, FeedbackUploadParams,
    FeedbackUploadResponse, FileChangeApprovalDecision, FsCopyParams, FsCopyResponse,
    FsCreateDirectoryParams, FsCreateDirectoryResponse, FsGetMetadataParams, FsGetMetadataResponse,
    FsReadDirectoryParams, FsReadDirectoryResponse, FsReadFileParams, FsReadFileResponse,
    FsRemoveParams, FsRemoveResponse, FsUnwatchParams, FsUnwatchResponse, FsWatchParams,
    FsWatchResponse, FsWriteFileParams, FsWriteFileResponse, FuzzyFileSearchParams,
    FuzzyFileSearchResponse, FuzzyFileSearchSessionStartParams,
    FuzzyFileSearchSessionStartResponse, FuzzyFileSearchSessionStopParams,
    FuzzyFileSearchSessionStopResponse, FuzzyFileSearchSessionUpdateParams,
    FuzzyFileSearchSessionUpdateResponse, GetAccountParams, GetAccountRateLimitsResponse,
    GetAccountResponse, GrantedPermissionProfile, JSONRPCErrorError, ListMcpServerStatusParams,
    ListMcpServerStatusResponse, LoginAccountParams, LoginAccountResponse, LogoutAccountResponse,
    MarketplaceAddParams, MarketplaceAddResponse, MarketplaceRemoveParams,
    MarketplaceRemoveResponse, MarketplaceUpgradeParams, MarketplaceUpgradeResponse,
    McpResourceReadParams, McpResourceReadResponse, McpServerElicitationAction,
    McpServerElicitationRequestResponse, McpServerOauthLoginParams, McpServerOauthLoginResponse,
    McpServerRefreshResponse, McpServerStatusDetail, McpServerToolCallParams,
    McpServerToolCallResponse, ModelListParams, ModelListResponse, PermissionGrantScope,
    PluginInstallParams, PluginInstallResponse, PluginListParams, PluginListResponse,
    PluginReadParams, PluginReadResponse, PluginUninstallParams, PluginUninstallResponse,
    RequestId, Result as JsonRpcResult, ReviewDelivery, ReviewStartParams, ReviewStartResponse,
    ReviewTarget, SandboxMode, SendAddCreditsNudgeEmailParams, SendAddCreditsNudgeEmailResponse,
    ServerRequest, SkillsConfigWriteParams, SkillsConfigWriteResponse, SkillsListParams,
    SkillsListResponse, SortDirection, ThreadApproveGuardianDeniedActionParams,
    ThreadApproveGuardianDeniedActionResponse, ThreadArchiveParams, ThreadArchiveResponse,
    ThreadBackgroundTerminalsCleanParams, ThreadBackgroundTerminalsCleanResponse,
    ThreadCompactStartParams, ThreadCompactStartResponse, ThreadDecrementElicitationParams,
    ThreadDecrementElicitationResponse, ThreadForkParams, ThreadForkResponse,
    ThreadGoalClearParams, ThreadGoalClearResponse, ThreadGoalGetParams, ThreadGoalGetResponse,
    ThreadGoalSetParams, ThreadGoalSetResponse, ThreadIncrementElicitationParams,
    ThreadIncrementElicitationResponse, ThreadInjectItemsParams, ThreadInjectItemsResponse,
    ThreadListCwdFilter, ThreadListParams, ThreadListResponse, ThreadLoadedListParams,
    ThreadLoadedListResponse, ThreadMemoryMode, ThreadMemoryModeSetParams,
    ThreadMemoryModeSetResponse, ThreadMetadataUpdateParams, ThreadMetadataUpdateResponse,
    ThreadReadParams, ThreadReadResponse, ThreadRealtimeAppendAudioParams,
    ThreadRealtimeAppendAudioResponse, ThreadRealtimeAppendTextParams,
    ThreadRealtimeAppendTextResponse, ThreadRealtimeListVoicesParams,
    ThreadRealtimeListVoicesResponse, ThreadRealtimeStartParams, ThreadRealtimeStartResponse,
    ThreadRealtimeStopParams, ThreadRealtimeStopResponse, ThreadResumeParams, ThreadResumeResponse,
    ThreadRollbackParams, ThreadRollbackResponse, ThreadSetNameParams, ThreadSetNameResponse,
    ThreadShellCommandParams, ThreadShellCommandResponse, ThreadSortKey, ThreadSourceKind,
    ThreadStartParams, ThreadStartResponse, ThreadTurnsListParams, ThreadTurnsListResponse,
    ThreadUnarchiveParams, ThreadUnarchiveResponse, ThreadUnsubscribeParams,
    ThreadUnsubscribeResponse, TurnInterruptParams, TurnInterruptResponse, TurnStartParams,
    TurnStartResponse, TurnSteerParams, TurnSteerResponse, UserInput as ProtocolUserInput,
    WindowsSandboxSetupStartParams, WindowsSandboxSetupStartResponse,
};
use codex_exec_server::{EnvironmentManager, EnvironmentManagerArgs, ExecServerRuntimePaths};
use codex_protocol::models::PermissionProfile as CorePermissionProfile;
use codex_protocol::protocol::ReviewDecision;
use toml::Value as TomlValue;
use tracing_subscriber::prelude::*;

const REMOTE_CODE_PROJECT_QUALIFIER: &str = "com";
const REMOTE_CODE_PROJECT_ORGANIZATION: &str = "RemoteCode";
const REMOTE_CODE_PROJECT_APPLICATION: &str = "remote-code";

/// Runtime options used when starting the in-process Codex app-server.
///
/// The adapter treats these as in-memory overrides and delegates final
/// validation to Codex's native `ConfigBuilder` and app-server request handlers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CodexAdapterOptions {
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
    pub permission_profile: Option<serde_json::Value>,
    pub service_tier: Option<serde_json::Value>,
    pub persist_extended_history: bool,
    pub ephemeral: Option<bool>,
    pub memories_enabled: Option<bool>,
    pub thread_store_endpoint: Option<String>,
    pub config_overrides: HashMap<String, String>,
    #[serde(skip)]
    pub cli_overrides: Vec<(String, TomlValue)>,
    pub mcp_servers: HashMap<String, serde_json::Value>,
    pub enable_codex_api_key_env: bool,
    pub client_name: Option<String>,
    pub client_version: Option<String>,
    pub exec_server_url: Option<String>,
    pub channel_capacity: Option<usize>,
    #[serde(default = "default_true")]
    pub feedback_capture_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexExecRequest {
    pub command: Vec<String>,
    pub process_id: Option<String>,
    #[serde(default)]
    pub tty: bool,
    #[serde(default)]
    pub stream_stdin: bool,
    #[serde(default)]
    pub stream_stdout_stderr: bool,
    pub output_bytes_cap: Option<usize>,
    #[serde(default)]
    pub disable_output_cap: bool,
    #[serde(default)]
    pub disable_timeout: bool,
    pub timeout_ms: Option<i64>,
    pub cwd: Option<PathBuf>,
    pub env: Option<HashMap<String, Option<String>>>,
    pub sandbox_policy: Option<serde_json::Value>,
    pub permission_profile: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadGoalRefRequest {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadGoalSetRequest {
    pub thread_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadRollbackRequest {
    pub thread_id: String,
    pub num_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTurnSteerRequest {
    pub thread_id: String,
    pub expected_turn_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTurnInterruptRequest {
    pub thread_id: String,
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginRefRequest {
    pub marketplace_path: Option<PathBuf>,
    pub remote_marketplace_name: Option<String>,
    pub plugin_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexServerRequestResolution {
    #[serde(default)]
    pub allow_all: bool,
    #[serde(default)]
    pub response: Option<serde_json::Value>,
}

impl Default for CodexServerRequestResolution {
    fn default() -> Self {
        Self {
            allow_all: false,
            response: None,
        }
    }
}

/// Resolve the Codex home used by Remote Code's embedded Codex runtime.
///
/// This intentionally does not use Codex's default `~/.codex` directory. It is
/// shared by the GUI process entry point and the adapter so helper binaries,
/// `.env`, thread stores, config, memories, and runtime state all live under the
/// same app-scoped directory.
pub fn isolated_codex_home() -> anyhow::Result<PathBuf> {
    let project_dirs = directories::ProjectDirs::from(
        REMOTE_CODE_PROJECT_QUALIFIER,
        REMOTE_CODE_PROJECT_ORGANIZATION,
        REMOTE_CODE_PROJECT_APPLICATION,
    )
    .ok_or_else(|| anyhow::anyhow!("Cannot determine OS data directory"))?;
    let codex_home = project_dirs.data_dir().join("codex");
    std::fs::create_dir_all(&codex_home)
        .with_context(|| format!("Failed to create isolated codex home at {:?}", codex_home))?;
    Ok(codex_home)
}

impl TryFrom<CodexExecRequest> for CommandExecParams {
    type Error = anyhow::Error;

    fn try_from(value: CodexExecRequest) -> Result<Self, Self::Error> {
        let sandbox_policy = value
            .sandbox_policy
            .map(serde_json::from_value)
            .transpose()
            .context("invalid Codex command/exec sandbox policy")?;
        let permission_profile = value
            .permission_profile
            .map(serde_json::from_value)
            .transpose()
            .context("invalid Codex command/exec permission profile")?;

        if sandbox_policy.is_some() && permission_profile.is_some() {
            return Err(anyhow::anyhow!(
                "Codex command/exec cannot combine sandboxPolicy and permissionProfile"
            ));
        }

        Ok(Self {
            command: value.command,
            process_id: value.process_id,
            tty: value.tty,
            stream_stdin: value.stream_stdin,
            stream_stdout_stderr: value.stream_stdout_stderr,
            output_bytes_cap: value.output_bytes_cap,
            disable_output_cap: value.disable_output_cap,
            disable_timeout: value.disable_timeout,
            timeout_ms: value.timeout_ms,
            cwd: value.cwd,
            env: value.env,
            size: None,
            sandbox_policy,
            permission_profile,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadListRequest {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub model_providers: Option<Vec<String>>,
    pub source_kinds: Option<Vec<String>>,
    pub archived: Option<bool>,
    pub cwd: Option<serde_json::Value>,
    #[serde(default)]
    pub use_state_db_only: bool,
    pub search_term: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexFeedbackRequest {
    pub classification: String,
    pub reason: Option<String>,
    pub thread_id: Option<String>,
    #[serde(default)]
    pub include_logs: bool,
    pub extra_log_files: Option<Vec<PathBuf>>,
    pub tags: Option<std::collections::BTreeMap<String, String>>,
}

impl From<CodexFeedbackRequest> for FeedbackUploadParams {
    fn from(value: CodexFeedbackRequest) -> Self {
        Self {
            classification: value.classification,
            reason: value.reason,
            thread_id: value.thread_id,
            include_logs: value.include_logs,
            extra_log_files: value.extra_log_files,
            tags: value.tags,
        }
    }
}

fn default_true() -> bool {
    true
}

fn shared_feedback(
    capture_enabled: bool,
    log_db: Option<codex_state::log_db::LogDbLayer>,
) -> codex_feedback::CodexFeedback {
    if !capture_enabled {
        return codex_feedback::CodexFeedback::new();
    }

    static FEEDBACK: OnceLock<codex_feedback::CodexFeedback> = OnceLock::new();
    let feedback = FEEDBACK
        .get_or_init(|| {
            let feedback = codex_feedback::CodexFeedback::new();
            let log_db_layer = log_db.clone().map(|layer| {
                layer.with_filter(
                    tracing_subscriber::filter::Targets::new().with_default(tracing::Level::TRACE),
                )
            });
            let _ = tracing_subscriber::registry()
                .with(feedback.logger_layer())
                .with(feedback.metadata_layer())
                .with(log_db_layer)
                .try_init();
            feedback
        })
        .clone();
    feedback
}

#[derive(Debug, Clone)]
enum PendingServerRequestKind {
    CommandExecution,
    FileChange,
    ApplyPatch,
    ExecCommand,
    Permissions(serde_json::Value),
    McpElicitation,
    ToolUserInput(serde_json::Value),
    #[allow(dead_code)]
    DynamicTool {
        call_id: String,
        namespace: Option<String>,
        tool: String,
        arguments: serde_json::Value,
    },
    #[allow(dead_code)]
    ChatgptAuthRefresh {
        reason: String,
        previous_account_id: Option<String>,
    },
}

impl PendingServerRequestKind {
    fn from_request(request: &ServerRequest) -> Self {
        match request {
            ServerRequest::CommandExecutionRequestApproval { .. } => Self::CommandExecution,
            ServerRequest::FileChangeRequestApproval { .. } => Self::FileChange,
            ServerRequest::ApplyPatchApproval { .. } => Self::ApplyPatch,
            ServerRequest::ExecCommandApproval { .. } => Self::ExecCommand,
            ServerRequest::PermissionsRequestApproval { params, .. } => Self::Permissions(
                serde_json::to_value(&params.permissions).unwrap_or(serde_json::Value::Null),
            ),
            ServerRequest::McpServerElicitationRequest { .. } => Self::McpElicitation,
            ServerRequest::ToolRequestUserInput { params, .. } => Self::ToolUserInput(
                serde_json::to_value(&params.questions).unwrap_or(serde_json::Value::Null),
            ),
            ServerRequest::DynamicToolCall { params, .. } => Self::DynamicTool {
                call_id: params.call_id.clone(),
                namespace: params.namespace.clone(),
                tool: params.tool.clone(),
                arguments: params.arguments.clone(),
            },
            ServerRequest::ChatgptAuthTokensRefresh { params, .. } => Self::ChatgptAuthRefresh {
                reason: format!("{:?}", params.reason),
                previous_account_id: params.previous_account_id.clone(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Shared event routing state
// ---------------------------------------------------------------------------

/// Shared state between the adapter and the background event pump.
///
/// The pump writes events to whichever sender is currently installed.
/// `send_message()` swaps in a new sender for each turn.
struct EventPumpState {
    /// The current event sender, swapped by `send_message()`.
    current_tx: Option<mpsc::Sender<UnifiedAgentEvent>>,
    /// Server request ids and their exact official response shape.
    pending_server_requests: HashMap<String, PendingServerRequestKind>,
}

impl EventPumpState {
    fn new() -> Self {
        Self {
            current_tx: None,
            pending_server_requests: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

/// In-process Codex adapter that wraps [`AppServerClient`].
///
/// On [`start()`](AgentAdapter::start), the adapter extracts a cloneable
/// [`AppServerRequestHandle`] from the client for sending commands, then spawns
/// a background tokio task that continuously drains events from the client,
/// maps them through [`event_mapper`], and forwards them to the caller via
/// a shared `mpsc::Sender`.
pub struct CodexInProcessAdapter {
    /// Cloneable handle for sending commands (requests, resolve, reject).
    request_handle: Option<AppServerRequestHandle>,
    /// Shared event routing state between adapter and background pump.
    event_state: Arc<Mutex<EventPumpState>>,
    /// Handle to the background event pump task.
    worker_handle: Option<tokio::task::JoinHandle<()>>,
    /// Static agent metadata.
    info: AgentInfo,
    /// Runtime status.
    status: AgentStatus,
    /// Current session ID (set during `start`).
    session_id: Option<String>,
    /// Current thread ID within the Codex runtime.
    thread_id: Option<String>,
    /// Monotonic request ID counter.
    request_counter: AtomicI64,
    /// Working directory for Codex operations.
    cwd: PathBuf,
    /// Model override.
    model: Option<String>,
    model_provider: Option<String>,
    approval_policy: Option<AskForApproval>,
    sandbox: Option<SandboxMode>,
    permission_profile: Option<codex_app_server_protocol::PermissionProfile>,
    service_tier: Option<Option<codex_protocol::config_types::ServiceTier>>,
    config_overrides: Option<HashMap<String, serde_json::Value>>,
    ephemeral: Option<bool>,
    persist_extended_history: bool,
    /// Placeholder to hold the client until `start()` consumes it for the event pump.
    _client_placeholder: Option<AppServerClient>,
}

impl CodexInProcessAdapter {
    /// Create a new adapter wrapping an already-started [`AppServerClient`].
    ///
    /// The caller is responsible for starting the Codex runtime
    /// (`InProcessAppServerClient::start` or `RemoteAppServerClient::connect`)
    /// before passing it here. The client will be consumed during [`start()`](AgentAdapter::start)
    /// when the background event pump is spawned.
    pub fn new(client: AppServerClient) -> Self {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::Streaming);
        caps.insert(AgentCapability::ToolUse);
        caps.insert(AgentCapability::Subtasks);
        caps.insert(AgentCapability::Permissions);

        // Extract the request handle immediately — it's cloneable and doesn't
        // need the full client.
        let request_handle = Some(client.request_handle());

        Self {
            request_handle,
            event_state: Arc::new(Mutex::new(EventPumpState::new())),
            worker_handle: None,
            info: AgentInfo {
                name: "Codex In-Process".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities: caps,
                status: AgentStatus::Starting,
            },
            status: AgentStatus::Starting,
            session_id: None,
            thread_id: None,
            request_counter: AtomicI64::new(1),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model: None,
            model_provider: None,
            approval_policy: None,
            sandbox: None,
            permission_profile: None,
            service_tier: None,
            config_overrides: None,
            ephemeral: None,
            persist_extended_history: true,
            // Hold the client until start() consumes it for the event pump.
            _client_placeholder: Some(client),
        }
    }

    /// Create a new adapter in the **Starting** state without a client.
    ///
    /// The client must be set later via [`Self::set_client`] before calling
    /// [`AgentAdapter::start`].
    pub fn empty() -> Self {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::Streaming);
        caps.insert(AgentCapability::ToolUse);
        caps.insert(AgentCapability::Subtasks);
        caps.insert(AgentCapability::Permissions);

        Self {
            request_handle: None,
            event_state: Arc::new(Mutex::new(EventPumpState::new())),
            worker_handle: None,
            info: AgentInfo {
                name: "Codex In-Process".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities: caps,
                status: AgentStatus::Starting,
            },
            status: AgentStatus::Starting,
            session_id: None,
            thread_id: None,
            request_counter: AtomicI64::new(1),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model: None,
            model_provider: None,
            approval_policy: None,
            sandbox: None,
            permission_profile: None,
            service_tier: None,
            config_overrides: None,
            ephemeral: None,
            persist_extended_history: true,
            _client_placeholder: None,
        }
    }

    /// Set the underlying [`AppServerClient`].
    ///
    /// Must be called before [`AgentAdapter::start`].
    pub fn set_client(&mut self, client: AppServerClient) {
        self.request_handle = Some(client.request_handle());
        self._client_placeholder = Some(client);
    }

    /// Set the working directory for Codex operations.
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    /// Set the model override.
    pub fn set_model(&mut self, model: String) {
        self.model = Some(model);
    }

    /// Create and start a fully-initialized adapter with an isolated Codex runtime.
    ///
    /// This is the recommended way to create a `CodexInProcessAdapter`. It:
    /// 1. Creates an isolated data directory (under the OS data dir, not `~/.codex`)
    /// 2. Builds a minimal Codex `Config` with that isolated home
    /// 3. Starts the in-process Codex runtime (`InProcessAppServerClient`)
    /// 4. Wraps it in the adapter
    ///
    /// Storage is fully isolated from any standalone Codex installation — no shared
    /// databases, config files, or session data.
    pub async fn start_in_process(cwd: PathBuf, model: Option<String>) -> anyhow::Result<Self> {
        Self::start_in_process_with_options(CodexAdapterOptions {
            cwd,
            model,
            persist_extended_history: true,
            ..Default::default()
        })
        .await
    }

    /// Create and start a fully-initialized adapter with native Codex options.
    pub async fn start_in_process_with_options(
        options: CodexAdapterOptions,
    ) -> anyhow::Result<Self> {
        let cwd = if options.cwd.as_os_str().is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            options.cwd.clone()
        };

        // 1. Compute isolated codex_home under the OS data directory.
        let codex_home = isolated_codex_home()?;
        info!(codex_home = %codex_home.display(), "Using isolated Codex home");

        let mut cli_overrides = options.cli_overrides.clone();
        cli_overrides.extend(build_cli_overrides(&options)?);
        let harness_overrides = build_harness_overrides(&options, &cwd)?;
        let loader_overrides = codex_core::config_loader::LoaderOverrides::default();
        let cloud_requirements = codex_core::config_loader::CloudRequirementsLoader::default();

        // 2. Build Codex Config with the isolated home and GUI/native overrides.
        let config = codex_core::config::ConfigBuilder::default()
            .codex_home(codex_home)
            .fallback_cwd(Some(cwd.clone()))
            .cli_overrides(cli_overrides.clone())
            .harness_overrides(harness_overrides)
            .loader_overrides(loader_overrides.clone())
            .cloud_requirements(cloud_requirements.clone())
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Codex ConfigBuilder failed: {e}"))?;

        let arg0_paths = codex_arg0::Arg0DispatchPaths {
            codex_self_exe: std::env::current_exe().ok(),
            codex_linux_sandbox_exe: None,
            main_execve_wrapper_exe: None,
        };
        let runtime_paths = ExecServerRuntimePaths::from_optional_paths(
            arg0_paths.codex_self_exe.clone(),
            arg0_paths.codex_linux_sandbox_exe.clone(),
        )
        .map_err(|e| anyhow::anyhow!("Codex runtime paths unavailable: {e}"))?;
        let environment_manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: options.exec_server_url.clone(),
            local_runtime_paths: runtime_paths,
        });

        let state_db = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .ok();
        let log_db = state_db.clone().map(codex_state::log_db::start);
        let feedback = shared_feedback(options.feedback_capture_enabled, log_db.clone());

        // 3. Build the in-process client start args.
        let args = codex_app_server_client::InProcessClientStartArgs {
            arg0_paths,
            config: Arc::new(config),
            cli_overrides,
            loader_overrides,
            cloud_requirements,
            feedback,
            log_db,
            environment_manager: Arc::new(environment_manager),
            config_warnings: Vec::new(),
            session_source: codex_app_server_protocol::SessionSource::AppServer.into(),
            enable_codex_api_key_env: options.enable_codex_api_key_env,
            client_name: options
                .client_name
                .clone()
                .unwrap_or_else(|| "remote-code-gui".to_string()),
            client_version: options
                .client_version
                .clone()
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
            experimental_api: true,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: options.channel_capacity.unwrap_or(1024),
        };

        // 4. Start the in-process Codex runtime.
        let client = codex_app_server_client::InProcessAppServerClient::start(args)
            .await
            .map_err(|e| anyhow::anyhow!("InProcessAppServerClient::start failed: {e}"))?;

        // 5. Wrap in adapter.
        let mut adapter = Self::new(codex_app_server_client::AppServerClient::InProcess(client));
        adapter.apply_options(options, cwd)?;

        Ok(adapter)
    }

    fn apply_options(&mut self, options: CodexAdapterOptions, cwd: PathBuf) -> anyhow::Result<()> {
        self.cwd = cwd;
        self.model = options.model.clone();
        self.model_provider = options.model_provider.clone();
        self.approval_policy = options
            .approval_policy
            .as_deref()
            .map(parse_approval_policy)
            .transpose()?;
        self.sandbox = options
            .sandbox_mode
            .as_deref()
            .map(parse_sandbox_mode)
            .transpose()?;
        self.permission_profile = options
            .permission_profile
            .clone()
            .map(serde_json::from_value)
            .transpose()
            .context("invalid Codex permission profile")?;
        self.service_tier = options
            .service_tier
            .clone()
            .map(serde_json::from_value)
            .transpose()
            .context("invalid Codex service tier")?;
        self.config_overrides = Some(build_thread_config_overrides_json(&options));
        self.ephemeral = options.ephemeral;
        self.persist_extended_history = options.persist_extended_history;
        Ok(())
    }

    /// Generate the next unique request ID.
    fn next_request_id(&self) -> RequestId {
        let n = self.request_counter.fetch_add(1, Ordering::Relaxed);
        RequestId::Integer(n)
    }

    fn handle(&self) -> anyhow::Result<&AppServerRequestHandle> {
        self.request_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Codex request handle not initialized"))
    }

    async fn request_typed<T>(&self, request: ClientRequest) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.handle()?
            .request_typed(request)
            .await
            .map_err(|e| anyhow::anyhow!("Codex request failed: {e}"))
    }

    /// Ensure a thread exists (create one if needed) and return its ID.
    async fn ensure_thread(&self) -> anyhow::Result<String> {
        if let Some(ref tid) = self.thread_id {
            return Ok(tid.clone());
        }

        let response = self.start_thread().await?;
        let thread_id = response.thread.id.clone();
        info!(thread_id = %thread_id, "Codex thread started");
        Ok(thread_id)
    }

    pub async fn start_thread(&self) -> anyhow::Result<ThreadStartResponse> {
        self.request_typed(ClientRequest::ThreadStart {
            request_id: self.next_request_id(),
            params: self.thread_start_params(),
        })
        .await
    }

    pub async fn start_new_thread(&mut self) -> anyhow::Result<ThreadStartResponse> {
        let response = self.start_thread().await?;
        self.thread_id = Some(response.thread.id.clone());
        Ok(response)
    }

    pub async fn start_thread_with_params(
        &mut self,
        params: Option<ThreadStartParams>,
    ) -> anyhow::Result<ThreadStartResponse> {
        let response: ThreadStartResponse = self
            .request_typed(ClientRequest::ThreadStart {
                request_id: self.next_request_id(),
                params: params.unwrap_or_else(|| self.thread_start_params()),
            })
            .await?;
        self.thread_id = Some(response.thread.id.clone());
        Ok(response)
    }

    fn thread_start_params(&self) -> ThreadStartParams {
        ThreadStartParams {
            cwd: Some(self.cwd.to_string_lossy().into_owned()),
            model: self.model.clone(),
            model_provider: self.model_provider.clone(),
            service_tier: self.service_tier.clone(),
            approval_policy: self.approval_policy,
            sandbox: self.sandbox,
            permission_profile: self.permission_profile.clone(),
            config: self.config_overrides.clone(),
            ephemeral: self.ephemeral,
            persist_extended_history: self.persist_extended_history,
            ..Default::default()
        }
    }

    fn thread_resume_params(&self, thread_id: String, include_turns: bool) -> ThreadResumeParams {
        ThreadResumeParams {
            thread_id,
            model: self.model.clone(),
            model_provider: self.model_provider.clone(),
            service_tier: self.service_tier.clone(),
            cwd: Some(self.cwd.to_string_lossy().into_owned()),
            approval_policy: self.approval_policy,
            sandbox: self.sandbox,
            permission_profile: self.permission_profile.clone(),
            config: self.config_overrides.clone(),
            exclude_turns: !include_turns,
            persist_extended_history: self.persist_extended_history,
            ..Default::default()
        }
    }

    fn thread_fork_params(&self, thread_id: String, include_turns: bool) -> ThreadForkParams {
        ThreadForkParams {
            thread_id,
            model: self.model.clone(),
            model_provider: self.model_provider.clone(),
            service_tier: self.service_tier.clone(),
            cwd: Some(self.cwd.to_string_lossy().into_owned()),
            approval_policy: self.approval_policy,
            sandbox: self.sandbox,
            permission_profile: self.permission_profile.clone(),
            config: self.config_overrides.clone(),
            ephemeral: self.ephemeral.unwrap_or(false),
            exclude_turns: !include_turns,
            persist_extended_history: self.persist_extended_history,
            ..Default::default()
        }
    }

    /// Background task that continuously drains events from the Codex client,
    /// maps them through the event mapper, and forwards them to the current
    /// event sender.
    async fn event_pump(
        mut client: AppServerClient,
        event_state: Arc<Mutex<EventPumpState>>,
        session_id: String,
    ) {
        info!("Codex event pump started");
        loop {
            match client.next_event().await {
                Some(event) => {
                    if let AppServerEvent::ServerRequest(request) = &event {
                        let id = request_id_to_string(request.id());
                        let kind = PendingServerRequestKind::from_request(request);
                        let mut state = event_state.lock().await;
                        state.pending_server_requests.insert(id, kind);
                    }

                    let mapped = event_mapper::map_app_server_event(event, &session_id);
                    let tx = {
                        let state = event_state.lock().await;
                        state.current_tx.clone()
                    };
                    if let Some(tx) = tx {
                        for evt in mapped {
                            if tx.send(evt).await.is_err() {
                                // Receiver dropped — clear the sender.
                                let mut state = event_state.lock().await;
                                state.current_tx = None;
                                break;
                            }
                        }
                    }
                }
                None => {
                    info!("Codex event pump: client disconnected");
                    break;
                }
            }
        }
        info!("Codex event pump stopped");
    }

    pub fn thread_id(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }

    pub async fn list_threads(
        &self,
        params: Option<CodexThreadListRequest>,
    ) -> anyhow::Result<ThreadListResponse> {
        self.request_typed(ClientRequest::ThreadList {
            request_id: self.next_request_id(),
            params: params
                .map(thread_list_params_from_request)
                .unwrap_or_else(|| {
                    thread_list_params_from_request(CodexThreadListRequest::default())
                }),
        })
        .await
    }

    pub async fn read_thread(
        &self,
        thread_id: String,
        include_turns: bool,
    ) -> anyhow::Result<ThreadReadResponse> {
        self.request_typed(ClientRequest::ThreadRead {
            request_id: self.next_request_id(),
            params: ThreadReadParams {
                thread_id,
                include_turns,
            },
        })
        .await
    }

    pub async fn resume_thread(
        &mut self,
        thread_id: String,
        include_turns: bool,
    ) -> anyhow::Result<ThreadResumeResponse> {
        let response: ThreadResumeResponse = self
            .request_typed(ClientRequest::ThreadResume {
                request_id: self.next_request_id(),
                params: self.thread_resume_params(thread_id, include_turns),
            })
            .await?;
        self.thread_id = Some(response.thread.id.clone());
        Ok(response)
    }

    pub async fn resume_thread_with_params(
        &mut self,
        params: ThreadResumeParams,
    ) -> anyhow::Result<ThreadResumeResponse> {
        let response: ThreadResumeResponse = self
            .request_typed(ClientRequest::ThreadResume {
                request_id: self.next_request_id(),
                params,
            })
            .await?;
        self.thread_id = Some(response.thread.id.clone());
        Ok(response)
    }

    pub async fn fork_thread(
        &mut self,
        thread_id: String,
        include_turns: bool,
    ) -> anyhow::Result<ThreadForkResponse> {
        let response: ThreadForkResponse = self
            .request_typed(ClientRequest::ThreadFork {
                request_id: self.next_request_id(),
                params: self.thread_fork_params(thread_id, include_turns),
            })
            .await?;
        self.thread_id = Some(response.thread.id.clone());
        Ok(response)
    }

    pub async fn fork_thread_with_params(
        &mut self,
        params: ThreadForkParams,
    ) -> anyhow::Result<ThreadForkResponse> {
        let response: ThreadForkResponse = self
            .request_typed(ClientRequest::ThreadFork {
                request_id: self.next_request_id(),
                params,
            })
            .await?;
        self.thread_id = Some(response.thread.id.clone());
        Ok(response)
    }

    pub async fn archive_thread(&self, thread_id: String) -> anyhow::Result<ThreadArchiveResponse> {
        self.request_typed(ClientRequest::ThreadArchive {
            request_id: self.next_request_id(),
            params: ThreadArchiveParams { thread_id },
        })
        .await
    }

    pub async fn unarchive_thread(
        &self,
        thread_id: String,
    ) -> anyhow::Result<ThreadUnarchiveResponse> {
        self.request_typed(ClientRequest::ThreadUnarchive {
            request_id: self.next_request_id(),
            params: ThreadUnarchiveParams { thread_id },
        })
        .await
    }

    pub async fn unsubscribe_thread(
        &self,
        thread_id: String,
    ) -> anyhow::Result<ThreadUnsubscribeResponse> {
        self.request_typed(ClientRequest::ThreadUnsubscribe {
            request_id: self.next_request_id(),
            params: ThreadUnsubscribeParams { thread_id },
        })
        .await
    }

    pub async fn increment_thread_elicitation(
        &self,
        thread_id: String,
    ) -> anyhow::Result<ThreadIncrementElicitationResponse> {
        self.request_typed(ClientRequest::ThreadIncrementElicitation {
            request_id: self.next_request_id(),
            params: ThreadIncrementElicitationParams { thread_id },
        })
        .await
    }

    pub async fn decrement_thread_elicitation(
        &self,
        thread_id: String,
    ) -> anyhow::Result<ThreadDecrementElicitationResponse> {
        self.request_typed(ClientRequest::ThreadDecrementElicitation {
            request_id: self.next_request_id(),
            params: ThreadDecrementElicitationParams { thread_id },
        })
        .await
    }

    pub async fn update_thread_metadata(
        &self,
        params: ThreadMetadataUpdateParams,
    ) -> anyhow::Result<ThreadMetadataUpdateResponse> {
        self.request_typed(ClientRequest::ThreadMetadataUpdate {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn set_thread_name(
        &self,
        thread_id: String,
        name: String,
    ) -> anyhow::Result<ThreadSetNameResponse> {
        self.request_typed(ClientRequest::ThreadSetName {
            request_id: self.next_request_id(),
            params: ThreadSetNameParams { thread_id, name },
        })
        .await
    }

    pub async fn set_thread_goal(
        &self,
        request: CodexThreadGoalSetRequest,
    ) -> anyhow::Result<ThreadGoalSetResponse> {
        self.request_typed(ClientRequest::ThreadGoalSet {
            request_id: self.next_request_id(),
            params: ThreadGoalSetParams {
                thread_id: request.thread_id,
                objective: Some(request.text),
                status: None,
                token_budget: None,
            },
        })
        .await
    }

    pub async fn get_thread_goal(
        &self,
        thread_id: String,
    ) -> anyhow::Result<ThreadGoalGetResponse> {
        self.request_typed(ClientRequest::ThreadGoalGet {
            request_id: self.next_request_id(),
            params: ThreadGoalGetParams { thread_id },
        })
        .await
    }

    pub async fn clear_thread_goal(
        &self,
        thread_id: String,
    ) -> anyhow::Result<ThreadGoalClearResponse> {
        self.request_typed(ClientRequest::ThreadGoalClear {
            request_id: self.next_request_id(),
            params: ThreadGoalClearParams { thread_id },
        })
        .await
    }

    pub async fn compact_thread(
        &self,
        thread_id: String,
    ) -> anyhow::Result<ThreadCompactStartResponse> {
        self.request_typed(ClientRequest::ThreadCompactStart {
            request_id: self.next_request_id(),
            params: ThreadCompactStartParams { thread_id },
        })
        .await
    }

    pub async fn run_thread_shell_command(
        &self,
        thread_id: String,
        command: String,
    ) -> anyhow::Result<ThreadShellCommandResponse> {
        self.request_typed(ClientRequest::ThreadShellCommand {
            request_id: self.next_request_id(),
            params: ThreadShellCommandParams { thread_id, command },
        })
        .await
    }

    pub async fn clean_thread_background_terminals(
        &self,
        thread_id: String,
    ) -> anyhow::Result<ThreadBackgroundTerminalsCleanResponse> {
        self.request_typed(ClientRequest::ThreadBackgroundTerminalsClean {
            request_id: self.next_request_id(),
            params: ThreadBackgroundTerminalsCleanParams { thread_id },
        })
        .await
    }

    pub async fn approve_guardian_denied_action(
        &self,
        params: ThreadApproveGuardianDeniedActionParams,
    ) -> anyhow::Result<ThreadApproveGuardianDeniedActionResponse> {
        self.request_typed(ClientRequest::ThreadApproveGuardianDeniedAction {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn rollback_thread(
        &self,
        request: CodexThreadRollbackRequest,
    ) -> anyhow::Result<ThreadRollbackResponse> {
        self.request_typed(ClientRequest::ThreadRollback {
            request_id: self.next_request_id(),
            params: ThreadRollbackParams {
                thread_id: request.thread_id,
                num_turns: request.num_turns.max(1),
            },
        })
        .await
    }

    pub async fn list_thread_turns(
        &self,
        thread_id: String,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> anyhow::Result<ThreadTurnsListResponse> {
        self.request_typed(ClientRequest::ThreadTurnsList {
            request_id: self.next_request_id(),
            params: ThreadTurnsListParams {
                thread_id,
                cursor,
                limit,
                sort_direction: Some(SortDirection::Desc),
            },
        })
        .await
    }

    pub async fn list_loaded_threads(
        &self,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> anyhow::Result<ThreadLoadedListResponse> {
        self.request_typed(ClientRequest::ThreadLoadedList {
            request_id: self.next_request_id(),
            params: ThreadLoadedListParams { cursor, limit },
        })
        .await
    }

    pub async fn inject_thread_items(
        &self,
        params: ThreadInjectItemsParams,
    ) -> anyhow::Result<ThreadInjectItemsResponse> {
        self.request_typed(ClientRequest::ThreadInjectItems {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn start_turn(&self, params: TurnStartParams) -> anyhow::Result<TurnStartResponse> {
        self.request_typed(ClientRequest::TurnStart {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn steer_turn(
        &self,
        request: CodexTurnSteerRequest,
    ) -> anyhow::Result<TurnSteerResponse> {
        self.request_typed(ClientRequest::TurnSteer {
            request_id: self.next_request_id(),
            params: TurnSteerParams {
                thread_id: request.thread_id,
                input: vec![ProtocolUserInput::Text {
                    text: request.message,
                    text_elements: Vec::new(),
                }],
                responsesapi_client_metadata: None,
                expected_turn_id: request.expected_turn_id,
            },
        })
        .await
    }

    pub async fn interrupt_turn(
        &self,
        request: CodexTurnInterruptRequest,
    ) -> anyhow::Result<TurnInterruptResponse> {
        self.request_typed(ClientRequest::TurnInterrupt {
            request_id: self.next_request_id(),
            params: TurnInterruptParams {
                thread_id: request.thread_id,
                turn_id: request.turn_id.unwrap_or_default(),
            },
        })
        .await
    }

    pub async fn list_models(
        &self,
        include_hidden: Option<bool>,
    ) -> anyhow::Result<ModelListResponse> {
        self.request_typed(ClientRequest::ModelList {
            request_id: self.next_request_id(),
            params: ModelListParams {
                cursor: None,
                limit: None,
                include_hidden,
            },
        })
        .await
    }

    pub async fn list_collaboration_modes(&self) -> anyhow::Result<CollaborationModeListResponse> {
        self.request_typed(ClientRequest::CollaborationModeList {
            request_id: self.next_request_id(),
            params: CollaborationModeListParams::default(),
        })
        .await
    }

    pub async fn list_experimental_features(
        &self,
    ) -> anyhow::Result<ExperimentalFeatureListResponse> {
        self.request_typed(ClientRequest::ExperimentalFeatureList {
            request_id: self.next_request_id(),
            params: ExperimentalFeatureListParams {
                cursor: None,
                limit: None,
            },
        })
        .await
    }

    pub async fn set_experimental_feature(
        &self,
        feature: String,
        enabled: bool,
    ) -> anyhow::Result<ExperimentalFeatureEnablementSetResponse> {
        self.request_typed(ClientRequest::ExperimentalFeatureEnablementSet {
            request_id: self.next_request_id(),
            params: ExperimentalFeatureEnablementSetParams {
                enablement: std::iter::once((feature, enabled)).collect(),
            },
        })
        .await
    }

    pub async fn read_account(&self) -> anyhow::Result<GetAccountResponse> {
        self.request_typed(ClientRequest::GetAccount {
            request_id: self.next_request_id(),
            params: GetAccountParams {
                refresh_token: false,
            },
        })
        .await
    }

    pub async fn login_account(
        &self,
        params: LoginAccountParams,
    ) -> anyhow::Result<LoginAccountResponse> {
        self.request_typed(ClientRequest::LoginAccount {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn cancel_login_account(
        &self,
        params: CancelLoginAccountParams,
    ) -> anyhow::Result<CancelLoginAccountResponse> {
        self.request_typed(ClientRequest::CancelLoginAccount {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn logout_account(&self) -> anyhow::Result<LogoutAccountResponse> {
        self.request_typed(ClientRequest::LogoutAccount {
            request_id: self.next_request_id(),
            params: None,
        })
        .await
    }

    pub async fn read_account_rate_limits(&self) -> anyhow::Result<GetAccountRateLimitsResponse> {
        self.request_typed(ClientRequest::GetAccountRateLimits {
            request_id: self.next_request_id(),
            params: None,
        })
        .await
    }

    pub async fn send_add_credits_nudge_email(
        &self,
        params: SendAddCreditsNudgeEmailParams,
    ) -> anyhow::Result<SendAddCreditsNudgeEmailResponse> {
        self.request_typed(ClientRequest::SendAddCreditsNudgeEmail {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn list_apps(&self) -> anyhow::Result<AppsListResponse> {
        self.request_typed(ClientRequest::AppsList {
            request_id: self.next_request_id(),
            params: AppsListParams {
                cursor: None,
                limit: None,
                thread_id: self.thread_id.clone(),
                force_refetch: false,
            },
        })
        .await
    }

    pub async fn list_skills(
        &self,
        cwds: Vec<PathBuf>,
        force_reload: bool,
    ) -> anyhow::Result<SkillsListResponse> {
        self.request_typed(ClientRequest::SkillsList {
            request_id: self.next_request_id(),
            params: SkillsListParams {
                cwds,
                force_reload,
                per_cwd_extra_user_roots: None,
            },
        })
        .await
    }

    pub async fn write_skills_config(
        &self,
        path: Option<PathBuf>,
        name: Option<String>,
        enabled: bool,
    ) -> anyhow::Result<SkillsConfigWriteResponse> {
        self.request_typed(ClientRequest::SkillsConfigWrite {
            request_id: self.next_request_id(),
            params: SkillsConfigWriteParams {
                path: path.map(|path| {
                    codex_utils_absolute_path::AbsolutePathBuf::resolve_path_against_base(
                        path, &self.cwd,
                    )
                }),
                name,
                enabled,
            },
        })
        .await
    }

    pub async fn list_plugins(
        &self,
        cwds: Option<Vec<PathBuf>>,
    ) -> anyhow::Result<PluginListResponse> {
        self.request_typed(ClientRequest::PluginList {
            request_id: self.next_request_id(),
            params: PluginListParams {
                cwds: cwds.map(|paths| {
                    paths
                        .into_iter()
                        .map(|path| {
                            codex_utils_absolute_path::AbsolutePathBuf::resolve_path_against_base(
                                path, &self.cwd,
                            )
                        })
                        .collect()
                }),
            },
        })
        .await
    }

    pub async fn read_plugin(
        &self,
        request: CodexPluginRefRequest,
    ) -> anyhow::Result<PluginReadResponse> {
        self.request_typed(ClientRequest::PluginRead {
            request_id: self.next_request_id(),
            params: PluginReadParams {
                marketplace_path: request.marketplace_path.map(|path| {
                    codex_utils_absolute_path::AbsolutePathBuf::resolve_path_against_base(
                        path, &self.cwd,
                    )
                }),
                remote_marketplace_name: request.remote_marketplace_name,
                plugin_name: request.plugin_name,
            },
        })
        .await
    }

    pub async fn install_plugin(
        &self,
        request: CodexPluginRefRequest,
    ) -> anyhow::Result<PluginInstallResponse> {
        self.request_typed(ClientRequest::PluginInstall {
            request_id: self.next_request_id(),
            params: PluginInstallParams {
                marketplace_path: request.marketplace_path.map(|path| {
                    codex_utils_absolute_path::AbsolutePathBuf::resolve_path_against_base(
                        path, &self.cwd,
                    )
                }),
                remote_marketplace_name: request.remote_marketplace_name,
                plugin_name: request.plugin_name,
            },
        })
        .await
    }

    pub async fn uninstall_plugin(
        &self,
        plugin_id: String,
    ) -> anyhow::Result<PluginUninstallResponse> {
        self.request_typed(ClientRequest::PluginUninstall {
            request_id: self.next_request_id(),
            params: PluginUninstallParams { plugin_id },
        })
        .await
    }

    pub async fn add_marketplace(&self, source: String) -> anyhow::Result<MarketplaceAddResponse> {
        self.request_typed(ClientRequest::MarketplaceAdd {
            request_id: self.next_request_id(),
            params: MarketplaceAddParams {
                source,
                ref_name: None,
                sparse_paths: None,
            },
        })
        .await
    }

    pub async fn remove_marketplace(
        &self,
        marketplace_name: String,
    ) -> anyhow::Result<MarketplaceRemoveResponse> {
        self.request_typed(ClientRequest::MarketplaceRemove {
            request_id: self.next_request_id(),
            params: MarketplaceRemoveParams { marketplace_name },
        })
        .await
    }

    pub async fn upgrade_marketplace(
        &self,
        marketplace_name: Option<String>,
    ) -> anyhow::Result<MarketplaceUpgradeResponse> {
        self.request_typed(ClientRequest::MarketplaceUpgrade {
            request_id: self.next_request_id(),
            params: MarketplaceUpgradeParams { marketplace_name },
        })
        .await
    }

    pub async fn mcp_oauth_login(
        &self,
        server: String,
    ) -> anyhow::Result<McpServerOauthLoginResponse> {
        self.request_typed(ClientRequest::McpServerOauthLogin {
            request_id: self.next_request_id(),
            params: McpServerOauthLoginParams {
                name: server,
                scopes: None,
                timeout_secs: None,
            },
        })
        .await
    }

    pub async fn start_review(
        &self,
        thread_id: String,
        prompt: Option<String>,
    ) -> anyhow::Result<ReviewStartResponse> {
        self.request_typed(ClientRequest::ReviewStart {
            request_id: self.next_request_id(),
            params: ReviewStartParams {
                thread_id,
                target: match prompt.filter(|value| !value.trim().is_empty()) {
                    Some(instructions) => ReviewTarget::Custom { instructions },
                    None => ReviewTarget::UncommittedChanges,
                },
                delivery: Some(ReviewDelivery::Inline),
            },
        })
        .await
    }

    pub async fn exec_command(
        &self,
        request: CodexExecRequest,
    ) -> anyhow::Result<CommandExecResponse> {
        self.request_typed(ClientRequest::OneOffCommandExec {
            request_id: self.next_request_id(),
            params: request.try_into()?,
        })
        .await
    }

    pub async fn app_server_request(
        &self,
        method: String,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<JsonRpcResult> {
        let mut value = serde_json::Map::new();
        value.insert(
            "id".to_string(),
            serde_json::to_value(self.next_request_id())?,
        );
        value.insert("method".to_string(), serde_json::Value::String(method));
        if let Some(params) = params {
            value.insert("params".to_string(), params);
        }

        let request = serde_json::from_value::<ClientRequest>(serde_json::Value::Object(value))
            .context("invalid Codex app-server request")?;
        let response = self
            .handle()?
            .request(request)
            .await
            .map_err(|error| anyhow::anyhow!("Codex request transport failed: {error}"))?;

        response.map_err(|error| {
            anyhow::anyhow!(
                "Codex request failed (code {}): {}",
                error.code,
                error.message
            )
        })
    }

    pub async fn exec_write(
        &self,
        params: CommandExecWriteParams,
    ) -> anyhow::Result<CommandExecWriteResponse> {
        self.request_typed(ClientRequest::CommandExecWrite {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn exec_terminate(
        &self,
        process_id: String,
    ) -> anyhow::Result<CommandExecTerminateResponse> {
        self.request_typed(ClientRequest::CommandExecTerminate {
            request_id: self.next_request_id(),
            params: CommandExecTerminateParams { process_id },
        })
        .await
    }

    pub async fn exec_resize(
        &self,
        params: CommandExecResizeParams,
    ) -> anyhow::Result<CommandExecResizeResponse> {
        self.request_typed(ClientRequest::CommandExecResize {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn start_windows_sandbox_setup(
        &self,
        params: WindowsSandboxSetupStartParams,
    ) -> anyhow::Result<WindowsSandboxSetupStartResponse> {
        self.request_typed(ClientRequest::WindowsSandboxSetupStart {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn refresh_mcp(&self) -> anyhow::Result<McpServerRefreshResponse> {
        self.request_typed(ClientRequest::McpServerRefresh {
            request_id: self.next_request_id(),
            params: None,
        })
        .await
    }

    pub async fn list_mcp_status(
        &self,
        detail: Option<McpServerStatusDetail>,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> anyhow::Result<ListMcpServerStatusResponse> {
        self.request_typed(ClientRequest::McpServerStatusList {
            request_id: self.next_request_id(),
            params: ListMcpServerStatusParams {
                cursor,
                limit,
                detail,
            },
        })
        .await
    }

    pub async fn read_mcp_resource(
        &self,
        server: String,
        uri: String,
    ) -> anyhow::Result<McpResourceReadResponse> {
        self.request_typed(ClientRequest::McpResourceRead {
            request_id: self.next_request_id(),
            params: McpResourceReadParams {
                thread_id: self.thread_id.clone(),
                server,
                uri,
            },
        })
        .await
    }

    pub async fn call_mcp_tool(
        &self,
        thread_id: String,
        server: String,
        tool: String,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<McpServerToolCallResponse> {
        self.request_typed(ClientRequest::McpServerToolCall {
            request_id: self.next_request_id(),
            params: McpServerToolCallParams {
                thread_id,
                server,
                tool,
                arguments,
                meta,
            },
        })
        .await
    }

    pub async fn read_config(&self, include_layers: bool) -> anyhow::Result<ConfigReadResponse> {
        self.request_typed(ClientRequest::ConfigRead {
            request_id: self.next_request_id(),
            params: ConfigReadParams {
                include_layers,
                cwd: Some(self.cwd.to_string_lossy().into_owned()),
            },
        })
        .await
    }

    pub async fn read_config_requirements(&self) -> anyhow::Result<ConfigRequirementsReadResponse> {
        self.request_typed(ClientRequest::ConfigRequirementsRead {
            request_id: self.next_request_id(),
            params: None,
        })
        .await
    }

    pub async fn detect_external_agent_config(
        &self,
        params: ExternalAgentConfigDetectParams,
    ) -> anyhow::Result<ExternalAgentConfigDetectResponse> {
        self.request_typed(ClientRequest::ExternalAgentConfigDetect {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn import_external_agent_config(
        &self,
        params: ExternalAgentConfigImportParams,
    ) -> anyhow::Result<ExternalAgentConfigImportResponse> {
        self.request_typed(ClientRequest::ExternalAgentConfigImport {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn write_config_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> anyhow::Result<ConfigWriteResponse> {
        self.request_typed(ClientRequest::ConfigValueWrite {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn write_config_batch(
        &self,
        params: ConfigBatchWriteParams,
    ) -> anyhow::Result<ConfigWriteResponse> {
        self.request_typed(ClientRequest::ConfigBatchWrite {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn upload_feedback(
        &self,
        request: CodexFeedbackRequest,
    ) -> anyhow::Result<FeedbackUploadResponse> {
        self.request_typed(ClientRequest::FeedbackUpload {
            request_id: self.next_request_id(),
            params: request.into(),
        })
        .await
    }

    pub async fn set_thread_memory_mode(
        &self,
        thread_id: String,
        enabled: bool,
    ) -> anyhow::Result<ThreadMemoryModeSetResponse> {
        self.request_typed(ClientRequest::ThreadMemoryModeSet {
            request_id: self.next_request_id(),
            params: ThreadMemoryModeSetParams {
                thread_id,
                mode: if enabled {
                    ThreadMemoryMode::Enabled
                } else {
                    ThreadMemoryMode::Disabled
                },
            },
        })
        .await
    }

    pub async fn reset_memories(
        &self,
    ) -> anyhow::Result<codex_app_server_protocol::MemoryResetResponse> {
        self.request_typed(ClientRequest::MemoryReset {
            request_id: self.next_request_id(),
            params: None,
        })
        .await
    }

    pub async fn start_realtime(
        &self,
        params: ThreadRealtimeStartParams,
    ) -> anyhow::Result<ThreadRealtimeStartResponse> {
        self.request_typed(ClientRequest::ThreadRealtimeStart {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn append_realtime_audio(
        &self,
        params: ThreadRealtimeAppendAudioParams,
    ) -> anyhow::Result<ThreadRealtimeAppendAudioResponse> {
        self.request_typed(ClientRequest::ThreadRealtimeAppendAudio {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn append_realtime_text(
        &self,
        params: ThreadRealtimeAppendTextParams,
    ) -> anyhow::Result<ThreadRealtimeAppendTextResponse> {
        self.request_typed(ClientRequest::ThreadRealtimeAppendText {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn stop_realtime(
        &self,
        params: ThreadRealtimeStopParams,
    ) -> anyhow::Result<ThreadRealtimeStopResponse> {
        self.request_typed(ClientRequest::ThreadRealtimeStop {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn list_realtime_voices(&self) -> anyhow::Result<ThreadRealtimeListVoicesResponse> {
        self.request_typed(ClientRequest::ThreadRealtimeListVoices {
            request_id: self.next_request_id(),
            params: ThreadRealtimeListVoicesParams::default(),
        })
        .await
    }

    pub async fn create_device_key(
        &self,
        params: DeviceKeyCreateParams,
    ) -> anyhow::Result<DeviceKeyCreateResponse> {
        self.request_typed(ClientRequest::DeviceKeyCreate {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn get_device_key_public(
        &self,
        params: DeviceKeyPublicParams,
    ) -> anyhow::Result<DeviceKeyPublicResponse> {
        self.request_typed(ClientRequest::DeviceKeyPublic {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn sign_device_key(
        &self,
        params: DeviceKeySignParams,
    ) -> anyhow::Result<DeviceKeySignResponse> {
        self.request_typed(ClientRequest::DeviceKeySign {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_read_file(
        &self,
        params: FsReadFileParams,
    ) -> anyhow::Result<FsReadFileResponse> {
        self.request_typed(ClientRequest::FsReadFile {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_write_file(
        &self,
        params: FsWriteFileParams,
    ) -> anyhow::Result<FsWriteFileResponse> {
        self.request_typed(ClientRequest::FsWriteFile {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> anyhow::Result<FsCreateDirectoryResponse> {
        self.request_typed(ClientRequest::FsCreateDirectory {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> anyhow::Result<FsGetMetadataResponse> {
        self.request_typed(ClientRequest::FsGetMetadata {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> anyhow::Result<FsReadDirectoryResponse> {
        self.request_typed(ClientRequest::FsReadDirectory {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_remove(&self, params: FsRemoveParams) -> anyhow::Result<FsRemoveResponse> {
        self.request_typed(ClientRequest::FsRemove {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_copy(&self, params: FsCopyParams) -> anyhow::Result<FsCopyResponse> {
        self.request_typed(ClientRequest::FsCopy {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_watch(&self, params: FsWatchParams) -> anyhow::Result<FsWatchResponse> {
        self.request_typed(ClientRequest::FsWatch {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fs_unwatch(&self, params: FsUnwatchParams) -> anyhow::Result<FsUnwatchResponse> {
        self.request_typed(ClientRequest::FsUnwatch {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn fuzzy_file_search(
        &self,
        params: FuzzyFileSearchParams,
    ) -> anyhow::Result<FuzzyFileSearchResponse> {
        self.request_typed(ClientRequest::FuzzyFileSearch {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn start_fuzzy_file_search_session(
        &self,
        params: FuzzyFileSearchSessionStartParams,
    ) -> anyhow::Result<FuzzyFileSearchSessionStartResponse> {
        self.request_typed(ClientRequest::FuzzyFileSearchSessionStart {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn update_fuzzy_file_search_session(
        &self,
        params: FuzzyFileSearchSessionUpdateParams,
    ) -> anyhow::Result<FuzzyFileSearchSessionUpdateResponse> {
        self.request_typed(ClientRequest::FuzzyFileSearchSessionUpdate {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn stop_fuzzy_file_search_session(
        &self,
        params: FuzzyFileSearchSessionStopParams,
    ) -> anyhow::Result<FuzzyFileSearchSessionStopResponse> {
        self.request_typed(ClientRequest::FuzzyFileSearchSessionStop {
            request_id: self.next_request_id(),
            params,
        })
        .await
    }

    pub async fn resolve_codex_server_request(
        &mut self,
        request_id: &str,
        decision: PermissionDecision,
        resolution: CodexServerRequestResolution,
    ) -> anyhow::Result<()> {
        let kind = {
            let mut state = self.event_state.lock().await;
            state.pending_server_requests.remove(request_id)
        }
        .ok_or_else(|| anyhow::anyhow!("unknown Codex permission request id {request_id}"))?;
        let req_id = RequestId::String(request_id.to_owned());

        let rejection_message = match &kind {
            PendingServerRequestKind::ChatgptAuthRefresh { .. } => {
                "chatgpt auth token refresh is not supported for in-process app-server clients"
                    .to_string()
            }
            _ => "Permission denied by user".to_string(),
        };

        if let Some(response) = typed_server_request_response(kind, decision, resolution)? {
            self.handle()?
                .resolve_server_request(req_id, response)
                .await
                .map_err(|e| anyhow::anyhow!("resolve_server_request failed: {e}"))?;
        } else {
            self.handle()?
                .reject_server_request(
                    req_id,
                    JSONRPCErrorError {
                        code: -32000,
                        message: rejection_message,
                        data: None,
                    },
                )
                .await
                .map_err(|e| anyhow::anyhow!("reject_server_request failed: {e}"))?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AgentAdapter implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl AgentAdapter for CodexInProcessAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        info!("CodexInProcessAdapter starting");

        // Apply config overrides.
        if let Some(ref cwd) = config.working_dir {
            self.cwd = cwd.clone();
        }
        if let Some(ref model) = config.model {
            self.model = Some(model.clone());
        }

        self.session_id = Some(uuid::Uuid::new_v4().to_string());
        let session_id = self.session_id.as_ref().unwrap().clone();

        // Take the client and spawn the background event pump.
        let client = self._client_placeholder.take().ok_or_else(|| {
            anyhow::anyhow!("Codex client not set — call set_client() before start()")
        })?;

        // Request handle was already extracted in new()/set_client().
        let handle = self.event_state.clone();
        let worker = tokio::spawn(Self::event_pump(client, handle, session_id));
        self.worker_handle = Some(worker);

        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;

        info!("CodexInProcessAdapter ready");
        Ok(())
    }

    async fn send_message(
        &mut self,
        _session_id: &str,
        message: &str,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        // Ensure we have a thread. Use the request handle (&self, no borrow conflict).
        let thread_id = self.ensure_thread().await?;
        self.thread_id = Some(thread_id.clone());

        // Interrupt any in-progress turn before starting a new one to avoid
        // conflicts.  Errors are logged but not fatal — there may be no
        // active turn.
        if let Err(err) = self
            .handle()?
            .request_typed::<TurnInterruptResponse>(ClientRequest::TurnInterrupt {
                request_id: self.next_request_id(),
                params: TurnInterruptParams {
                    thread_id: thread_id.clone(),
                    turn_id: String::new(), // empty = current turn
                },
            })
            .await
        {
            warn!(error = %err, "Pre-turn interrupt failed (no active turn?)");
        }

        // Create a new channel for this turn's events.
        let (tx, rx) = mpsc::channel(256);

        // Install the sender in the shared state so the event pump can forward
        // events to it. Do this BEFORE starting the turn so we don't miss any
        // events.
        {
            let mut state = self.event_state.lock().await;
            state.current_tx = Some(tx);
        }

        // Send TurnStart via the request handle.
        let request_id = self.next_request_id();
        let cwd = self.cwd.clone();

        let user_input = ProtocolUserInput::Text {
            text: message.to_owned(),
            text_elements: Vec::new(),
        };

        let _response: TurnStartResponse = self
            .handle()?
            .request_typed(ClientRequest::TurnStart {
                request_id,
                params: TurnStartParams {
                    thread_id: thread_id.clone(),
                    input: vec![user_input],
                    cwd: Some(cwd.clone()),
                    model: self.model.clone(),
                    service_tier: self.service_tier.clone(),
                    approval_policy: self.approval_policy,
                    permission_profile: self.permission_profile.clone(),
                    ..Default::default()
                },
            })
            .await
            .map_err(|e| anyhow::anyhow!("turn/start failed: {e}"))?;

        Ok(rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> anyhow::Result<()> {
        info!("Cancelling Codex turn");

        let thread_id = self
            .thread_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active thread"))?;

        let result = self
            .handle()?
            .request_typed::<codex_app_server_protocol::TurnInterruptResponse>(
                ClientRequest::TurnInterrupt {
                    request_id: self.next_request_id(),
                    params: codex_app_server_protocol::TurnInterruptParams {
                        thread_id,
                        turn_id: String::new(), // empty = current turn
                    },
                },
            )
            .await;

        if let Err(err) = result {
            warn!(error = %err, "turn/interrupt failed (may be no active turn)");
        }

        Ok(())
    }

    async fn resolve_permission(
        &mut self,
        _session_id: &str,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        self.resolve_codex_server_request(
            request_id,
            decision,
            CodexServerRequestResolution::default(),
        )
        .await
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("CodexInProcessAdapter stopping");

        // Drain and reject all pending server requests so the Codex runtime
        // doesn't hang waiting for responses.
        {
            let mut state = self.event_state.lock().await;
            state.current_tx = None;
            let pending_ids: Vec<String> =
                state.pending_server_requests.keys().cloned().collect();
            state.pending_server_requests.clear();
            drop(state);

            // Reject each pending request.  Best-effort — errors are logged
            // but don't prevent shutdown.
            if let Ok(handle) = self.handle() {
                for id in pending_ids {
                    let req_id = RequestId::String(id);
                    if let Err(err) = handle
                        .reject_server_request(
                            req_id,
                            JSONRPCErrorError {
                                code: -32000,
                                message: "Adapter shutting down".to_string(),
                                data: None,
                            },
                        )
                        .await
                    {
                        warn!(error = %err, "Failed to reject pending request during stop");
                    }
                }
            }
        }

        // Drop the request handle.
        self.request_handle = None;

        // Abort the background event pump.
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }

        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        !matches!(self.status, AgentStatus::Stopped | AgentStatus::Error)
            && self.request_handle.is_some()
            && self
                .worker_handle
                .as_ref()
                .map_or(false, |h| !h.is_finished())
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::RemoteCodex
    }
}

fn build_harness_overrides(
    options: &CodexAdapterOptions,
    cwd: &std::path::Path,
) -> anyhow::Result<codex_core::config::ConfigOverrides> {
    Ok(codex_core::config::ConfigOverrides {
        model: options.model.clone(),
        cwd: Some(cwd.to_path_buf()),
        approval_policy: options
            .approval_policy
            .as_deref()
            .map(parse_approval_policy_core)
            .transpose()?,
        sandbox_mode: options
            .sandbox_mode
            .as_deref()
            .map(parse_sandbox_mode_core)
            .transpose()?,
        permission_profile: options
            .permission_profile
            .clone()
            .map(serde_json::from_value::<codex_app_server_protocol::PermissionProfile>)
            .transpose()
            .context("invalid Codex permission profile")?
            .map(CorePermissionProfile::from),
        model_provider: options.model_provider.clone(),
        service_tier: options
            .service_tier
            .clone()
            .map(serde_json::from_value)
            .transpose()
            .context("invalid Codex service tier")?,
        ephemeral: options.ephemeral,
        ..Default::default()
    })
}

fn build_cli_overrides(options: &CodexAdapterOptions) -> anyhow::Result<Vec<(String, TomlValue)>> {
    let mut overrides = Vec::new();

    if let Some(endpoint) = trim_opt(options.thread_store_endpoint.clone()) {
        overrides.push((
            "experimental_thread_store".to_string(),
            toml::from_str::<TomlValue>(&format!(
                "{{ type = \"remote\", endpoint = {} }}",
                toml_string(&endpoint)
            ))
            .context("failed to build thread store override")?,
        ));
    } else {
        overrides.push((
            "experimental_thread_store".to_string(),
            toml::from_str::<TomlValue>("{ type = \"local\" }")
                .context("failed to build local thread store override")?,
        ));
    }

    if let Some(enabled) = options.memories_enabled {
        overrides.push((
            "memories.generate_memories".to_string(),
            TomlValue::Boolean(enabled),
        ));
        overrides.push((
            "memories.use_memories".to_string(),
            TomlValue::Boolean(enabled),
        ));
    }

    if let Some(provider_id) = trim_opt(options.model_provider.clone()) {
        let provider_prefix = format!("model_providers.{provider_id}");
        overrides.push((
            "model_provider".to_string(),
            TomlValue::String(provider_id.clone()),
        ));
        overrides.push((
            format!("{provider_prefix}.name"),
            TomlValue::String(provider_id.clone()),
        ));
        overrides.push((
            format!("{provider_prefix}.wire_api"),
            TomlValue::String("responses".to_string()),
        ));
        if let Some(base_url) = trim_opt(options.base_url.clone()) {
            overrides.push((
                format!("{provider_prefix}.base_url"),
                TomlValue::String(base_url),
            ));
        }
        if let Some(api_key) = trim_opt(options.api_key.clone()) {
            overrides.push((
                format!("{provider_prefix}.experimental_bearer_token"),
                TomlValue::String(api_key),
            ));
        }
    } else if let Some(base_url) = trim_opt(options.base_url.clone()) {
        overrides.push(("openai_base_url".to_string(), TomlValue::String(base_url)));
    }

    for (name, value) in &options.mcp_servers {
        overrides.push((format!("mcp_servers.{name}"), json_to_toml(value.clone())?));
    }

    for (key, raw) in &options.config_overrides {
        overrides.push((key.clone(), parse_toml_scalar(raw)));
    }

    Ok(overrides)
}

fn build_thread_config_overrides_json(
    options: &CodexAdapterOptions,
) -> HashMap<String, serde_json::Value> {
    let mut config = HashMap::new();
    if let Some(enabled) = options.memories_enabled {
        config.insert(
            "memories".to_string(),
            serde_json::json!({
                "generate_memories": enabled,
                "use_memories": enabled,
            }),
        );
    }
    config
}

fn parse_toml_scalar(raw: &str) -> TomlValue {
    let wrapped = format!("_x_ = {raw}");
    toml::from_str::<toml::Table>(&wrapped)
        .ok()
        .and_then(|table| table.get("_x_").cloned())
        .unwrap_or_else(|| TomlValue::String(raw.trim_matches(['"', '\'']).to_string()))
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn json_to_toml(value: serde_json::Value) -> anyhow::Result<TomlValue> {
    match value {
        serde_json::Value::Null => Ok(TomlValue::String(String::new())),
        serde_json::Value::Bool(v) => Ok(TomlValue::Boolean(v)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(TomlValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(TomlValue::Float(f))
            } else {
                Err(anyhow::anyhow!(
                    "unsupported JSON number for TOML conversion"
                ))
            }
        }
        serde_json::Value::String(v) => Ok(TomlValue::String(v)),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(json_to_toml)
            .collect::<anyhow::Result<Vec<_>>>()
            .map(TomlValue::Array),
        serde_json::Value::Object(values) => {
            let mut table = toml::map::Map::new();
            for (key, value) in values {
                if !value.is_null() {
                    table.insert(key, json_to_toml(value)?);
                }
            }
            Ok(TomlValue::Table(table))
        }
    }
}

fn parse_approval_policy(value: &str) -> anyhow::Result<AskForApproval> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "untrusted" | "unless-trusted" | "unless_trusted" => Ok(AskForApproval::UnlessTrusted),
        "on-failure" | "on_failure" | "onfailure" => Ok(AskForApproval::OnFailure),
        "on-request" | "on_request" | "onrequest" => Ok(AskForApproval::OnRequest),
        "never" => Ok(AskForApproval::Never),
        other => Err(anyhow::anyhow!(
            "unsupported Codex approval policy `{other}`"
        )),
    }
}

fn parse_approval_policy_core(
    value: &str,
) -> anyhow::Result<codex_protocol::protocol::AskForApproval> {
    Ok(parse_approval_policy(value)?.to_core())
}

fn parse_sandbox_mode(value: &str) -> anyhow::Result<SandboxMode> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "read-only" | "readonly" | "read_only" => Ok(SandboxMode::ReadOnly),
        "workspace-write" | "workspace_write" | "workspacewrite" => Ok(SandboxMode::WorkspaceWrite),
        "danger-full-access" | "danger_full_access" | "dangerfullaccess" | "none" => {
            Ok(SandboxMode::DangerFullAccess)
        }
        other => Err(anyhow::anyhow!("unsupported Codex sandbox mode `{other}`")),
    }
}

fn parse_sandbox_mode_core(
    value: &str,
) -> anyhow::Result<codex_protocol::config_types::SandboxMode> {
    Ok(parse_sandbox_mode(value)?.to_core())
}

fn typed_server_request_response(
    kind: PendingServerRequestKind,
    decision: PermissionDecision,
    resolution: CodexServerRequestResolution,
) -> anyhow::Result<Option<serde_json::Value>> {
    let allow = matches!(
        decision,
        PermissionDecision::Allow | PermissionDecision::AllowAll
    );
    let allow_all = matches!(decision, PermissionDecision::AllowAll) || resolution.allow_all;

    if let Some(ref response) = resolution.response {
        if allow {
            return Ok(Some(response.clone()));
        }
    }

    if !allow
        && !matches!(
            &kind,
            PendingServerRequestKind::CommandExecution
                | PendingServerRequestKind::FileChange
                | PendingServerRequestKind::ApplyPatch
                | PendingServerRequestKind::ExecCommand
                | PendingServerRequestKind::McpElicitation
        )
    {
        return Ok(None);
    }

    let value = match kind {
        PendingServerRequestKind::CommandExecution => serde_json::to_value(
            codex_app_server_protocol::CommandExecutionRequestApprovalResponse {
                decision: if allow_all {
                    codex_app_server_protocol::CommandExecutionApprovalDecision::AcceptForSession
                } else if allow {
                    codex_app_server_protocol::CommandExecutionApprovalDecision::Accept
                } else {
                    codex_app_server_protocol::CommandExecutionApprovalDecision::Decline
                },
            },
        )?,
        PendingServerRequestKind::FileChange => serde_json::to_value(
            codex_app_server_protocol::FileChangeRequestApprovalResponse {
                decision: if allow_all {
                    FileChangeApprovalDecision::AcceptForSession
                } else if allow {
                    FileChangeApprovalDecision::Accept
                } else {
                    FileChangeApprovalDecision::Decline
                },
            },
        )?,
        PendingServerRequestKind::ApplyPatch => {
            serde_json::to_value(codex_app_server_protocol::ApplyPatchApprovalResponse {
                decision: if allow {
                    ReviewDecision::Approved
                } else {
                    ReviewDecision::Denied
                },
            })?
        }
        PendingServerRequestKind::ExecCommand => {
            serde_json::to_value(codex_app_server_protocol::ExecCommandApprovalResponse {
                decision: if allow_all {
                    ReviewDecision::ApprovedForSession
                } else if allow {
                    ReviewDecision::Approved
                } else {
                    ReviewDecision::Denied
                },
            })?
        }
        PendingServerRequestKind::Permissions(requested_permissions) => serde_json::to_value(
            codex_app_server_protocol::PermissionsRequestApprovalResponse {
                permissions: requested_permissions_to_granted_profile(requested_permissions)?,
                scope: if allow_all {
                    PermissionGrantScope::Session
                } else {
                    PermissionGrantScope::Turn
                },
                strict_auto_review: None,
            },
        )?,
        PendingServerRequestKind::McpElicitation => {
            serde_json::to_value(McpServerElicitationRequestResponse {
                action: if allow {
                    McpServerElicitationAction::Accept
                } else {
                    McpServerElicitationAction::Decline
                },
                content: None,
                meta: None,
            })?
        }
        PendingServerRequestKind::ToolUserInput(questions) => {
            serde_json::to_value(codex_app_server_protocol::ToolRequestUserInputResponse {
                answers: default_tool_user_input_answers(questions),
            })?
        }
        PendingServerRequestKind::DynamicTool { .. } => {
            serde_json::to_value(DynamicToolCallResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: if allow {
                        "No dynamic client tool handler is registered.".to_string()
                    } else {
                        "Dynamic tool call denied by user.".to_string()
                    },
                }],
                success: false,
            })?
        }
        PendingServerRequestKind::ChatgptAuthRefresh { .. } => {
            if let Some(response) = resolution.response {
                let _refresh_response: ChatgptAuthTokensRefreshResponse =
                    serde_json::from_value(response.clone())
                    .with_context(|| "Invalid ChatgptAuthTokensRefreshResponse in resolution")?;
                return Ok(Some(response));
            }
            return Ok(None);
        }
    };

    Ok(Some(value))
}

fn requested_permissions_to_granted_profile(
    value: serde_json::Value,
) -> anyhow::Result<GrantedPermissionProfile> {
    if value.is_null() {
        return Ok(GrantedPermissionProfile::default());
    }

    let mut granted = serde_json::Map::new();
    if let Some(network) = value.get("network").cloned() {
        granted.insert("network".to_owned(), network);
    }
    if let Some(file_system) = value.get("fileSystem").cloned() {
        granted.insert("fileSystem".to_owned(), file_system);
    }

    serde_json::from_value(serde_json::Value::Object(granted))
        .context("failed to convert requested permissions to granted permissions")
}

fn default_tool_user_input_answers(
    questions: serde_json::Value,
) -> HashMap<String, codex_app_server_protocol::ToolRequestUserInputAnswer> {
    questions
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|question| {
            let id = question.get("id")?.as_str()?.to_owned();
            let answers = question
                .get("options")
                .and_then(serde_json::Value::as_array)
                .and_then(|options| options.first())
                .and_then(|option| option.get("label"))
                .and_then(serde_json::Value::as_str)
                .map(|label| vec![label.to_owned()])
                .unwrap_or_default();
            Some((
                id,
                codex_app_server_protocol::ToolRequestUserInputAnswer { answers },
            ))
        })
        .collect()
}

fn thread_list_params_from_request(request: CodexThreadListRequest) -> ThreadListParams {
    ThreadListParams {
        cursor: request.cursor,
        limit: request.limit,
        sort_key: request.sort_key.as_deref().and_then(parse_thread_sort_key),
        sort_direction: request
            .sort_direction
            .as_deref()
            .and_then(parse_sort_direction),
        model_providers: request.model_providers,
        source_kinds: request.source_kinds.map(|values| {
            values
                .into_iter()
                .filter_map(|value| parse_thread_source_kind(&value))
                .collect()
        }),
        archived: request.archived,
        cwd: request.cwd.and_then(parse_thread_cwd_filter),
        use_state_db_only: request.use_state_db_only,
        search_term: request.search_term,
    }
}

fn parse_thread_sort_key(value: &str) -> Option<ThreadSortKey> {
    match value.trim().to_ascii_lowercase().as_str() {
        "created_at" | "createdat" | "created-at" => Some(ThreadSortKey::CreatedAt),
        "updated_at" | "updatedat" | "updated-at" => Some(ThreadSortKey::UpdatedAt),
        _ => None,
    }
}

fn parse_sort_direction(value: &str) -> Option<SortDirection> {
    match value.trim().to_ascii_lowercase().as_str() {
        "asc" | "ascending" => Some(SortDirection::Asc),
        "desc" | "descending" => Some(SortDirection::Desc),
        _ => None,
    }
}

fn parse_thread_source_kind(value: &str) -> Option<ThreadSourceKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cli" => Some(ThreadSourceKind::Cli),
        "vscode" | "vs_code" | "vs-code" => Some(ThreadSourceKind::VsCode),
        "exec" => Some(ThreadSourceKind::Exec),
        "appserver" | "app-server" | "app_server" => Some(ThreadSourceKind::AppServer),
        "subagent" | "sub-agent" | "sub_agent" => Some(ThreadSourceKind::SubAgent),
        "unknown" => Some(ThreadSourceKind::Unknown),
        _ => None,
    }
}

fn parse_thread_cwd_filter(value: serde_json::Value) -> Option<ThreadListCwdFilter> {
    match value {
        serde_json::Value::String(path) => Some(ThreadListCwdFilter::One(path)),
        serde_json::Value::Array(values) => Some(ThreadListCwdFilter::Many(
            values
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
        )),
        _ => None,
    }
}

fn request_id_to_string(id: &RequestId) -> String {
    match id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(n) => n.to_string(),
    }
}

fn trim_opt(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response_json(
        kind: PendingServerRequestKind,
        decision: PermissionDecision,
        resolution: CodexServerRequestResolution,
    ) -> serde_json::Value {
        typed_server_request_response(kind, decision, resolution)
            .expect("request response should serialize")
            .expect("request kind should produce a response")
    }

    #[test]
    fn command_execution_allow_uses_turn_scope_decision() {
        let response = response_json(
            PendingServerRequestKind::CommandExecution,
            PermissionDecision::Allow,
            CodexServerRequestResolution::default(),
        );

        assert_eq!(response, json!({ "decision": "accept" }));
    }

    #[test]
    fn command_execution_allow_all_uses_session_scope_decision() {
        let response = response_json(
            PendingServerRequestKind::CommandExecution,
            PermissionDecision::AllowAll,
            CodexServerRequestResolution::default(),
        );

        assert_eq!(response, json!({ "decision": "acceptForSession" }));
    }

    #[test]
    fn resolution_allow_all_upgrades_allow_to_session_scope() {
        let response = response_json(
            PendingServerRequestKind::ExecCommand,
            PermissionDecision::Allow,
            CodexServerRequestResolution {
                allow_all: true,
                response: None,
            },
        );

        assert_eq!(response, json!({ "decision": "approved_for_session" }));
    }

    #[test]
    fn hard_deny_returns_none_for_untyped_or_unavailable_response_kinds() {
        for kind in [
            PendingServerRequestKind::Permissions(json!({
                "network": { "enabled": true }
            })),
            PendingServerRequestKind::ToolUserInput(json!([])),
            PendingServerRequestKind::DynamicTool {
                call_id: "test".to_string(),
                namespace: None,
                tool: "test_tool".to_string(),
                arguments: json!(null),
            },
            PendingServerRequestKind::ChatgptAuthRefresh {
                reason: "Unauthorized".to_string(),
                previous_account_id: None,
            },
        ] {
            let response = typed_server_request_response(
                kind,
                PermissionDecision::Deny,
                CodexServerRequestResolution::default(),
            )
            .expect("deny should not fail");

            assert_eq!(response, None);
        }
    }

    #[test]
    fn permissions_grant_preserves_requested_network_and_file_system() {
        let requested = json!({
            "network": { "enabled": true },
            "fileSystem": {
                "read": ["C:\\repo"],
                "write": ["C:\\repo\\out"]
            }
        });

        let response = response_json(
            PendingServerRequestKind::Permissions(requested),
            PermissionDecision::Allow,
            CodexServerRequestResolution::default(),
        );

        assert_eq!(response["scope"], "turn");
        assert_eq!(
            response["permissions"]["network"],
            json!({ "enabled": true })
        );
        assert_eq!(
            response["permissions"]["fileSystem"],
            json!({
                "read": ["C:\\repo"],
                "write": ["C:\\repo\\out"]
            })
        );
    }

    #[test]
    fn permissions_allow_all_uses_session_scope() {
        let response = response_json(
            PendingServerRequestKind::Permissions(json!({
                "network": { "enabled": true }
            })),
            PermissionDecision::AllowAll,
            CodexServerRequestResolution::default(),
        );

        assert_eq!(response["scope"], "session");
    }

    #[test]
    fn tool_user_input_defaults_to_first_option_label() {
        let response = response_json(
            PendingServerRequestKind::ToolUserInput(json!([
                {
                    "id": "approval_mode",
                    "header": "Mode",
                    "question": "Choose a mode",
                    "options": [
                        { "label": "Careful", "description": "Ask first" },
                        { "label": "Fast", "description": "Proceed" }
                    ]
                }
            ])),
            PermissionDecision::Allow,
            CodexServerRequestResolution::default(),
        );

        assert_eq!(
            response,
            json!({
                "answers": {
                    "approval_mode": {
                        "answers": ["Careful"]
                    }
                }
            })
        );
    }

    #[test]
    fn custom_response_overrides_allow_response() {
        let custom = json!({
            "decision": "custom",
            "extra": true
        });

        let response = response_json(
            PendingServerRequestKind::CommandExecution,
            PermissionDecision::Allow,
            CodexServerRequestResolution {
                allow_all: false,
                response: Some(custom.clone()),
            },
        );

        assert_eq!(response, custom);
    }

    #[test]
    fn custom_response_is_ignored_for_deny() {
        let response = response_json(
            PendingServerRequestKind::CommandExecution,
            PermissionDecision::Deny,
            CodexServerRequestResolution {
                allow_all: false,
                response: Some(json!({ "decision": "accept" })),
            },
        );

        assert_eq!(response, json!({ "decision": "decline" }));
    }

    #[test]
    fn chatgpt_auth_refresh_returns_none_without_tokens() {
        let response = typed_server_request_response(
            PendingServerRequestKind::ChatgptAuthRefresh {
                reason: "Unauthorized".to_string(),
                previous_account_id: None,
            },
            PermissionDecision::Allow,
            CodexServerRequestResolution::default(),
        )
        .expect("auth refresh should not fail");

        assert_eq!(response, None);
    }

    #[test]
    fn chatgpt_auth_refresh_returns_tokens_from_resolution() {
        let tokens = json!({
            "accessToken": "new-token-123",
            "chatgptAccountId": "acct-456",
            "chatgptPlanType": "plus"
        });

        let response = typed_server_request_response(
            PendingServerRequestKind::ChatgptAuthRefresh {
                reason: "Unauthorized".to_string(),
                previous_account_id: None,
            },
            PermissionDecision::Allow,
            CodexServerRequestResolution {
                allow_all: false,
                response: Some(tokens.clone()),
            },
        )
        .expect("auth refresh should not fail");

        assert_eq!(response, Some(tokens));
    }
}
