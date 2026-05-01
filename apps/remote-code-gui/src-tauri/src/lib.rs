mod mobile;
mod query_engine_gui;

use std::collections::{BTreeMap, HashMap};
use std::env;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
// Stdio removed — no longer needed after Roo in-process migration
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use codex_app_server_protocol::{
    CancelLoginAccountParams, CommandExecResizeParams, CommandExecTerminalSize,
    CommandExecWriteParams, ConfigBatchWriteParams, ConfigEdit, ConfigValueWriteParams,
    DeviceKeyCreateParams, DeviceKeyPublicParams, DeviceKeySignParams,
    ExternalAgentConfigDetectParams, ExternalAgentConfigImportParams, FsCopyParams,
    FsCreateDirectoryParams, FsGetMetadataParams, FsReadDirectoryParams, FsReadFileParams,
    FsRemoveParams, FsUnwatchParams, FsWatchParams, FsWriteFileParams, FuzzyFileSearchParams,
    FuzzyFileSearchSessionStartParams, FuzzyFileSearchSessionStopParams,
    FuzzyFileSearchSessionUpdateParams, LoginAccountParams, McpServerStatusDetail, MergeStrategy,
    SendAddCreditsNudgeEmailParams, ThreadApproveGuardianDeniedActionParams, ThreadForkParams,
    ThreadInjectItemsParams, ThreadMetadataGitInfoUpdateParams, ThreadMetadataUpdateParams,
    ThreadRealtimeAppendAudioParams, ThreadRealtimeAppendTextParams, ThreadRealtimeStartParams,
    ThreadRealtimeStopParams, ThreadResumeParams, ThreadStartParams, TurnStartParams,
    WindowsSandboxSetupStartParams,
};
use claude_agent_protocol::AgentAdapter;
use claude_agent_protocol::permission::PermissionDecision as AgentPermissionDecision;
use claude_agent_protocol::types::AgentType as ProtocolAgentType;
use rc_codex_adapter::{
    CodexAdapterOptions, CodexExecRequest, CodexFeedbackRequest, CodexInProcessAdapter,
    CodexPluginRefRequest, CodexServerRequestResolution, CodexThreadGoalSetRequest,
    CodexThreadListRequest, CodexThreadRollbackRequest, CodexTurnInterruptRequest,
    CodexTurnSteerRequest,
};
use rc_roo_adapter::RooInProcessAdapter;
use claude_config::{
    AppPaths, ProviderConfig as RuntimeProviderConfig, ProviderOverrides, RuntimeConfig,
    RuntimeOverrides, SettingSource, discover_env_providers, load_runtime_config,
    normalize_base_url, validate_provider_config,
};
use claude_core::{
    ConversationEntry, ConversationRole, PermissionMode, ProviderProtocol, ToolCall, UsageSummary,
};
use claude_mcp::{
    DEFAULT_MCP_CONFIG_FILE, McpClientInfo, McpConfig, McpServerConfig, McpServerInspection,
    McpTransport, McpTransportConfig, inspect_server,
};
use claude_permissions::{
    LayeredPermissionBroker, PermissionBroker, PermissionClass, PermissionDecision,
    PermissionRequest, PermissionUpdate, PermissionUpdateDestination, auto_allows, classify_tool,
    load_layered_rules, rules::summarize_rule_sources,
};
use claude_plugins::{PluginBundle, discover_plugins_including_disabled};
use claude_provider::ProviderClient;
use claude_provider::model_info::{ModelCapability, get_model_info};
use claude_session::runtime_context::{
    persist_runtime_config_session_context, restore_runtime_config_session_context,
};
use claude_session::{SessionStore, SessionSummary, conversation::ensure_conversation_initialized};
use claude_skills::discover_skills;
use claude_tools::shell::ShellExecutionPolicy;
use claude_tools::{
    ToolRuntimePolicy, configure_tool_runtime_policy,
    mcp_runtime::{
        RuntimeMcpServerObservation, observe_runtime_mcp_servers, runtime_mcp_inventory_summary,
        runtime_mcp_policy_entries,
    },
    runtime_plan_mode::RuntimePlanModeController,
    tasks::load_persisted_ui_task_snapshots,
};
use claude_ui_bridge::{
    UiProviderStatusSnapshot, UiRuntimeMcpInventorySummary, UiRuntimeMcpServerStatus,
    UiRuntimeStatusSnapshot,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::{Mutex, oneshot};
// TokioCommand removed — no longer needed after Roo in-process migration
use tokio::time::{Duration, timeout};
use uuid::Uuid;

const CODEX_GLOBAL_ADAPTER_KEY: &str = "__codex_global__";
const APP_EVENT_PERMISSION_REQUEST: &str = "gui://permission-request";
const APP_EVENT_PERMISSION_RESOLVED: &str = "gui://permission-resolved";
const APP_EVENT_TOOL_START: &str = "gui://tool-start";
const APP_EVENT_TOOL_RESULT: &str = "gui://tool-result";
const APP_EVENT_TOOL_PROGRESS: &str = "gui://tool-progress";
const APP_EVENT_CODEX_APP_SERVER_NOTIFICATION: &str = "gui://codex-app-server-notification";
const APP_EVENT_STREAMING_DELTA: &str = "gui://streaming-delta";
const APP_EVENT_PROMPT_DONE: &str = "gui://prompt-done";
const APP_EVENT_SUBTASK_STARTED: &str = "gui://subtask-started";
const APP_EVENT_SUBTASK_PROGRESS: &str = "gui://subtask-progress";
const APP_EVENT_SUBTASK_COMPLETED: &str = "gui://subtask-completed";
const APP_EVENT_BATCH_PROGRESS: &str = "gui://batch-progress";
const APP_EVENT_TASK_SNAPSHOT: &str = "gui://task-snapshot";
const APP_EVENT_CONTEXT_USAGE: &str = "gui://context-usage";
const APP_EVENT_CONTEXT_OVERFLOW: &str = "gui://context-overflow";
const APP_EVENT_CONTEXT_COMPACTED: &str = "gui://context-compacted";
const APP_EVENT_RUNTIME_STATUS: &str = "gui://runtime-status";
const APP_EVENT_CODEX_RECOVERABLE_ERROR: &str = "gui://codex-recoverable-error";
const PROJECTS_FILE_NAME: &str = "gui-projects.json";
const PROVIDERS_FILE_NAME: &str = "gui-providers.json";
const SETTINGS_FILE_NAME: &str = "gui-settings.json";
const DEFAULT_MAX_TURNS: usize = 128;
const PERMISSION_WAIT_SECS: u64 = 60 * 30;

/// Service name used for OS Keychain / Credential Manager entries.
const KEYRING_SERVICE: &str = "remote-code-gui";

/// Store an API key in the OS keychain for the given provider name.
fn keyring_store(provider_name: &str, api_key: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, provider_name) {
        let _ = entry.set_password(api_key);
    }
}

/// Retrieve an API key from the OS keychain for the given provider name.
fn keyring_retrieve(provider_name: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, provider_name)
        .ok()
        .and_then(|entry| entry.get_password().ok())
}

/// Delete an API key from the OS keychain for the given provider name.
fn keyring_delete(provider_name: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, provider_name) {
        let _ = entry.delete_password();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProjectEntry {
    path: PathBuf,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProjectListFile {
    #[serde(default)]
    projects: Vec<ProjectEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelProfile {
    name: String,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderConfig {
    name: String,
    protocol: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    profiles: Vec<ModelProfile>,
    #[serde(default)]
    active_profile: Option<String>,
    /// Read-only: true when an API key exists in the OS keychain for this provider.
    /// Not persisted in JSON — computed at runtime when listing configs.
    #[serde(default, skip_deserializing)]
    api_key_stored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProviderConfigList {
    #[serde(default)]
    providers: Vec<ProviderConfig>,
    #[serde(default)]
    active_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GuiSettingsFile {
    #[serde(default)]
    provider_name: Option<String>,
    #[serde(default)]
    provider_model: Option<String>,
    #[serde(default)]
    provider_base_url: Option<String>,
    #[serde(default)]
    provider_protocol: Option<String>,
    #[serde(default)]
    max_output_tokens: Option<u32>,
    #[serde(default)]
    thinking_budget: Option<Option<u32>>,
    #[serde(default)]
    max_retries: Option<u32>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    retry_initial_backoff_ms: Option<u64>,
    #[serde(default)]
    retry_max_backoff_ms: Option<u64>,
    #[serde(default)]
    respect_retry_after: Option<bool>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    verbose: Option<bool>,
    #[serde(default)]
    codex_model_provider: Option<String>,
    #[serde(default)]
    codex_approval_policy: Option<String>,
    #[serde(default)]
    codex_sandbox_mode: Option<String>,
    #[serde(default)]
    codex_persist_extended_history: Option<bool>,
    #[serde(default)]
    codex_memories_enabled: Option<bool>,
    #[serde(default)]
    codex_thread_store_endpoint: Option<String>,
    #[serde(default)]
    codex_config_overrides: BTreeMap<String, String>,
    #[serde(default)]
    codex_permission_profile: Option<serde_json::Value>,
    #[serde(default)]
    codex_service_tier: Option<String>,
    #[serde(default)]
    codex_ephemeral: Option<bool>,
}

impl Default for GuiSettingsFile {
    fn default() -> Self {
        Self {
            provider_name: None,
            provider_model: None,
            provider_base_url: None,
            provider_protocol: None,
            max_output_tokens: None,
            thinking_budget: None,
            max_retries: None,
            timeout_ms: None,
            retry_initial_backoff_ms: None,
            retry_max_backoff_ms: None,
            respect_retry_after: None,
            permission_mode: None,
            verbose: Some(false),
            codex_model_provider: None,
            codex_approval_policy: None,
            codex_sandbox_mode: None,
            codex_persist_extended_history: Some(true),
            codex_memories_enabled: Some(true),
            codex_thread_store_endpoint: None,
            codex_config_overrides: BTreeMap::new(),
            codex_permission_profile: None,
            codex_service_tier: None,
            codex_ephemeral: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ProviderInfoDto {
    name: String,
    model: Option<String>,
    protocol: String,
    base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SessionSummaryDto {
    id: String,
    title: String,
    cwd: String,
    provider_name: String,
    model: Option<String>,
    created_at: String,
    updated_at: String,
    archived: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ToolCallDto {
    id: String,
    name: String,
    input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
struct ConversationEntryDto {
    role: String,
    text: String,
    content_blocks: Vec<serde_json::Value>,
    tool_calls: Vec<ToolCallDto>,
    tool_call_id: Option<String>,
    name: Option<String>,
    is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
struct PromptResultDto {
    session_id: String,
    text: String,
    tool_calls: Vec<ToolCallDto>,
    usage: UsageDto,
    num_turns: u32,
    stop_reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct UsageDto {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
struct InitResultDto {
    provider: Option<ProviderInfoDto>,
    sessions_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct FullSettingsDto {
    provider_name: String,
    provider_model: Option<String>,
    provider_base_url: Option<String>,
    provider_protocol: String,
    provider_api_key_set: bool,
    max_output_tokens: u32,
    thinking_budget: Option<u32>,
    max_retries: u32,
    timeout_ms: u64,
    retry_initial_backoff_ms: u64,
    retry_max_backoff_ms: u64,
    respect_retry_after: bool,
    permission_mode: String,
    max_turns: usize,
    verbose: bool,
    codex_model_provider: Option<String>,
    codex_approval_policy: Option<String>,
    codex_sandbox_mode: Option<String>,
    codex_persist_extended_history: bool,
    codex_memories_enabled: bool,
    codex_thread_store_endpoint: Option<String>,
    codex_config_overrides: BTreeMap<String, String>,
    codex_permission_profile: Option<serde_json::Value>,
    codex_service_tier: Option<String>,
    codex_ephemeral: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct UpdateProviderRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    provider_name: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    provider_model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    provider_base_url: Option<String>,
    #[serde(default)]
    protocol: Option<String>,
    #[serde(default)]
    provider_protocol: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    max_output_tokens: Option<u32>,
    #[serde(default)]
    thinking_budget: Option<Option<u32>>,
    #[serde(default)]
    max_retries: Option<u32>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    retry_initial_backoff_ms: Option<u64>,
    #[serde(default)]
    retry_max_backoff_ms: Option<u64>,
    #[serde(default)]
    respect_retry_after: Option<bool>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    verbose: Option<bool>,
    #[serde(default)]
    codex_model_provider: Option<String>,
    #[serde(default)]
    codex_approval_policy: Option<String>,
    #[serde(default)]
    codex_sandbox_mode: Option<String>,
    #[serde(default)]
    codex_persist_extended_history: Option<bool>,
    #[serde(default)]
    codex_memories_enabled: Option<bool>,
    #[serde(default)]
    codex_thread_store_endpoint: Option<String>,
    #[serde(default)]
    codex_config_overrides: Option<BTreeMap<String, String>>,
    #[serde(default)]
    codex_permission_profile: Option<serde_json::Value>,
    #[serde(default)]
    codex_service_tier: Option<String>,
    #[serde(default)]
    codex_ephemeral: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectInfoDto {
    path: String,
    name: String,
    session_count: usize,
    is_auto_detected: bool,
}

#[derive(Debug, Clone, Serialize)]
struct PermissionRequestDto {
    request_id: String,
    tool_name: String,
    tool_use_id: String,
    title: String,
    description: String,
    input: serde_json::Value,
    blocked_path: Option<String>,
    permission_suggestions: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct PermissionDecisionDto {
    request_id: String,
    allowed: bool,
    message: Option<String>,
    updated_input: Option<serde_json::Value>,
    permission_updates: Vec<claude_permissions::PermissionUpdate>,
    feedback: Option<String>,
    content_blocks: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolProgressDto {
    tool_call_id: String,
    tool_name: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct ToolResultDto {
    tool_call_id: String,
    tool_name: String,
    is_error: bool,
    output: String,
}

#[derive(Debug, Clone, Serialize)]
struct StreamingDeltaDto {
    session_id: String,
    delta: String,
}

#[derive(Debug, Clone, Serialize)]
struct PromptDoneDto {
    session_id: String,
    is_error: bool,
    error: Option<String>,
    result: Option<PromptResultDto>,
}

#[derive(Debug, Clone, Serialize)]
struct SubtaskStartedDto {
    session_id: String,
    task_id: String,
    parent_task_id: Option<String>,
    description: String,
    depth: u32,
}

#[derive(Debug, Clone, Serialize)]
struct SubtaskProgressDto {
    session_id: String,
    task_id: String,
    turn: u32,
    max_turns: u32,
    summary: String,
}

#[derive(Debug, Clone, Serialize)]
struct SubtaskCompletedDto {
    session_id: String,
    task_id: String,
    success: bool,
    output_preview: String,
    turns_used: u32,
}

#[derive(Debug, Clone, Serialize)]
struct BatchProgressDto {
    session_id: String,
    total: usize,
    completed: usize,
    running: usize,
}

#[derive(Debug, Clone, Serialize)]
struct SessionTaskDto {
    session_id: String,
    task_id: String,
    parent_task_id: Option<String>,
    description: String,
    depth: u32,
    status: String,
    summary: String,
    output_preview: Option<String>,
    turns_used: Option<u32>,
    kind: String,
    output_path: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct TaskSnapshotDto {
    session_id: String,
    tasks: Vec<SessionTaskDto>,
}

#[derive(Debug, Clone, Serialize)]
struct ContextUsageDto {
    session_id: String,
    estimated_tokens: u64,
    max_input_tokens: u64,
    threshold_tokens: u64,
    ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ContextOverflowDto {
    session_id: String,
    estimated_tokens: u64,
    max_input_tokens: u64,
    threshold_tokens: u64,
    ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ContextCompactedDto {
    session_id: String,
    entries_removed: usize,
    usage_ratio: f64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentTypeInfoDto {
    agent_type: String,
    display_name: String,
    available: bool,
    installed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConfigScopeDto {
    Profile,
    Project,
}

impl ConfigScopeDto {
    fn label(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Project => "project",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SessionExportFormatDto {
    Json,
    Ndjson,
}

impl SessionExportFormatDto {
    fn label(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Ndjson => "ndjson",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SessionExportResultDto {
    session_id: String,
    format: String,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorReportDto {
    ok: bool,
    runtime: GuiDoctorRuntimeDto,
    provider: GuiDoctorProviderDto,
    tools: GuiDoctorToolsDto,
    permissions: GuiDoctorPermissionsDto,
    extensions: GuiDoctorExtensionsDto,
    mcp_runtime: GuiDoctorMcpRuntimeDto,
    network: Vec<GuiDoctorProbeDto>,
    env_providers: Vec<GuiDoctorEnvProviderDto>,
    issues: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorRuntimeDto {
    version: String,
    cwd: String,
    profile_dir: String,
    session_id: String,
    session_name: Option<String>,
    permission_mode: String,
    setting_sources: Vec<String>,
    allowed_setting_sources: Vec<String>,
    settings_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorProviderDto {
    name: String,
    protocol: String,
    base_url: Option<String>,
    model: Option<String>,
    api_key_present: bool,
    auth_source: Option<String>,
    effort: Option<String>,
    fallback_model: Option<String>,
    context_window_tokens: u64,
    output_reserve_tokens: u64,
    multimodal: bool,
    reasoning: bool,
    validation_ok: bool,
    validation_issues: Vec<String>,
    probe: Option<GuiDoctorProbeDto>,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorToolsDto {
    builtin_tools: usize,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorRuleSourceDto {
    source: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorPermissionsDto {
    layered_rules: usize,
    rule_sources: Vec<GuiDoctorRuleSourceDto>,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorExtensionsDto {
    skills: usize,
    plugins: usize,
    disabled_plugins: usize,
    managed_mcp_servers: usize,
    plugin_mcp_servers: usize,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorMcpRuntimeDto {
    probed: bool,
    summary: UiRuntimeMcpInventorySummary,
    servers: Vec<GuiDoctorMcpRuntimeServerDto>,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorMcpRuntimeServerDto {
    name: String,
    status: UiRuntimeMcpServerStatus,
    enabled: bool,
    origin_kind: String,
    origin_name: String,
    config_path: String,
    tool_count: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorEnvProviderDto {
    name: String,
    protocol: String,
    base_url: Option<String>,
    model: Option<String>,
    api_key_present: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum GuiDoctorProbeOutcomeDto {
    Reachable,
    AuthRejected,
    RateLimited,
    ServerError,
    TransportError,
}

#[derive(Debug, Clone, Serialize)]
struct GuiDoctorProbeDto {
    label: String,
    url: String,
    outcome: GuiDoctorProbeOutcomeDto,
    status_code: Option<u16>,
    latency_ms: u128,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
struct McpServerListDto {
    scope: String,
    config_path: String,
    warnings: Vec<String>,
    servers: Vec<McpServerDto>,
}

#[derive(Debug, Clone, Serialize)]
struct McpServerDto {
    name: String,
    enabled: bool,
    transport: String,
    config_path: String,
    command: Option<String>,
    url: Option<String>,
    args: Vec<String>,
    cwd: Option<String>,
    env_keys: Vec<String>,
    metadata_keys: Vec<String>,
    startup_timeout_secs: Option<u64>,
    request_timeout_secs: Option<u64>,
    live: Option<McpServerLiveDto>,
}

#[derive(Debug, Clone, Serialize)]
struct McpServerLiveDto {
    status: String,
    protocol_version: Option<String>,
    peer_name: Option<String>,
    peer_version: Option<String>,
    tool_count: usize,
    tools: Vec<McpToolInfoDto>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct McpToolInfoDto {
    name: String,
    description: Option<String>,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeMcpInventoryDto {
    effective_cwd: String,
    warnings: Vec<String>,
    summary: UiRuntimeMcpInventorySummary,
    servers: Vec<RuntimeMcpServerDto>,
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeMcpServerDto {
    name: String,
    status: String,
    enabled: bool,
    origin_kind: String,
    origin_name: String,
    config_path: String,
    transport: String,
    command: Option<String>,
    url: Option<String>,
    args: Vec<String>,
    cwd: Option<String>,
    env_keys: Vec<String>,
    metadata_keys: Vec<String>,
    startup_timeout_secs: Option<u64>,
    request_timeout_secs: Option<u64>,
    live: Option<McpServerLiveDto>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpServerUpsertRequestDto {
    scope: ConfigScopeDto,
    #[serde(default)]
    project_path: Option<String>,
    name: String,
    transport: String,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    startup_timeout_secs: Option<u64>,
    #[serde(default)]
    request_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct McpMutationResultDto {
    status: String,
    scope: String,
    config_path: String,
    name: Option<String>,
    enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadRefRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    #[serde(default = "default_true")]
    include_turns: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CodexNativeParamsRequest {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadNativeParamsRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadArchiveRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadSetNameRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadMetadataUpdateRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    #[serde(default)]
    sha: Option<Option<String>>,
    #[serde(default)]
    branch: Option<Option<String>>,
    #[serde(default)]
    origin_url: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadShellCommandRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    command: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CodexThreadLoadedListRequest {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadGoalRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadGoalSetUiRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadRollbackUiRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    num_turns: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadTurnsListRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexTurnSteerUiRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    expected_turn_id: String,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexTurnInterruptUiRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    #[serde(default)]
    turn_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexExperimentalFeatureSetRequest {
    feature: String,
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CodexSkillsListRequest {
    #[serde(default)]
    cwds: Option<Vec<String>>,
    #[serde(default)]
    force_reload: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexSkillsConfigWriteRequest {
    skill_id: String,
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CodexPluginListRequest {
    #[serde(default)]
    cwds: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexPluginIdRequest {
    plugin_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexPluginInstallRequest {
    source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexMarketplaceRequest {
    source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexMcpOAuthLoginRequest {
    #[serde(default)]
    session_id: Option<String>,
    server: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexReviewStartRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    #[serde(default)]
    prompt: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexExecWriteRequest {
    #[serde(default)]
    session_id: Option<String>,
    process_id: String,
    #[serde(default)]
    delta_base64: Option<String>,
    #[serde(default)]
    close_stdin: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexExecResizeRequest {
    #[serde(default)]
    session_id: Option<String>,
    process_id: String,
    rows: u16,
    cols: u16,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexExecAgentRequest {
    prompt: String,
    #[serde(default)]
    json: bool,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexExecAgentResponse {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    codex_home: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexMcpStatusRequest {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexMcpResourceReadRequest {
    #[serde(default)]
    session_id: Option<String>,
    server: String,
    uri: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexMcpToolCallRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    server: String,
    tool: String,
    #[serde(default)]
    arguments: Option<serde_json::Value>,
    #[serde(default)]
    meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexConfigValueWriteRequest {
    key_path: String,
    value: serde_json::Value,
    #[serde(default)]
    merge_strategy: Option<String>,
    #[serde(default)]
    file_path: Option<String>,
    #[serde(default)]
    expected_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexConfigBatchEditRequest {
    key_path: String,
    value: serde_json::Value,
    #[serde(default)]
    merge_strategy: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexConfigBatchWriteRequest {
    edits: Vec<CodexConfigBatchEditRequest>,
    #[serde(default)]
    file_path: Option<String>,
    #[serde(default)]
    expected_version: Option<String>,
    #[serde(default)]
    reload_user_config: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CodexExternalAgentConfigDetectRequest {
    #[serde(default)]
    include_home: bool,
    #[serde(default)]
    cwds: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexMemoryModeRequest {
    #[serde(default)]
    session_id: Option<String>,
    thread_id: String,
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerRequest {
    #[serde(default)]
    session_id: Option<String>,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}

struct RuntimeState {
    config: RuntimeConfig,
    provider: Arc<ProviderClient>,
    session_store: Arc<SessionStore>,
    projects: Vec<ProjectEntry>,
    provider_configs: ProviderConfigList,
    gui_settings: GuiSettingsFile,
}

struct AppState {
    runtime: Mutex<RuntimeState>,
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
    pending_codex_permissions: Arc<Mutex<HashMap<String, CodexPendingPermission>>>,
    running_prompts: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    /// Codex in-process adapters (session_id → adapter).
    active_codex_adapters: Arc<Mutex<HashMap<String, CodexInProcessAdapter>>>,
    /// Roo in-process adapters (session_id → adapter).
    active_roo_adapters: Arc<Mutex<HashMap<String, RooInProcessAdapter>>>,
}

#[derive(Debug, Clone)]
struct CodexPendingPermission {
    session_id: String,
    request_id: String,
}

fn codex_permission_decision(
    allowed: bool,
    permission_updates: &[PermissionUpdate],
) -> AgentPermissionDecision {
    if !allowed {
        return AgentPermissionDecision::Deny;
    }

    let session_scoped = permission_updates.iter().any(|update| match update {
        PermissionUpdate::AddRules { destination, .. }
        | PermissionUpdate::ReplaceRules { destination, .. }
        | PermissionUpdate::RemoveRules { destination, .. }
        | PermissionUpdate::SetMode { destination, .. }
        | PermissionUpdate::AddDirectories { destination, .. }
        | PermissionUpdate::RemoveDirectories { destination, .. } => {
            *destination == PermissionUpdateDestination::Session
        }
    });

    if session_scoped {
        AgentPermissionDecision::AllowAll
    } else {
        AgentPermissionDecision::Allow
    }
}

fn usage_info_from_codex_token_usage(
    value: &serde_json::Value,
) -> Option<claude_agent_protocol::events::UsageInfo> {
    let params = value.get("params").unwrap_or(value);
    let total = params.get("tokenUsage")?.get("total")?;
    Some(claude_agent_protocol::events::UsageInfo {
        input_tokens: total
            .get("inputTokens")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(0),
        output_tokens: total
            .get("outputTokens")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(0),
        cache_read: total
            .get("cachedInputTokens")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(0),
        cache_write: 0,
    })
}

fn gui_storage_path(paths: &AppPaths, file_name: &str) -> PathBuf {
    paths.profile_dir.join(file_name)
}

fn profile_override_from_env() -> Option<PathBuf> {
    env::var("REMOTE_CODE_PROFILE_DIR")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn load_json_file<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let parsed = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(parsed)
}

fn save_json_file<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let contents = serde_json::to_vec_pretty(value)?;
    std::fs::write(path, contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn load_projects(paths: &AppPaths) -> Result<Vec<ProjectEntry>> {
    let file: ProjectListFile = load_json_file(&gui_storage_path(paths, PROJECTS_FILE_NAME))?;
    Ok(normalize_project_entries(file.projects))
}

fn save_projects(paths: &AppPaths, projects: &[ProjectEntry]) -> Result<()> {
    let file = ProjectListFile {
        projects: normalize_project_entries(projects.to_vec()),
    };
    save_json_file(&gui_storage_path(paths, PROJECTS_FILE_NAME), &file)
}

fn load_provider_configs(paths: &AppPaths) -> Result<ProviderConfigList> {
    load_json_file(&gui_storage_path(paths, PROVIDERS_FILE_NAME))
}

fn save_provider_configs(paths: &AppPaths, configs: &ProviderConfigList) -> Result<()> {
    save_json_file(&gui_storage_path(paths, PROVIDERS_FILE_NAME), configs)
}

fn load_gui_settings(paths: &AppPaths) -> Result<GuiSettingsFile> {
    load_json_file(&gui_storage_path(paths, SETTINGS_FILE_NAME))
}

fn save_gui_settings(paths: &AppPaths, settings: &GuiSettingsFile) -> Result<()> {
    save_json_file(&gui_storage_path(paths, SETTINGS_FILE_NAME), settings)
}

fn parse_protocol(value: Option<&str>) -> Option<ProviderProtocol> {
    let value = value?.trim().to_ascii_lowercase();
    match value.as_str() {
        "openai" | "open-ai" | "open_ai" => Some(ProviderProtocol::OpenAi),
        "anthropic" | "claude" => Some(ProviderProtocol::Anthropic),
        "bedrock" | "aws" | "amazon" => Some(ProviderProtocol::Bedrock),
        "vertex" | "google" | "gemini" => Some(ProviderProtocol::Vertex),
        _ => None,
    }
}

fn parse_permission_mode(value: Option<&str>) -> Option<PermissionMode> {
    let value = value?.trim().to_ascii_lowercase();
    match value.as_str() {
        "default" | "suggest" => Some(PermissionMode::Default),
        "acceptedits" | "accept-edits" | "accept_edits" | "auto-edit" | "auto_edit" => {
            Some(PermissionMode::AcceptEdits)
        }
        "dontask" | "dont-ask" | "dont_ask" => Some(PermissionMode::DontAsk),
        "bypasspermissions" | "bypass-permissions" | "bypass_permissions" | "full-auto"
        | "full_auto" | "yolo" => Some(PermissionMode::BypassPermissions),
        "plan" => Some(PermissionMode::Plan),
        _ => None,
    }
}

fn normalize_provider_config(input: ProviderConfig) -> Result<ProviderConfig> {
    let name = input.name.trim().to_owned();
    if name.is_empty() {
        return Err(anyhow!("provider name cannot be empty"));
    }
    let protocol = parse_protocol(Some(&input.protocol)).unwrap_or(ProviderProtocol::OpenAi);
    let base_url = normalize_base_url(trimmed_option(input.base_url), protocol);
    let api_key = trimmed_option(input.api_key);
    let model = trimmed_option(input.model);
    Ok(ProviderConfig {
        name,
        protocol: protocol.as_str().to_owned(),
        base_url,
        api_key,
        model,
        profiles: input.profiles,
        active_profile: input.active_profile,
        api_key_stored: false,
    })
}

fn trimmed_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn strip_windows_unc_prefix(path: PathBuf) -> PathBuf {
    let raw = path.to_string_lossy();
    if cfg!(windows) && raw.starts_with(r"\\?\") {
        PathBuf::from(raw.trim_start_matches(r"\\?\"))
    } else {
        path
    }
}

fn normalize_existing_path(path: &Path) -> Result<PathBuf> {
    let path = if path.exists() {
        std::fs::canonicalize(path)?
    } else {
        path.to_path_buf()
    };
    Ok(strip_windows_unc_prefix(path))
}

fn path_identity(path: &Path) -> String {
    let raw = if cfg!(windows) {
        path.to_string_lossy().replace('/', "\\")
    } else {
        path.to_string_lossy().into_owned()
    };
    let separator = if cfg!(windows) { '\\' } else { '/' };
    let normalized = raw.trim_end_matches(separator).to_owned();
    if cfg!(windows) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

fn normalize_project_entries(projects: Vec<ProjectEntry>) -> Vec<ProjectEntry> {
    let mut deduped = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for project in projects {
        let normalized_path =
            normalize_existing_path(&project.path).unwrap_or_else(|_| project.path.clone());
        let key = path_identity(&normalized_path);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        let fallback_name = normalized_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project")
            .to_owned();
        let name = project.name.trim();
        deduped.push(ProjectEntry {
            path: normalized_path,
            name: if name.is_empty() {
                fallback_name
            } else {
                name.to_owned()
            },
        });
    }

    deduped.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    deduped
}

fn project_entry_from_path(path: &Path) -> ProjectEntry {
    let normalized_path = normalize_existing_path(path).unwrap_or_else(|_| path.to_path_buf());
    let fallback_name = normalized_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project")
        .to_owned();
    ProjectEntry {
        path: normalized_path,
        name: fallback_name,
    }
}

fn ensure_sessions_have_projects(
    projects: &mut Vec<ProjectEntry>,
    sessions: &[SessionSummary],
) -> bool {
    let mut merged = projects.clone();
    let mut seen = merged
        .iter()
        .map(|project| path_identity(&project.path))
        .collect::<std::collections::HashSet<_>>();
    let mut changed = false;

    for session in sessions {
        let normalized_path =
            normalize_existing_path(&session.cwd).unwrap_or_else(|_| session.cwd.clone());
        let key = path_identity(&normalized_path);
        if seen.insert(key) {
            merged.push(project_entry_from_path(&normalized_path));
            changed = true;
        }
    }

    if changed {
        *projects = normalize_project_entries(merged);
    }

    changed
}

fn ensure_project_entry(projects: &mut Vec<ProjectEntry>, path: &Path) -> bool {
    let normalized_path = normalize_existing_path(path).unwrap_or_else(|_| path.to_path_buf());
    let key = path_identity(&normalized_path);
    if projects
        .iter()
        .any(|project| path_identity(&project.path) == key)
    {
        return false;
    }
    let mut merged = projects.clone();
    merged.push(project_entry_from_path(&normalized_path));
    *projects = normalize_project_entries(merged);
    true
}

fn project_session_count(project_path: &Path, sessions: &[SessionSummary]) -> usize {
    let key = path_identity(project_path);
    sessions
        .iter()
        .filter(|summary| path_identity(&summary.cwd) == key)
        .count()
}

fn provider_info_from_runtime(provider: &RuntimeProviderConfig) -> ProviderInfoDto {
    ProviderInfoDto {
        name: provider.name.clone(),
        model: provider.model.clone(),
        protocol: provider.protocol.as_str().to_owned(),
        base_url: provider.base_url.clone(),
    }
}

fn runtime_status_snapshot_from_config(config: &RuntimeConfig) -> UiRuntimeStatusSnapshot {
    UiRuntimeStatusSnapshot {
        session_name: config.session_name.clone(),
        provider: UiProviderStatusSnapshot {
            name: config.provider.name.clone(),
            model: config.provider.model.clone(),
            protocol: config.provider.protocol.as_str().to_owned(),
            base_url: config.provider.base_url.clone(),
            auth_source: config.auth_source.clone(),
            effort: config.effort.clone(),
            fallback_model: config.fallback_model.clone(),
        },
        permission_mode: config.permission_mode.as_legacy_str().to_owned(),
        output_style: config.output_style.clone(),
        language: config.language.clone(),
        brief_enabled: config.brief_enabled,
        proactive_active: config.proactive_active,
        setting_sources: config.setting_sources.clone(),
        allowed_setting_sources: config
            .allowed_setting_sources
            .iter()
            .map(|source| source.as_str().to_owned())
            .collect(),
        allowed_tools: config.allowed_tools.clone(),
        disallowed_tools: config.disallowed_tools.clone(),
        mcp: runtime_mcp_inventory_summary(config, &[]),
    }
}

fn emit_runtime_status(app: &AppHandle, config: &RuntimeConfig) {
    let _ = app.emit(
        APP_EVENT_RUNTIME_STATUS,
        runtime_status_snapshot_from_config(config),
    );
}

fn repository_slug() -> Option<String> {
    let repository = env!("CARGO_PKG_REPOSITORY").trim();
    let repository = repository
        .strip_suffix(".git")
        .unwrap_or(repository)
        .trim_end_matches('/');
    if let Some(stripped) = repository.strip_prefix("https://github.com/") {
        return Some(stripped.to_owned());
    }
    if let Some(stripped) = repository.strip_prefix("http://github.com/") {
        return Some(stripped.to_owned());
    }
    repository
        .strip_prefix("git@github.com:")
        .map(ToOwned::to_owned)
}

fn provider_endpoint_url(provider: &RuntimeProviderConfig) -> Option<String> {
    provider
        .base_url
        .clone()
        .or_else(|| match provider.protocol {
            ProviderProtocol::Anthropic => Some("https://api.anthropic.com/v1/messages".to_owned()),
            ProviderProtocol::OpenAi => {
                Some("https://api.openai.com/v1/chat/completions".to_owned())
            }
            ProviderProtocol::Bedrock | ProviderProtocol::Vertex => None,
        })
}

fn classify_probe_status(status: reqwest::StatusCode) -> (GuiDoctorProbeOutcomeDto, String) {
    let code = status.as_u16();
    if status.is_success() {
        return (
            GuiDoctorProbeOutcomeDto::Reachable,
            format!("HTTP {code} confirms the endpoint is reachable"),
        );
    }

    match code {
        400 | 404 | 405 | 406 | 409 | 415 | 422 => (
            GuiDoctorProbeOutcomeDto::Reachable,
            format!("HTTP {code} confirms the endpoint is reachable"),
        ),
        401 | 403 => (
            GuiDoctorProbeOutcomeDto::AuthRejected,
            format!("HTTP {code} indicates the endpoint rejected the supplied credentials"),
        ),
        429 => (
            GuiDoctorProbeOutcomeDto::RateLimited,
            "HTTP 429 indicates the endpoint is reachable but currently rate limited".to_owned(),
        ),
        500..=599 => (
            GuiDoctorProbeOutcomeDto::ServerError,
            format!("HTTP {code} indicates an upstream server failure"),
        ),
        _ => (
            GuiDoctorProbeOutcomeDto::Reachable,
            format!("HTTP {code} returned from the endpoint"),
        ),
    }
}

fn probe_is_issue(probe: &GuiDoctorProbeDto) -> bool {
    matches!(
        probe.outcome,
        GuiDoctorProbeOutcomeDto::AuthRejected
            | GuiDoctorProbeOutcomeDto::ServerError
            | GuiDoctorProbeOutcomeDto::TransportError
    )
}

fn probe_is_warning(probe: &GuiDoctorProbeDto) -> bool {
    matches!(probe.outcome, GuiDoctorProbeOutcomeDto::RateLimited)
}

async fn run_doctor_probe(
    label: impl Into<String>,
    url: impl Into<String>,
    headers: &BTreeMap<String, String>,
) -> GuiDoctorProbeDto {
    let label = label.into();
    let url = url.into();
    let client = match reqwest::Client::builder()
        .user_agent("remote-code-gui-doctor")
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return GuiDoctorProbeDto {
                label,
                url,
                outcome: GuiDoctorProbeOutcomeDto::TransportError,
                status_code: None,
                latency_ms: 0,
                detail: format!("failed to build HTTP client: {error}"),
            };
        }
    };

    let started = Instant::now();
    let mut request = client.get(&url);
    for (name, value) in headers {
        request = request.header(name, value);
    }

    match request.send().await {
        Ok(response) => {
            let (outcome, detail) = classify_probe_status(response.status());
            GuiDoctorProbeDto {
                label,
                url,
                outcome,
                status_code: Some(response.status().as_u16()),
                latency_ms: started.elapsed().as_millis(),
                detail,
            }
        }
        Err(error) => GuiDoctorProbeDto {
            label,
            url,
            outcome: GuiDoctorProbeOutcomeDto::TransportError,
            status_code: None,
            latency_ms: started.elapsed().as_millis(),
            detail: error.to_string(),
        },
    }
}

fn count_managed_mcp_servers(path: &Path, warnings: &mut Vec<String>) -> usize {
    if !path.exists() {
        return 0;
    }
    match McpConfig::load(path) {
        Ok(config) => config.servers.len(),
        Err(error) => {
            warnings.push(format!(
                "Failed to load MCP config {}: {error}",
                path.display()
            ));
            0
        }
    }
}

fn count_plugin_mcp_servers(plugins: &[PluginBundle], warnings: &mut Vec<String>) -> usize {
    let mut count = 0usize;
    for plugin in plugins {
        let Some(path) = plugin.mcp_config_path() else {
            continue;
        };
        match McpConfig::load(&path) {
            Ok(config) => count += config.servers.len(),
            Err(error) => warnings.push(format!(
                "Failed to load plugin MCP config for {}: {error}",
                plugin.manifest.name
            )),
        }
    }
    count
}

async fn build_gui_doctor_report(
    config: &RuntimeConfig,
    probe_network: bool,
    probe_provider: bool,
    probe_mcp: bool,
    include_env_providers: bool,
) -> Result<GuiDoctorReportDto> {
    let validation = validate_provider_config(&config.provider);
    let model_info = get_model_info(config.provider.model.as_deref().unwrap_or("unknown"));
    let layered_rules = load_layered_rules(
        &config.cwd,
        &config.paths.profile_dir,
        &config.settings_files,
        &config.cli_settings_files,
    );
    let mcp_runtime = observe_runtime_mcp_servers(
        config,
        &[],
        probe_mcp,
        &McpClientInfo::new("remote-code-gui", env!("CARGO_PKG_VERSION")),
    )
    .await;

    let mut warnings = Vec::new();
    let mut issues = validation.issues.clone();
    let user_sources_enabled = setting_source_enabled(config, SettingSource::User);
    let project_sources_enabled = setting_source_enabled(config, SettingSource::Project);

    let skills = if user_sources_enabled {
        match discover_skills(&config.paths.skills_dir) {
            Ok(skills) => skills,
            Err(error) => {
                warnings.push(format!("Failed to discover skills: {error}"));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let all_plugins = if user_sources_enabled {
        match discover_plugins_including_disabled(&config.paths.plugins_dir) {
            Ok(plugins) => plugins,
            Err(error) => {
                warnings.push(format!("Failed to discover plugins: {error}"));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let disabled_plugins = all_plugins
        .iter()
        .filter(|plugin| plugin.is_disabled())
        .count();
    let plugins = all_plugins
        .into_iter()
        .filter(|plugin| !plugin.is_disabled())
        .collect::<Vec<_>>();
    let managed_mcp_servers = if user_sources_enabled {
        count_managed_mcp_servers(
            &config.paths.profile_dir.join(DEFAULT_MCP_CONFIG_FILE),
            &mut warnings,
        )
    } else {
        0
    } + if project_sources_enabled {
        count_managed_mcp_servers(&config.cwd.join(DEFAULT_MCP_CONFIG_FILE), &mut warnings)
    } else {
        0
    };
    let plugin_mcp_servers = if user_sources_enabled {
        count_plugin_mcp_servers(&plugins, &mut warnings)
    } else {
        0
    };
    extend_unique_strings(&mut warnings, mcp_runtime.warnings.clone());

    let provider_probe = if probe_provider {
        if let Some(url) = provider_endpoint_url(&config.provider) {
            let mut headers = config.provider.request_header_overrides.clone();
            match config.provider.protocol {
                ProviderProtocol::Anthropic => {
                    headers.insert("anthropic-version".to_owned(), "2023-06-01".to_owned());
                    if let Some(api_key) = &config.provider.api_key {
                        headers.insert("x-api-key".to_owned(), api_key.clone());
                    }
                }
                ProviderProtocol::OpenAi => {
                    if let Some(api_key) = &config.provider.api_key {
                        headers.insert("authorization".to_owned(), format!("Bearer {api_key}"));
                    }
                }
                ProviderProtocol::Bedrock | ProviderProtocol::Vertex => {}
            }
            let probe =
                run_doctor_probe(format!("provider:{}", config.provider.name), url, &headers).await;
            if probe_is_issue(&probe) {
                issues.push(format!("Provider probe failed: {}", probe.detail));
            } else if probe_is_warning(&probe) {
                warnings.push(format!("Provider probe warning: {}", probe.detail));
            }
            Some(probe)
        } else {
            warnings.push(
                "Provider probe skipped: no probeable endpoint for the active protocol.".to_owned(),
            );
            None
        }
    } else {
        None
    };

    let mut network = Vec::new();
    if probe_network {
        if let Some(slug) = repository_slug() {
            let github_probe = run_doctor_probe(
                "github:releases",
                format!("https://api.github.com/repos/{slug}/releases/latest"),
                &BTreeMap::new(),
            )
            .await;
            if probe_is_warning(&github_probe) || probe_is_issue(&github_probe) {
                warnings.push(format!("Network probe warning: {}", github_probe.detail));
            }
            network.push(github_probe);
        }
        if !probe_provider {
            if let Some(url) = provider_endpoint_url(&config.provider) {
                let provider_network_probe =
                    run_doctor_probe("provider:network", url, &BTreeMap::new()).await;
                if probe_is_warning(&provider_network_probe)
                    || probe_is_issue(&provider_network_probe)
                {
                    warnings.push(format!(
                        "Network probe warning: {}",
                        provider_network_probe.detail
                    ));
                }
                network.push(provider_network_probe);
            }
        }
    }

    let env_providers = if include_env_providers {
        discover_env_providers()
            .into_iter()
            .map(|provider| GuiDoctorEnvProviderDto {
                name: provider.name,
                protocol: provider.protocol.as_str().to_owned(),
                base_url: provider.base_url,
                model: provider.model,
                api_key_present: provider.api_key.is_some(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let mcp_runtime = GuiDoctorMcpRuntimeDto {
        probed: probe_mcp,
        summary: mcp_runtime.inventory_summary(),
        servers: mcp_runtime
            .servers
            .into_iter()
            .map(|server| GuiDoctorMcpRuntimeServerDto {
                name: server.entry.server.name,
                status: server.status,
                enabled: server.entry.server.enabled,
                origin_kind: server.entry.origin_kind.to_owned(),
                origin_name: server.entry.origin_name,
                config_path: server.entry.config_path.display().to_string(),
                tool_count: server
                    .inspection
                    .as_ref()
                    .map_or(0, |inspection| inspection.tools.len()),
                error: server.error,
            })
            .collect(),
    };

    Ok(GuiDoctorReportDto {
        ok: issues.is_empty(),
        runtime: GuiDoctorRuntimeDto {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            cwd: config.cwd.display().to_string(),
            profile_dir: config.paths.profile_dir.display().to_string(),
            session_id: config.session_id.to_string(),
            session_name: config.session_name.clone(),
            permission_mode: config.permission_mode.as_legacy_str().to_owned(),
            setting_sources: config.setting_sources.clone(),
            allowed_setting_sources: config
                .allowed_setting_sources
                .iter()
                .map(|source| source.as_str().to_owned())
                .collect(),
            settings_files: config
                .settings_files
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
        },
        provider: GuiDoctorProviderDto {
            name: config.provider.name.clone(),
            protocol: config.provider.protocol.as_str().to_owned(),
            base_url: config.provider.base_url.clone(),
            model: config.provider.model.clone(),
            api_key_present: config.provider.api_key.is_some(),
            auth_source: config.auth_source.clone(),
            effort: config.effort.clone(),
            fallback_model: config.fallback_model.clone(),
            context_window_tokens: model_info.max_context,
            output_reserve_tokens: model_info.max_output,
            multimodal: model_info.multimodal,
            reasoning: model_info
                .capabilities
                .contains(&ModelCapability::Reasoning),
            validation_ok: validation.ok,
            validation_issues: validation.issues,
            probe: provider_probe,
        },
        tools: GuiDoctorToolsDto {
            builtin_tools: claude_tools::runtime_builtin_tool_specs().len(),
            allowed_tools: config.allowed_tools.clone(),
            disallowed_tools: config.disallowed_tools.clone(),
        },
        permissions: GuiDoctorPermissionsDto {
            layered_rules: layered_rules.len(),
            rule_sources: summarize_rule_sources(&layered_rules)
                .into_iter()
                .map(|(source, count)| GuiDoctorRuleSourceDto {
                    source: source.as_str().to_owned(),
                    count,
                })
                .collect(),
        },
        extensions: GuiDoctorExtensionsDto {
            skills: skills.len(),
            plugins: plugins.len(),
            disabled_plugins,
            managed_mcp_servers,
            plugin_mcp_servers,
        },
        mcp_runtime,
        network,
        env_providers,
        issues,
        warnings,
    })
}

fn extend_unique_strings(target: &mut Vec<String>, items: Vec<String>) {
    for item in items {
        if !target.contains(&item) {
            target.push(item);
        }
    }
}

fn mcp_config_path_for_scope(
    config: &RuntimeConfig,
    scope: ConfigScopeDto,
    project_path: Option<&str>,
) -> Result<PathBuf> {
    match scope {
        ConfigScopeDto::Profile => Ok(config.paths.profile_dir.join(DEFAULT_MCP_CONFIG_FILE)),
        ConfigScopeDto::Project => {
            let project_path = project_path.ok_or_else(|| {
                anyhow!("project path is required for project-scope MCP management")
            })?;
            let project_root = normalize_existing_path(Path::new(project_path))?;
            Ok(project_root.join(DEFAULT_MCP_CONFIG_FILE))
        }
    }
}

fn setting_source_enabled(config: &RuntimeConfig, source: SettingSource) -> bool {
    config.allowed_setting_sources.contains(&source)
}

fn mcp_scope_enabled(config: &RuntimeConfig, scope: ConfigScopeDto) -> bool {
    match scope {
        ConfigScopeDto::Profile => setting_source_enabled(config, SettingSource::User),
        ConfigScopeDto::Project => setting_source_enabled(config, SettingSource::Project),
    }
}

fn load_managed_mcp_config_or_default(path: &Path) -> Result<McpConfig> {
    if path.exists() {
        Ok(McpConfig::load(path)?)
    } else {
        Ok(McpConfig::default())
    }
}

fn mcp_live_to_dto(inspection: McpServerInspection) -> McpServerLiveDto {
    McpServerLiveDto {
        status: UiRuntimeMcpServerStatus::Connected.as_str().to_owned(),
        protocol_version: Some(inspection.protocol_version),
        peer_name: inspection
            .server_info
            .as_ref()
            .map(|info| info.name.clone()),
        peer_version: inspection
            .server_info
            .as_ref()
            .and_then(|info| info.version.clone()),
        tool_count: inspection.tools.len(),
        tools: inspection
            .tools
            .into_iter()
            .map(|tool| McpToolInfoDto {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
            })
            .collect(),
        error: None,
    }
}

fn runtime_mcp_failed_live_to_dto(
    status: UiRuntimeMcpServerStatus,
    error: String,
) -> McpServerLiveDto {
    McpServerLiveDto {
        status: status.as_str().to_owned(),
        protocol_version: None,
        peer_name: None,
        peer_version: None,
        tool_count: 0,
        tools: Vec::new(),
        error: Some(error),
    }
}

fn mcp_transport_to_display(transport: McpTransport) -> String {
    match transport {
        McpTransport::Stdio => "stdio",
        McpTransport::Http => "http",
        McpTransport::WebSocket => "websocket",
    }
    .to_owned()
}

/// Transport fields extracted from an MCP server config for display.
struct McpTransportFields {
    command: Option<String>,
    url: Option<String>,
    args: Vec<String>,
    cwd: Option<String>,
    env_keys: Vec<String>,
}

fn mcp_server_transport_fields(server: &McpServerConfig) -> McpTransportFields {
    match &server.transport {
        McpTransportConfig::Stdio {
            command,
            args,
            cwd,
            env,
        } => {
            let mut env_keys = env.keys().cloned().collect::<Vec<_>>();
            env_keys.sort();
            McpTransportFields {
                command: Some(command.clone()),
                url: None,
                args: args.clone(),
                cwd: cwd.as_ref().map(|path| path.display().to_string()),
                env_keys,
            }
        }
        McpTransportConfig::Http { url, headers }
        | McpTransportConfig::WebSocket { url, headers } => {
            let mut env_keys = headers.keys().cloned().collect::<Vec<_>>();
            env_keys.sort();
            McpTransportFields {
                command: None,
                url: Some(url.clone()),
                args: Vec::new(),
                cwd: None,
                env_keys,
            }
        }
    }
}

fn mcp_server_to_dto(
    config_path: &Path,
    server: McpServerConfig,
    live: Option<McpServerLiveDto>,
) -> McpServerDto {
    let fields = mcp_server_transport_fields(&server);
    let mut metadata_keys = server.metadata.keys().cloned().collect::<Vec<_>>();
    metadata_keys.sort();
    McpServerDto {
        name: server.name,
        enabled: server.enabled,
        transport: mcp_transport_to_display(server.transport.kind()),
        config_path: config_path.display().to_string(),
        command: fields.command,
        url: fields.url,
        args: fields.args,
        cwd: fields.cwd,
        env_keys: fields.env_keys,
        metadata_keys,
        startup_timeout_secs: server.startup_timeout_secs,
        request_timeout_secs: server.request_timeout_secs,
        live,
    }
}

fn runtime_mcp_server_to_dto(observation: RuntimeMcpServerObservation) -> RuntimeMcpServerDto {
    let entry = observation.entry;
    let fields = mcp_server_transport_fields(&entry.server);
    let mut metadata_keys = entry.server.metadata.keys().cloned().collect::<Vec<_>>();
    metadata_keys.sort();
    let live = match (observation.inspection, observation.error) {
        (Some(inspection), _) => Some(mcp_live_to_dto(inspection)),
        (None, Some(error)) => Some(runtime_mcp_failed_live_to_dto(observation.status, error)),
        (None, None) => None,
    };
    RuntimeMcpServerDto {
        name: entry.server.name.clone(),
        status: observation.status.as_str().to_owned(),
        enabled: entry.server.enabled,
        origin_kind: entry.origin_kind.to_owned(),
        origin_name: entry.origin_name,
        config_path: entry.config_path.display().to_string(),
        transport: mcp_transport_to_display(entry.server.transport.kind()),
        command: fields.command,
        url: fields.url,
        args: fields.args,
        cwd: fields.cwd,
        env_keys: fields.env_keys,
        metadata_keys,
        startup_timeout_secs: entry.server.startup_timeout_secs,
        request_timeout_secs: entry.server.request_timeout_secs,
        live,
    }
}

async fn build_mcp_server_list(
    config: &RuntimeConfig,
    scope: ConfigScopeDto,
    project_path: Option<&str>,
    connect: bool,
    include_disabled: bool,
) -> Result<McpServerListDto> {
    let config_path = mcp_config_path_for_scope(config, scope, project_path)?;
    if !mcp_scope_enabled(config, scope) {
        return Ok(McpServerListDto {
            scope: scope.label().to_owned(),
            config_path: config_path.display().to_string(),
            warnings: vec![format!(
                "{} MCP discovery is disabled by active setting sources",
                scope.label()
            )],
            servers: Vec::new(),
        });
    }
    let mcp_config = load_managed_mcp_config_or_default(&config_path)?;
    let mut warnings = Vec::new();
    let mut servers = Vec::new();

    for server in mcp_config.servers.into_values() {
        if !server.enabled && !include_disabled {
            continue;
        }
        let live = if connect {
            match inspect_server(
                &server,
                &McpClientInfo::new("remote-code-gui", env!("CARGO_PKG_VERSION")),
            )
            .await
            {
                Ok(inspection) => Some(mcp_live_to_dto(inspection)),
                Err(error) => Some(McpServerLiveDto {
                    status: UiRuntimeMcpServerStatus::Failed.as_str().to_owned(),
                    protocol_version: None,
                    peer_name: None,
                    peer_version: None,
                    tool_count: 0,
                    tools: Vec::new(),
                    error: Some(error.to_string()),
                }),
            }
        } else {
            None
        };
        servers.push(mcp_server_to_dto(&config_path, server, live));
    }

    servers.sort_by(|left, right| left.name.cmp(&right.name));
    if !config_path.exists() {
        warnings.push(format!(
            "Managed MCP config does not exist yet at {}",
            config_path.display()
        ));
    }

    Ok(McpServerListDto {
        scope: scope.label().to_owned(),
        config_path: config_path.display().to_string(),
        warnings,
        servers,
    })
}

async fn build_runtime_mcp_inventory(
    config: &RuntimeConfig,
    project_path: Option<&str>,
    connect: bool,
    include_disabled: bool,
) -> Result<RuntimeMcpInventoryDto> {
    let mut effective_config = config.clone();
    if let Some(project_path) = project_path.filter(|value| !value.trim().is_empty()) {
        effective_config.cwd = normalize_existing_path(Path::new(project_path))?;
    }

    let observation = observe_runtime_mcp_servers(
        &effective_config,
        &[],
        connect,
        &McpClientInfo::new("remote-code-gui", env!("CARGO_PKG_VERSION")),
    )
    .await;
    let summary = observation.inventory_summary();
    let mut servers = Vec::new();
    for server in observation.servers {
        if !server.entry.server.enabled && !include_disabled {
            continue;
        }
        servers.push(runtime_mcp_server_to_dto(server));
    }

    servers.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.origin_kind.cmp(&right.origin_kind))
            .then_with(|| left.origin_name.cmp(&right.origin_name))
            .then_with(|| left.config_path.cmp(&right.config_path))
    });

    Ok(RuntimeMcpInventoryDto {
        effective_cwd: effective_config.cwd.display().to_string(),
        warnings: observation.warnings,
        summary,
        servers,
    })
}

fn build_mcp_transport(request: &McpServerUpsertRequestDto) -> Result<McpTransportConfig> {
    match request.transport.trim().to_ascii_lowercase().as_str() {
        "stdio" => {
            let command = request
                .command
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("stdio MCP servers require a command"))?;
            Ok(McpTransportConfig::Stdio {
                command: command.to_owned(),
                args: request.args.clone(),
                cwd: request.cwd.as_deref().map(PathBuf::from),
                env: request.env.clone(),
            })
        }
        "http" => {
            let url = request
                .url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("http MCP servers require a url"))?;
            Ok(McpTransportConfig::Http {
                url: url.to_owned(),
                headers: request.headers.clone(),
            })
        }
        "websocket" | "ws" => {
            let url = request
                .url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("websocket MCP servers require a url"))?;
            Ok(McpTransportConfig::WebSocket {
                url: url.to_owned(),
                headers: request.headers.clone(),
            })
        }
        other => Err(anyhow!("unsupported MCP transport: {other}")),
    }
}

fn save_managed_mcp_server_at_path(
    config_path: &Path,
    scope: ConfigScopeDto,
    request: &McpServerUpsertRequestDto,
) -> Result<McpMutationResultDto> {
    let mut mcp_config = load_managed_mcp_config_or_default(config_path)?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err(anyhow!("MCP server name cannot be empty."));
    }
    let existed = mcp_config.servers.contains_key(name);
    let transport = build_mcp_transport(request)?;
    mcp_config.servers.insert(
        name.to_owned(),
        McpServerConfig {
            name: name.to_owned(),
            enabled: !request.disabled,
            transport,
            capabilities: claude_mcp::McpCapabilityMatrix::default(),
            startup_timeout_secs: request.startup_timeout_secs,
            request_timeout_secs: request.request_timeout_secs,
            metadata: request.metadata.clone(),
            oauth: None,
        },
    );
    mcp_config.save(config_path)?;
    Ok(McpMutationResultDto {
        status: if existed { "updated" } else { "created" }.to_owned(),
        scope: scope.label().to_owned(),
        config_path: config_path.display().to_string(),
        name: Some(name.to_owned()),
        enabled: Some(!request.disabled),
    })
}

fn toggle_managed_mcp_server_at_path(
    config_path: &Path,
    scope: ConfigScopeDto,
    name: &str,
    enabled: bool,
    if_exists: bool,
) -> Result<McpMutationResultDto> {
    let mut mcp_config = load_managed_mcp_config_or_default(config_path)?;
    let name = name.trim().to_owned();
    let Some(server) = mcp_config.servers.get_mut(&name) else {
        if if_exists {
            return Ok(McpMutationResultDto {
                status: "noop".to_owned(),
                scope: scope.label().to_owned(),
                config_path: config_path.display().to_string(),
                name: Some(name),
                enabled: Some(enabled),
            });
        }
        return Err(anyhow!(
            "No MCP server named `{}` exists in {}",
            name,
            config_path.display()
        ));
    };

    let status = if server.enabled == enabled {
        "noop"
    } else {
        server.enabled = enabled;
        mcp_config.save(config_path)?;
        if enabled { "enabled" } else { "disabled" }
    };

    Ok(McpMutationResultDto {
        status: status.to_owned(),
        scope: scope.label().to_owned(),
        config_path: config_path.display().to_string(),
        name: Some(name),
        enabled: Some(enabled),
    })
}

fn remove_managed_mcp_server_at_path(
    config_path: &Path,
    scope: ConfigScopeDto,
    name: &str,
    if_exists: bool,
) -> Result<McpMutationResultDto> {
    let mut mcp_config = load_managed_mcp_config_or_default(config_path)?;
    let name = name.trim().to_owned();
    let removed = mcp_config.servers.remove(&name);
    if removed.is_none() && !if_exists {
        return Err(anyhow!(
            "No MCP server named `{}` exists in {}",
            name,
            config_path.display()
        ));
    }
    mcp_config.save(config_path)?;
    Ok(McpMutationResultDto {
        status: if removed.is_some() { "removed" } else { "noop" }.to_owned(),
        scope: scope.label().to_owned(),
        config_path: config_path.display().to_string(),
        name: Some(name),
        enabled: None,
    })
}

fn reset_managed_mcp_config_at_path(
    config_path: &Path,
    scope: ConfigScopeDto,
    if_exists: bool,
) -> Result<McpMutationResultDto> {
    let status = if config_path.exists() {
        std::fs::remove_file(config_path)?;
        "reset"
    } else if if_exists {
        "noop"
    } else {
        return Err(anyhow!(
            "Managed MCP config {} does not exist",
            config_path.display()
        ));
    };

    Ok(McpMutationResultDto {
        status: status.to_owned(),
        scope: scope.label().to_owned(),
        config_path: config_path.display().to_string(),
        name: None,
        enabled: None,
    })
}

fn export_session_bundle_for_store(
    store: &SessionStore,
    session_id: Uuid,
    format: SessionExportFormatDto,
) -> Result<SessionExportResultDto> {
    let path = match format {
        SessionExportFormatDto::Json => store.export_session_bundle_json(session_id, None),
        SessionExportFormatDto::Ndjson => store.export_session(session_id, None),
    }?;

    Ok(SessionExportResultDto {
        session_id: session_id.to_string(),
        format: format.label().to_owned(),
        path: path.display().to_string(),
    })
}

fn usage_to_dto(usage: &UsageSummary) -> UsageDto {
    UsageDto {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        total_tokens: usage.input_tokens + usage.output_tokens,
    }
}

fn task_dir_for_paths(paths: &AppPaths, session_id: Uuid) -> PathBuf {
    paths
        .artifacts_dir
        .join("tasks")
        .join(session_id.to_string())
}

fn shell_output_dir_for_paths(paths: &AppPaths, session_id: Uuid) -> PathBuf {
    paths
        .artifacts_dir
        .join("shell")
        .join(session_id.to_string())
}

fn configure_runtime_policy_for_config(config: &RuntimeConfig) -> Result<()> {
    let session_dir = config
        .paths
        .sessions_dir
        .join(config.session_id.to_string());
    configure_tool_runtime_policy(ToolRuntimePolicy {
        allowed_tools: config.allowed_tools.clone(),
        disallowed_tools: config.disallowed_tools.clone(),
        task_output_dir: Some(task_dir_for_paths(&config.paths, config.session_id)),
        tasks_dir: Some(claude_tools::tasks::task_list_base_dir()),
        tool_results_dir: Some(session_dir.join("tool-results")),
        mcp_servers: runtime_mcp_policy_entries(config, &[]),
        shell_policy: ShellExecutionPolicy {
            block_inline_cwd: true,
            allow_background: true,
            block_destructive_git: true,
            max_capture_chars: ShellExecutionPolicy::default().max_capture_chars,
            output_dir: Some(shell_output_dir_for_paths(&config.paths, config.session_id)),
            tool_results_dir: Some(session_dir.join("tool-results")),
            task_output_dir: Some(task_dir_for_paths(&config.paths, config.session_id)),
        },
    })
}

fn ui_task_node_to_dto(session_id: &str, task: claude_ui_bridge::UiTaskNode) -> SessionTaskDto {
    SessionTaskDto {
        session_id: session_id.to_owned(),
        task_id: task.id,
        parent_task_id: task.parent_task_id,
        description: task.title,
        depth: task.depth,
        status: match task.status {
            claude_ui_bridge::UiTaskStatus::Pending => "pending",
            claude_ui_bridge::UiTaskStatus::Running => "running",
            claude_ui_bridge::UiTaskStatus::Completed => "completed",
            claude_ui_bridge::UiTaskStatus::Failed => "failed",
            claude_ui_bridge::UiTaskStatus::Stopped => "stopped",
        }
        .to_owned(),
        summary: task.summary.clone(),
        output_preview: if task.summary.trim().is_empty() {
            None
        } else {
            Some(task.summary)
        },
        turns_used: task.turns_used,
        kind: match task.kind {
            claude_ui_bridge::UiTaskKind::Background => "background",
            claude_ui_bridge::UiTaskKind::Delegation => "delegation",
            claude_ui_bridge::UiTaskKind::Batch => "batch",
        }
        .to_owned(),
        output_path: task.output_path,
        created_at: task.created_at,
        updated_at: task.updated_at,
    }
}

fn load_session_tasks_from_paths(
    paths: &AppPaths,
    session_id: Uuid,
) -> Result<Vec<SessionTaskDto>> {
    let session_id_string = session_id.to_string();
    Ok(
        load_persisted_ui_task_snapshots(&task_dir_for_paths(paths, session_id))?
            .into_iter()
            .map(|task| ui_task_node_to_dto(&session_id_string, task))
            .collect(),
    )
}

fn emit_task_snapshot_for_session(app: &AppHandle, paths: &AppPaths, session_id: Uuid) {
    if let Ok(tasks) = load_session_tasks_from_paths(paths, session_id) {
        let _ = app.emit(
            APP_EVENT_TASK_SNAPSHOT,
            TaskSnapshotDto {
                session_id: session_id.to_string(),
                tasks,
            },
        );
    }
}

fn tool_call_to_dto(call: &ToolCall) -> ToolCallDto {
    ToolCallDto {
        id: call.id.clone(),
        name: call.name.clone(),
        input: call.input.clone(),
    }
}

fn conversation_entry_to_dto(entry: &ConversationEntry) -> ConversationEntryDto {
    ConversationEntryDto {
        role: match entry.role {
            ConversationRole::System => "system",
            ConversationRole::User => "user",
            ConversationRole::Assistant => "assistant",
            ConversationRole::Tool => "tool",
        }
        .to_owned(),
        text: entry.text.clone(),
        content_blocks: entry.content_blocks.clone(),
        tool_calls: entry.tool_calls.iter().map(tool_call_to_dto).collect(),
        tool_call_id: entry.tool_call_id.clone(),
        name: entry.name.clone(),
        is_error: entry.is_error,
    }
}

fn session_summary_to_dto(summary: SessionSummary) -> SessionSummaryDto {
    SessionSummaryDto {
        id: summary.session_id.to_string(),
        title: summary.title,
        cwd: summary.cwd.display().to_string(),
        provider_name: summary.provider_name,
        model: summary.model,
        created_at: summary.created_at.to_rfc3339(),
        updated_at: summary.updated_at.to_rfc3339(),
        archived: summary.archived,
    }
}

fn full_settings_from_runtime(
    config: &RuntimeConfig,
    gui_settings: &GuiSettingsFile,
) -> FullSettingsDto {
    FullSettingsDto {
        provider_name: config.provider.name.clone(),
        provider_model: config.provider.model.clone(),
        provider_base_url: config.provider.base_url.clone(),
        provider_protocol: config.provider.protocol.as_str().to_owned(),
        provider_api_key_set: config.provider.api_key.is_some(),
        max_output_tokens: config.provider.max_output_tokens,
        thinking_budget: config.provider.thinking_budget,
        max_retries: config.provider.max_retries,
        timeout_ms: config.provider.timeout_ms,
        retry_initial_backoff_ms: config.provider.retry_initial_backoff_ms,
        retry_max_backoff_ms: config.provider.retry_max_backoff_ms,
        respect_retry_after: config.provider.respect_retry_after,
        permission_mode: config.permission_mode.as_legacy_str().to_owned(),
        max_turns: config.max_turns,
        verbose: gui_settings.verbose.unwrap_or(config.verbose),
        codex_model_provider: gui_settings.codex_model_provider.clone(),
        codex_approval_policy: gui_settings.codex_approval_policy.clone(),
        codex_sandbox_mode: gui_settings.codex_sandbox_mode.clone(),
        codex_persist_extended_history: gui_settings.codex_persist_extended_history.unwrap_or(true),
        codex_memories_enabled: gui_settings.codex_memories_enabled.unwrap_or(true),
        codex_thread_store_endpoint: gui_settings.codex_thread_store_endpoint.clone(),
        codex_config_overrides: gui_settings.codex_config_overrides.clone(),
        codex_permission_profile: gui_settings.codex_permission_profile.clone(),
        codex_service_tier: gui_settings.codex_service_tier.clone(),
        codex_ephemeral: gui_settings.codex_ephemeral,
    }
}

fn apply_gui_settings_to_runtime(
    config: &mut RuntimeConfig,
    gui_settings: &GuiSettingsFile,
) -> Result<()> {
    if let Some(provider_name) = gui_settings.provider_name.as_deref() {
        config.provider.name = provider_name.trim().to_owned();
    }
    if let Some(model) = gui_settings.provider_model.clone() {
        config.provider.model = Some(model);
    }
    if let Some(base_url) = gui_settings.provider_base_url.clone() {
        let protocol = config.provider.protocol;
        config.provider.base_url = normalize_base_url(Some(base_url), protocol);
    }
    if let Some(protocol) = parse_protocol(gui_settings.provider_protocol.as_deref()) {
        config.provider.protocol = protocol;
        config.provider.base_url =
            normalize_base_url(config.provider.base_url.clone(), config.provider.protocol);
    }
    if let Some(max_output_tokens) = gui_settings.max_output_tokens {
        config.provider.max_output_tokens = max_output_tokens.max(256);
    }
    if let Some(thinking_budget) = gui_settings.thinking_budget {
        config.provider.thinking_budget = thinking_budget;
    }
    if let Some(max_retries) = gui_settings.max_retries {
        config.provider.max_retries = max_retries;
    }
    if let Some(timeout_ms) = gui_settings.timeout_ms {
        config.provider.timeout_ms = timeout_ms.max(1_000);
    }
    if let Some(retry_initial_backoff_ms) = gui_settings.retry_initial_backoff_ms {
        config.provider.retry_initial_backoff_ms = retry_initial_backoff_ms.max(50);
    }
    if let Some(retry_max_backoff_ms) = gui_settings.retry_max_backoff_ms {
        config.provider.retry_max_backoff_ms =
            retry_max_backoff_ms.max(config.provider.retry_initial_backoff_ms);
    }
    if let Some(respect_retry_after) = gui_settings.respect_retry_after {
        config.provider.respect_retry_after = respect_retry_after;
    }
    if let Some(permission_mode) = parse_permission_mode(gui_settings.permission_mode.as_deref()) {
        config.permission_mode = permission_mode;
    }
    if let Some(verbose) = gui_settings.verbose {
        config.verbose = verbose;
    }
    if let Some(thinking_budget) = config.provider.thinking_budget {
        if thinking_budget >= config.provider.max_output_tokens {
            return Err(anyhow!(
                "thinking budget must be lower than max output tokens"
            ));
        }
    }
    Ok(())
}

fn provider_config_to_runtime(
    stored: &ProviderConfig,
    current: &RuntimeProviderConfig,
) -> Result<RuntimeProviderConfig> {
    let protocol = parse_protocol(Some(&stored.protocol)).unwrap_or(ProviderProtocol::OpenAi);
    let base_url = normalize_base_url(stored.base_url.clone(), protocol);
    Ok(RuntimeProviderConfig {
        name: stored.name.clone(),
        base_url,
        api_key: trimmed_option(stored.api_key.clone()),
        model: trimmed_option(stored.model.clone()),
        protocol,
        timeout_ms: current.timeout_ms,
        max_output_tokens: current.max_output_tokens,
        max_retries: current.max_retries,
        retry_initial_backoff_ms: current.retry_initial_backoff_ms,
        retry_max_backoff_ms: current.retry_max_backoff_ms,
        respect_retry_after: current.respect_retry_after,
        request_header_overrides: current.request_header_overrides.clone(),
        request_metadata: current.request_metadata.clone(),
        thinking_budget: current.thinking_budget,
    })
}

fn sync_active_provider_from_runtime(config: &RuntimeConfig, configs: &mut ProviderConfigList) {
    if configs.providers.is_empty() {
        configs.active_provider = None;
        return;
    }
    if configs.active_provider.as_ref().is_some_and(|name| {
        configs
            .providers
            .iter()
            .any(|provider| provider.name == *name)
    }) {
        return;
    }
    if configs
        .providers
        .iter()
        .any(|provider| provider.name == config.provider.name)
    {
        configs.active_provider = Some(config.provider.name.clone());
        return;
    }
    configs.active_provider = configs
        .providers
        .first()
        .map(|provider| provider.name.clone());
}

fn active_provider_config(configs: &ProviderConfigList) -> Option<&ProviderConfig> {
    let active_name = configs.active_provider.as_ref()?;
    configs
        .providers
        .iter()
        .find(|provider| provider.name == *active_name)
}

fn load_base_runtime_config(profile_override: Option<PathBuf>) -> Result<RuntimeConfig> {
    load_runtime_config(
        None,
        profile_override,
        None,
        PermissionMode::Default,
        claude_core::InputFormat::Text,
        claude_core::OutputFormat::Text,
        false,
        false,
        false,
        false,
        DEFAULT_MAX_TURNS,
        ProviderOverrides::default(),
        RuntimeOverrides::default(),
    )
}

fn build_runtime_state() -> Result<RuntimeState> {
    let profile_override = profile_override_from_env();
    let mut config = load_base_runtime_config(profile_override)?;
    let mut provider_configs = load_provider_configs(&config.paths)?;
    let gui_settings = load_gui_settings(&config.paths)?;

    sync_active_provider_from_runtime(&config, &mut provider_configs);
    if let Some(stored) = active_provider_config(&provider_configs) {
        config.provider = provider_config_to_runtime(stored, &config.provider)?;
    }
    apply_gui_settings_to_runtime(&mut config, &gui_settings)?;

    let readiness = validate_provider_config(&config.provider);
    if !readiness.ok && readiness.issues.len() > 2 {
        config.provider.base_url =
            normalize_base_url(config.provider.base_url.clone(), config.provider.protocol);
    }
    configure_runtime_policy_for_config(&config)?;

    let provider = Arc::new(ProviderClient::new()?);
    let session_store = Arc::new(SessionStore::open(config.paths.clone())?);
    let sessions = session_store.list_active_sessions()?;
    let mut projects = load_projects(&config.paths)?;
    if ensure_sessions_have_projects(&mut projects, &sessions) {
        save_projects(&config.paths, &projects)?;
    }

    Ok(RuntimeState {
        config,
        provider,
        session_store,
        projects,
        provider_configs,
        gui_settings,
    })
}

fn persist_runtime_files(state: &RuntimeState) -> Result<()> {
    save_projects(&state.config.paths, &state.projects)?;
    save_provider_configs(&state.config.paths, &state.provider_configs)?;
    save_gui_settings(&state.config.paths, &state.gui_settings)?;
    Ok(())
}

fn persist_session_context(store: &SessionStore, config: &RuntimeConfig) -> Result<()> {
    persist_runtime_config_session_context(store, config)
}

fn restore_session_context(store: &SessionStore, config: &mut RuntimeConfig) -> Result<()> {
    restore_runtime_config_session_context(store, config)
}

fn initialize_session_conversation(
    store: &SessionStore,
    config: &RuntimeConfig,
    title_hint: Option<&str>,
) -> Result<Vec<ConversationEntry>> {
    persist_session_context(store, config)?;
    ensure_conversation_initialized(
        store,
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        title_hint,
    )
}

fn build_project_infos(
    stored_projects: &[ProjectEntry],
    sessions: &[SessionSummary],
) -> Vec<ProjectInfoDto> {
    let mut projects = Vec::new();

    for project in stored_projects {
        projects.push(ProjectInfoDto {
            path: project.path.display().to_string(),
            name: project.name.clone(),
            session_count: project_session_count(&project.path, sessions),
            is_auto_detected: false,
        });
    }

    projects.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    projects
}

fn find_provider_config_index(configs: &ProviderConfigList, name: &str) -> Option<usize> {
    configs
        .providers
        .iter()
        .position(|provider| provider.name == name)
}

fn apply_provider_credentials_from_configs(
    provider: &mut RuntimeProviderConfig,
    provider_configs: &ProviderConfigList,
) {
    let Some(index) = find_provider_config_index(provider_configs, &provider.name) else {
        return;
    };
    let stored = &provider_configs.providers[index];
    // Prefer OS keychain, fall back to JSON plaintext (backward compat).
    if let Some(api_key) = keyring_retrieve(&stored.name) {
        provider.api_key = Some(api_key);
    } else if let Some(api_key) = trimmed_option(stored.api_key.clone()) {
        provider.api_key = Some(api_key);
    }
    // Apply active profile model override if set.
    let profile_model = stored.active_profile.as_ref().and_then(|profile_name| {
        stored
            .profiles
            .iter()
            .find(|p| p.name == *profile_name)
            .and_then(|p| p.model.clone())
    });
    if let Some(ref model) = profile_model {
        provider.model = Some(model.clone());
    } else if provider.model.is_none() {
        provider.model = trimmed_option(stored.model.clone());
    }
    if provider.base_url.is_none() {
        provider.base_url = normalize_base_url(stored.base_url.clone(), provider.protocol);
    }
}

fn store_provider_selection(state: &mut RuntimeState, config: &RuntimeProviderConfig) {
    state.gui_settings.provider_name = Some(config.name.clone());
    state.gui_settings.provider_model = config.model.clone();
    state.gui_settings.provider_base_url = config.base_url.clone();
    state.gui_settings.provider_protocol = Some(config.protocol.as_str().to_owned());
}

fn codex_provider_id(provider: &RuntimeProviderConfig, gui_settings: &GuiSettingsFile) -> String {
    gui_settings
        .codex_model_provider
        .clone()
        .or_else(|| {
            Some(format!(
                "remote-code-{}",
                provider.name.to_ascii_lowercase()
            ))
        })
        .map(|value| {
            value
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                        ch.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .trim_matches('-')
                .to_owned()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "remote-code-provider".to_owned())
}

fn codex_approval_policy_from_permission_mode(mode: PermissionMode) -> String {
    match mode {
        PermissionMode::BypassPermissions => "never",
        PermissionMode::Default | PermissionMode::AcceptEdits | PermissionMode::DontAsk => {
            "on-request"
        }
        PermissionMode::Plan => "never",
    }
    .to_owned()
}

fn codex_sandbox_from_permission_mode(mode: PermissionMode) -> String {
    match mode {
        PermissionMode::BypassPermissions => "danger-full-access",
        PermissionMode::Plan => "read-only",
        PermissionMode::Default | PermissionMode::AcceptEdits | PermissionMode::DontAsk => {
            "workspace-write"
        }
    }
    .to_owned()
}

fn codex_mcp_server_overrides(config: &RuntimeConfig) -> HashMap<String, serde_json::Value> {
    let mut servers = HashMap::new();
    for entry in runtime_mcp_policy_entries(config, &config.mcp_config_paths) {
        if !entry.server.enabled {
            continue;
        }
        let mut server = serde_json::Map::new();
        server.insert("enabled".to_owned(), serde_json::Value::Bool(true));
        if let Some(timeout) = entry.server.startup_timeout_secs {
            server.insert("startup_timeout_sec".to_owned(), serde_json::json!(timeout));
        }
        if let Some(timeout) = entry.server.request_timeout_secs {
            server.insert("tool_timeout_sec".to_owned(), serde_json::json!(timeout));
        }

        match entry.server.transport {
            McpTransportConfig::Stdio {
                command,
                args,
                cwd,
                env,
            } => {
                server.insert("command".to_owned(), serde_json::json!(command));
                if !args.is_empty() {
                    server.insert("args".to_owned(), serde_json::json!(args));
                }
                if let Some(cwd) = cwd {
                    server.insert("cwd".to_owned(), serde_json::json!(cwd));
                }
                if !env.is_empty() {
                    server.insert("env".to_owned(), serde_json::json!(env));
                }
            }
            McpTransportConfig::Http { url, headers }
            | McpTransportConfig::WebSocket { url, headers } => {
                server.insert("url".to_owned(), serde_json::json!(url));
                if !headers.is_empty() {
                    server.insert("http_headers".to_owned(), serde_json::json!(headers));
                }
            }
        }

        servers.insert(entry.server.name, serde_json::Value::Object(server));
    }
    servers
}

fn codex_adapter_options_from_runtime(
    config: &RuntimeConfig,
    gui_settings: &GuiSettingsFile,
) -> CodexAdapterOptions {
    let mut config_overrides = gui_settings
        .codex_config_overrides
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<HashMap<_, _>>();
    if let Some(effort) = config.effort.clone() {
        config_overrides
            .entry("model_reasoning_effort".to_owned())
            .or_insert(effort);
    }

    CodexAdapterOptions {
        cwd: config.cwd.clone(),
        model: config
            .provider
            .model
            .clone()
            .or_else(|| config.fallback_model.clone()),
        model_provider: Some(codex_provider_id(&config.provider, gui_settings)),
        api_key: config.provider.api_key.clone(),
        base_url: config.provider.base_url.clone(),
        approval_policy: gui_settings.codex_approval_policy.clone().or_else(|| {
            Some(codex_approval_policy_from_permission_mode(
                config.permission_mode,
            ))
        }),
        sandbox_mode: gui_settings
            .codex_sandbox_mode
            .clone()
            .or_else(|| Some(codex_sandbox_from_permission_mode(config.permission_mode))),
        permission_profile: gui_settings.codex_permission_profile.clone(),
        service_tier: gui_settings.codex_service_tier.as_ref().map(|v| {
            serde_json::json!(v.to_lowercase())
        }),
        persist_extended_history: gui_settings.codex_persist_extended_history.unwrap_or(true),
        ephemeral: gui_settings.codex_ephemeral,
        memories_enabled: gui_settings.codex_memories_enabled.or(Some(true)),
        thread_store_endpoint: gui_settings.codex_thread_store_endpoint.clone(),
        config_overrides,
        cli_overrides: Vec::new(),
        mcp_servers: codex_mcp_server_overrides(config),
        enable_codex_api_key_env: true,
        client_name: Some("remote-code-gui".to_owned()),
        client_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        exec_server_url: env::var("CODEX_EXEC_SERVER_URL").ok(),
        channel_capacity: Some(1024),
        feedback_capture_enabled: true,
    }
}

fn codex_agent_config_from_options(
    options: &CodexAdapterOptions,
) -> claude_agent_protocol::types::AgentConfig {
    claude_agent_protocol::types::AgentConfig {
        agent_type: ProtocolAgentType::RemoteCodex,
        binary_path: None,
        args: Vec::new(),
        env: Vec::new(),
        working_dir: Some(options.cwd.clone()),
        model: options.model.clone(),
        provider: None,
        api_key: None,
        base_url: None,
    }
}

async fn codex_options_snapshot(
    state: &State<'_, AppState>,
) -> std::result::Result<CodexAdapterOptions, String> {
    let mut config = {
        let runtime = state.runtime.lock().await;
        let mut config = runtime.config.clone();
        apply_provider_credentials_from_configs(&mut config.provider, &runtime.provider_configs);
        codex_adapter_options_from_runtime(&config, &runtime.gui_settings)
    };
    if config.cwd.as_os_str().is_empty() {
        config.cwd = env::current_dir().map_err(|error| error.to_string())?;
    }
    Ok(config)
}

async fn ensure_codex_adapter(
    state: &State<'_, AppState>,
    key: &str,
) -> std::result::Result<(), String> {
    let options = codex_options_snapshot(state).await?;
    let mut adapters = state.active_codex_adapters.lock().await;
    let needs_create = adapters
        .get(key)
        .map(|adapter| !adapter.is_alive())
        .unwrap_or(true);
    if needs_create {
        let agent_config = codex_agent_config_from_options(&options);
        let mut adapter = CodexInProcessAdapter::start_in_process_with_options(options)
            .await
            .map_err(|error| format!("Failed to start Codex runtime: {error:#}"))?;
        adapter
            .start(&agent_config)
            .await
            .map_err(|error| format!("Failed to initialize Codex adapter: {error:#}"))?;
        adapters.insert(key.to_owned(), adapter);
    }
    Ok(())
}

fn codex_adapter_key(session_id: Option<String>) -> String {
    session_id
        .and_then(|value| trimmed_option(Some(value)))
        .unwrap_or_else(|| CODEX_GLOBAL_ADAPTER_KEY.to_owned())
}

async fn with_codex_adapter_value<F>(
    state: &State<'_, AppState>,
    session_id: Option<String>,
    operation: F,
) -> std::result::Result<serde_json::Value, String>
where
    F: for<'a> FnOnce(
        &'a mut CodexInProcessAdapter,
    ) -> Pin<
        Box<dyn Future<Output = anyhow::Result<serde_json::Value>> + Send + 'a>,
    >,
{
    let key = codex_adapter_key(session_id);
    ensure_codex_adapter(state, &key).await?;
    let mut adapters = state.active_codex_adapters.lock().await;
    let adapter = adapters
        .get_mut(&key)
        .ok_or_else(|| "Codex adapter was not initialized".to_owned())?;
    operation(adapter)
        .await
        .map_err(|error| format!("{error:#}"))
}

fn parse_codex_mcp_detail(value: Option<&str>) -> Option<McpServerStatusDetail> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value)
            if value.eq_ignore_ascii_case("toolsAndAuthOnly")
                || value.eq_ignore_ascii_case("tools_and_auth_only")
                || value.eq_ignore_ascii_case("tools-auth") =>
        {
            Some(McpServerStatusDetail::ToolsAndAuthOnly)
        }
        Some(value) if value.eq_ignore_ascii_case("full") => Some(McpServerStatusDetail::Full),
        _ => None,
    }
}

fn parse_codex_merge_strategy(value: Option<&str>) -> MergeStrategy {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) if value.eq_ignore_ascii_case("upsert") => MergeStrategy::Upsert,
        _ => MergeStrategy::Replace,
    }
}

fn decode_codex_params<T>(
    params: Option<serde_json::Value>,
    default: impl FnOnce() -> T,
) -> Result<T>
where
    T: DeserializeOwned,
{
    params
        .map(serde_json::from_value)
        .transpose()
        .context("invalid Codex app-server params")
        .map(|value| value.unwrap_or_else(default))
}

fn decode_required_codex_params<T>(params: Option<serde_json::Value>) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(params.unwrap_or(serde_json::Value::Object(Default::default())))
        .context("invalid Codex app-server params")
}

fn parse_codex_plugin_ref(value: String) -> CodexPluginRefRequest {
    let value = value.trim().to_owned();
    if let Some((plugin_name, remote_marketplace_name)) = value.split_once('@') {
        return CodexPluginRefRequest {
            marketplace_path: None,
            remote_marketplace_name: Some(remote_marketplace_name.to_owned()),
            plugin_name: plugin_name.to_owned(),
        };
    }

    CodexPluginRefRequest {
        marketplace_path: None,
        remote_marketplace_name: None,
        plugin_name: value,
    }
}

#[derive(Debug)]
struct GuiPermissionFallbackBroker {
    controller: Arc<RuntimePlanModeController>,
    app: AppHandle,
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
}

fn permission_request_dto(request_id: String, request: &PermissionRequest) -> PermissionRequestDto {
    PermissionRequestDto {
        request_id,
        tool_name: request.tool_name.clone(),
        tool_use_id: request.tool_use_id.clone().unwrap_or_default(),
        title: request.title.clone().unwrap_or_default(),
        description: request.description.clone().unwrap_or_default(),
        input: request.tool_input.clone(),
        blocked_path: request.blocked_path.clone(),
        permission_suggestions: request.permission_suggestions.clone(),
    }
}

impl GuiPermissionFallbackBroker {
    async fn prompt(&self, request: PermissionRequest) -> PermissionDecision {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_permissions.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        let payload = permission_request_dto(request_id.clone(), &request);

        if self
            .app
            .emit(APP_EVENT_PERMISSION_REQUEST, payload)
            .is_err()
        {
            let mut pending = self.pending_permissions.lock().await;
            pending.remove(&request_id);
            return PermissionDecision::deny("Failed to deliver permission request to GUI.");
        }

        let decision = timeout(Duration::from_secs(PERMISSION_WAIT_SECS), rx).await;
        let decision = match decision {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => PermissionDecision::deny("Permission request channel closed."),
            Err(_) => PermissionDecision::deny(format!(
                "Permission request timed out for {}.",
                request.tool_name
            )),
        };

        let _ = self.app.emit(
            APP_EVENT_PERMISSION_RESOLVED,
            PermissionDecisionDto {
                request_id,
                allowed: decision.allowed,
                message: decision.message.clone(),
                updated_input: decision.updated_input.clone(),
                permission_updates: decision.permission_updates.clone(),
                feedback: decision.feedback.clone(),
                content_blocks: decision.content_blocks.clone(),
            },
        );

        decision
    }
}

#[async_trait]
impl PermissionBroker for GuiPermissionFallbackBroker {
    fn mode(&self) -> Option<PermissionMode> {
        Some(self.controller.current_mode())
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        let mode = self.controller.current_mode();

        if matches!(mode, PermissionMode::BypassPermissions) && request.blocked_path.is_none() {
            return PermissionDecision::allow();
        }

        if matches!(mode, PermissionMode::DontAsk | PermissionMode::AcceptEdits)
            && request.blocked_path.is_none()
            && auto_allows(mode, classify_tool(&request.tool_name))
        {
            return PermissionDecision::allow();
        }

        if matches!(mode, PermissionMode::Plan) {
            return PermissionDecision::deny(
                "Plan mode is active. Only read-only tools and plan-file edits are allowed.",
            );
        }

        self.prompt(request).await
    }

    async fn decide_forced_prompt(&self, request: PermissionRequest) -> PermissionDecision {
        self.prompt(request).await
    }
}

struct GuiRuntimePermissionBroker {
    controller: Arc<RuntimePlanModeController>,
    inner: LayeredPermissionBroker<GuiPermissionFallbackBroker>,
}

impl std::fmt::Debug for GuiRuntimePermissionBroker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiRuntimePermissionBroker")
            .field("mode", &self.controller.current_mode())
            .finish_non_exhaustive()
    }
}

impl GuiRuntimePermissionBroker {
    fn new(
        config: &RuntimeConfig,
        controller: Arc<RuntimePlanModeController>,
        app: AppHandle,
        pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
    ) -> Self {
        let inner = LayeredPermissionBroker::new(
            GuiPermissionFallbackBroker {
                controller: controller.clone(),
                app,
                pending_permissions,
            },
            load_layered_rules(
                &config.cwd,
                &config.paths.profile_dir,
                &config.settings_files,
                &config.cli_settings_files,
            ),
        );
        Self { controller, inner }
    }

    fn decide_plan_mode(&self, request: PermissionRequest) -> PermissionDecision {
        match request.resolved_permission_class() {
            PermissionClass::Read => PermissionDecision::allow(),
            PermissionClass::Edit if self.controller.plan_file_matches_request(&request) => {
                PermissionDecision::allow()
            }
            PermissionClass::Edit => PermissionDecision::deny(
                "Plan mode is active. Only the current plan file may be edited.",
            ),
            _ => PermissionDecision::deny(
                "Plan mode is active. Only read-only tools and plan-file edits are allowed.",
            ),
        }
    }
}

#[async_trait]
impl PermissionBroker for GuiRuntimePermissionBroker {
    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        if self.controller.current_mode() == PermissionMode::Plan {
            return self.decide_plan_mode(request);
        }
        self.inner.decide(request).await
    }

    async fn decide_forced_prompt(&self, request: PermissionRequest) -> PermissionDecision {
        self.inner.decide_forced_prompt(request).await
    }

    fn mode(&self) -> Option<PermissionMode> {
        if self.controller.current_mode() == PermissionMode::Plan {
            Some(PermissionMode::Plan)
        } else {
            self.inner.mode()
        }
    }

    fn additional_working_directories(&self) -> Vec<std::path::PathBuf> {
        self.inner.additional_working_directories()
    }

    fn add_session_rule(
        &self,
        action: claude_permissions::RuleAction,
        tool_pattern: String,
    ) -> Result<()> {
        self.inner.add_session_rule(action, tool_pattern)
    }

    fn clear_session_rules(&self) -> Result<usize> {
        self.inner.clear_session_rules()
    }

    fn apply_permission_updates(
        &self,
        updates: &[claude_permissions::PermissionUpdate],
    ) -> Result<usize> {
        self.inner.apply_permission_updates(updates)
    }

    fn audit_records(&self) -> Vec<claude_permissions::PermissionAuditRecord> {
        self.inner.audit_records()
    }

    fn layered_rules(&self) -> Vec<claude_permissions::SourceAwarePermissionRule> {
        self.inner.layered_rules()
    }

    fn matching_rule(
        &self,
        request: &PermissionRequest,
    ) -> Option<claude_permissions::SourceAwarePermissionRule> {
        self.inner.matching_rule(request)
    }

    fn matching_rule_action(
        &self,
        request: &PermissionRequest,
    ) -> Option<claude_permissions::RuleAction> {
        self.inner.matching_rule_action(request)
    }
}

fn as_error<T>(result: Result<T>) -> std::result::Result<T, String> {
    result.map_err(|error| format!("{error:#}"))
}

/// Read the agent_type stored in session metadata.
/// Returns `"remote_claude"` when no agent_type has been set (the default path).
fn get_session_agent_type(store: &SessionStore, session_id: Uuid) -> String {
    store
        .load_transcript(session_id)
        .ok()
        .and_then(|transcript| {
            transcript
                .latest_named_event_payload("agent_type")
                .and_then(|val| {
                    val.get("agent_type")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
        })
        .unwrap_or_else(|| "remote_claude".to_owned())
}

#[allow(dead_code)]
fn codex_cli_binary_path() -> Result<PathBuf> {
    let binary_name = if cfg!(windows) { "codex.exe" } else { "codex" };
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        let candidate = exe_dir.join(binary_name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let mut candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidate.pop();
    candidate.pop();
    candidate.pop();
    candidate = candidate.join("target").join("debug").join(binary_name);
    if candidate.exists() {
        return Ok(candidate);
    }

    let lookup_cmd = if cfg!(windows) { "where" } else { "which" };
    if let Ok(output) = std::process::Command::new(lookup_cmd)
        .arg(if cfg!(windows) { "codex.exe" } else { "codex" })
        .output()
        && output.status.success()
    {
        if let Some(first_line) = String::from_utf8_lossy(&output.stdout).lines().next() {
            let path = PathBuf::from(first_line.trim());
            if path.exists() {
                return Ok(path);
            }
        }
    }

    Err(anyhow!(
        "Could not locate Codex CLI binary. Build `cargo build -p codex-cli` first."
    ))
}

// ---------------------------------------------------------------------------
// In-process Codex prompt execution
// ---------------------------------------------------------------------------

/// Execute a prompt via the native Codex in-process adapter.
///
/// Creates or reuses a [`CodexInProcessAdapter`] with isolated storage,
/// sends the user message, and forwards events to the frontend via Tauri emissions.
async fn run_codex_in_process_prompt(
    app: &AppHandle,
    codex_adapters: &Arc<Mutex<HashMap<String, CodexInProcessAdapter>>>,
    pending_codex_permissions: &Arc<Mutex<HashMap<String, CodexPendingPermission>>>,
    session_id: &str,
    prompt: &str,
    options: CodexAdapterOptions,
) -> std::result::Result<String, String> {
    let working_dir = options.cwd.clone();
    let model = options.model.clone();
    // Ensure the adapter exists for this session.
    {
        let mut adapters = codex_adapters.lock().await;
        if !adapters.contains_key(session_id) {
            tracing::info!(session_id, "Creating new CodexInProcessAdapter");
            let adapter = CodexInProcessAdapter::start_in_process_with_options(options)
                .await
                .map_err(|e| format!("Failed to start Codex in-process runtime: {e}"))?;
            adapters.insert(session_id.to_string(), adapter);
        }
    }

    // Get the adapter and send the message.
    let mut adapters = codex_adapters.lock().await;
    let adapter = adapters
        .get_mut(session_id)
        .ok_or_else(|| "Codex adapter not found".to_string())?;

    // Start the adapter if not yet started.
    if !adapter.is_alive() {
        let agent_config = claude_agent_protocol::types::AgentConfig {
            agent_type: ProtocolAgentType::RemoteCodex,
            binary_path: None,
            args: Vec::new(),
            env: Vec::new(),
            working_dir: Some(working_dir.clone()),
            model,
            provider: None,
            api_key: None,
            base_url: None,
        };
        adapter
            .start(&agent_config)
            .await
            .map_err(|e| format!("CodexInProcessAdapter::start failed: {e}"))?;
    }

    let mut rx = adapter
        .send_message(session_id, prompt)
        .await
        .map_err(|e| format!("CodexInProcessAdapter::send_message failed: {e}"))?;

    // Forward events to the frontend. Drop the lock before streaming.
    drop(adapters);

    let mut final_text = String::new();
    let mut tool_calls: Vec<ToolCallDto> = Vec::new();
    let mut usage_info: claude_agent_protocol::events::UsageInfo = Default::default();
    let mut stop_reason = "completed".to_owned();
    while let Some(event) = rx.recv().await {
        use claude_agent_protocol::events::UnifiedAgentEvent;
        match event {
            UnifiedAgentEvent::MessageDelta { delta, .. } => {
                final_text.push_str(&delta);
                let _ = app.emit(
                    APP_EVENT_STREAMING_DELTA,
                    StreamingDeltaDto {
                        session_id: session_id.to_string(),
                        delta,
                    },
                );
            }
            UnifiedAgentEvent::ToolCallStarted {
                tool_name,
                tool_input,
                ..
            } => {
                let tool_call_id = Uuid::new_v4().to_string();
                tool_calls.push(ToolCallDto {
                    id: tool_call_id.clone(),
                    name: tool_name.clone(),
                    input: tool_input,
                });
                let _ = app.emit(
                    APP_EVENT_TOOL_START,
                    ToolProgressDto {
                        tool_call_id,
                        tool_name,
                        message: "started".to_owned(),
                    },
                );
            }
            UnifiedAgentEvent::ToolCallProgress {
                tool_name,
                progress,
                ..
            } => {
                if tool_name == "codex_token_usage" {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&progress) {
                        if let Some(next_usage) = usage_info_from_codex_token_usage(&value) {
                            usage_info = next_usage;
                        }
                    }
                }
                let _ = app.emit(
                    APP_EVENT_TOOL_PROGRESS,
                    ToolProgressDto {
                        tool_call_id: String::new(),
                        tool_name,
                        message: progress,
                    },
                );
            }
            UnifiedAgentEvent::CodexAppServerNotification { method, params, .. } => {
                let _ = app.emit(
                    APP_EVENT_CODEX_APP_SERVER_NOTIFICATION,
                    serde_json::json!({
                        "session_id": session_id,
                        "method": method,
                        "params": params.clone(),
                    }),
                );
                let _ = app.emit(
                    APP_EVENT_TOOL_PROGRESS,
                    ToolProgressDto {
                        tool_call_id: String::new(),
                        tool_name: "codex_app_server_event".to_owned(),
                        message: serde_json::json!({
                            "method": method,
                            "params": params,
                        })
                        .to_string(),
                    },
                );
            }
            UnifiedAgentEvent::ToolCallCompleted {
                tool_name, result, ..
            } => {
                let _ = app.emit(
                    APP_EVENT_TOOL_RESULT,
                    ToolResultDto {
                        tool_call_id: String::new(),
                        tool_name,
                        is_error: false,
                        output: result.to_string(),
                    },
                );
            }
            UnifiedAgentEvent::PermissionRequest {
                request_id,
                tool_name,
                input,
                ..
            } => {
                let gui_request_id = Uuid::new_v4().to_string();
                {
                    let mut pending = pending_codex_permissions.lock().await;
                    pending.insert(
                        gui_request_id.clone(),
                        CodexPendingPermission {
                            session_id: session_id.to_string(),
                            request_id,
                        },
                    );
                }
                let _ = app.emit(
                    APP_EVENT_PERMISSION_REQUEST,
                    PermissionRequestDto {
                        request_id: gui_request_id,
                        tool_name: tool_name.clone(),
                        tool_use_id: String::new(),
                        title: "Codex 请求权限".to_owned(),
                        description: format!("工具 {tool_name} 需要授权才能执行。"),
                        input,
                        blocked_path: None,
                        permission_suggestions: vec![],
                    },
                );
            }
            UnifiedAgentEvent::SubtaskStarted {
                task_id,
                description,
                ..
            } => {
                let _ = app.emit(
                    APP_EVENT_SUBTASK_STARTED,
                    SubtaskStartedDto {
                        session_id: session_id.to_string(),
                        task_id,
                        parent_task_id: None,
                        description,
                        depth: 0,
                    },
                );
            }
            UnifiedAgentEvent::SubtaskProgress {
                task_id, progress, ..
            } => {
                let _ = app.emit(
                    APP_EVENT_SUBTASK_PROGRESS,
                    SubtaskProgressDto {
                        session_id: session_id.to_string(),
                        task_id,
                        turn: 0,
                        max_turns: 0,
                        summary: progress,
                    },
                );
            }
            UnifiedAgentEvent::SubtaskCompleted {
                task_id, result, ..
            } => {
                let _ = app.emit(
                    APP_EVENT_SUBTASK_COMPLETED,
                    SubtaskCompletedDto {
                        session_id: session_id.to_string(),
                        task_id,
                        success: true,
                        output_preview: result.to_string(),
                        turns_used: 0,
                    },
                );
            }
            UnifiedAgentEvent::ContextUsage { used, total, .. } => {
                let _ = app.emit(
                    APP_EVENT_CONTEXT_USAGE,
                    serde_json::json!({
                        "session_id": session_id,
                        "used": used,
                        "total": total,
                    }),
                );
            }
            UnifiedAgentEvent::ContextOverflow { used, total, .. } => {
                let _ = app.emit(
                    APP_EVENT_CONTEXT_OVERFLOW,
                    serde_json::json!({
                        "session_id": session_id,
                        "used": used,
                        "total": total,
                    }),
                );
            }
            UnifiedAgentEvent::ContextCompacted {
                entries_removed,
                usage_ratio,
                ..
            } => {
                let _ = app.emit(
                    APP_EVENT_CONTEXT_COMPACTED,
                    serde_json::json!({
                        "session_id": session_id,
                        "entries_removed": entries_removed,
                        "usage_ratio": usage_ratio,
                    }),
                );
            }
            UnifiedAgentEvent::Completed { result, .. } => {
                if !result.response_text.is_empty() {
                    final_text = result.response_text;
                }
                if !result.tool_calls.is_empty() {
                    tool_calls = result
                        .tool_calls
                        .into_iter()
                        .map(|tc| ToolCallDto {
                            id: tc.id,
                            name: tc.name,
                            input: tc.input,
                        })
                        .collect();
                }
                if result.usage.input_tokens > 0
                    || result.usage.output_tokens > 0
                    || result.usage.cache_read > 0
                    || result.usage.cache_write > 0
                {
                    usage_info = result.usage;
                }
                break;
            }
            UnifiedAgentEvent::Error {
                message,
                recoverable,
                ..
            } => {
                tracing::warn!(error = %message, "Codex error event");
                if recoverable {
                    let _ = app.emit(
                        APP_EVENT_CODEX_RECOVERABLE_ERROR,
                        serde_json::json!({
                            "session_id": session_id,
                            "message": message,
                            "timestamp": chrono::Utc::now().timestamp_millis(),
                        }),
                    );
                } else {
                    return Err(message);
                }
            }
            UnifiedAgentEvent::Stopped => {
                stop_reason = "stopped".to_owned();
                break;
            }
            UnifiedAgentEvent::Started(_) | UnifiedAgentEvent::Ready => {
                tracing::debug!(event = ?event, "Codex lifecycle event");
            }
        }
    }

    let _ = app.emit(
        APP_EVENT_PROMPT_DONE,
        PromptDoneDto {
            session_id: session_id.to_string(),
            is_error: false,
            error: None,
            result: Some(PromptResultDto {
                session_id: session_id.to_string(),
                text: final_text.clone(),
                tool_calls,
                usage: UsageDto {
                    input_tokens: usage_info.input_tokens,
                    output_tokens: usage_info.output_tokens,
                    total_tokens: usage_info.input_tokens + usage_info.output_tokens,
                },
                num_turns: 1,
                stop_reason,
            }),
        },
    );

    Ok(final_text)
}

// ---------------------------------------------------------------------------
// Roo in-process prompt execution
// ---------------------------------------------------------------------------

/// Execute a prompt via the native Roo in-process adapter.
///
/// This creates a [`RooInProcessAdapter`], starts it, sends the user message,
/// and forwards events to the frontend via Tauri emissions.
async fn run_roo_in_process_prompt(
    app: &AppHandle,
    roo_adapters: &Arc<Mutex<HashMap<String, RooInProcessAdapter>>>,
    session_id: &str,
    prompt: &str,
    working_dir: PathBuf,
    model: Option<String>,
    api_key: Option<String>,
    provider_name: String,
) -> std::result::Result<String, String> {
    // Ensure the adapter exists for this session.
    {
        let mut adapters = roo_adapters.lock().await;
        if !adapters.contains_key(session_id) {
            tracing::info!(session_id, "Creating new RooInProcessAdapter");
            let mut adapter = RooInProcessAdapter::new();
            let agent_config = claude_agent_protocol::types::AgentConfig {
                agent_type: ProtocolAgentType::RemoteRoo,
                binary_path: None,
                args: Vec::new(),
                env: Vec::new(),
                working_dir: Some(working_dir.clone()),
                model: model.clone(),
                provider: Some(provider_name.clone()),
                api_key: api_key.clone(),
                base_url: None,
            };
            adapter
                .start(&agent_config)
                .await
                .map_err(|e| format!("Failed to start Roo in-process runtime: {e}"))?;
            adapters.insert(session_id.to_string(), adapter);
        }
    }

    // Get the adapter and send the message.
    let mut adapters = roo_adapters.lock().await;
    let adapter = adapters
        .get_mut(session_id)
        .ok_or_else(|| "Roo adapter not found".to_string())?;

    let mut rx = adapter
        .send_message(session_id, prompt)
        .await
        .map_err(|e| format!("RooInProcessAdapter::send_message failed: {e}"))?;

    // Forward events to the frontend. Drop the lock before streaming.
    drop(adapters);

    let mut final_text = String::new();
    let mut tool_calls: Vec<ToolCallDto> = Vec::new();
    let mut usage_info: claude_agent_protocol::events::UsageInfo = Default::default();
    let mut stop_reason = "completed".to_owned();
    while let Some(event) = rx.recv().await {
        use claude_agent_protocol::events::UnifiedAgentEvent;
        match event {
            UnifiedAgentEvent::MessageDelta { delta, .. } => {
                final_text.push_str(&delta);
                let _ = app.emit(
                    APP_EVENT_STREAMING_DELTA,
                    StreamingDeltaDto {
                        session_id: session_id.to_string(),
                        delta,
                    },
                );
            }
            UnifiedAgentEvent::ToolCallStarted {
                tool_name,
                tool_input,
                ..
            } => {
                let tool_call_id = Uuid::new_v4().to_string();
                tool_calls.push(ToolCallDto {
                    id: tool_call_id.clone(),
                    name: tool_name.clone(),
                    input: tool_input,
                });
                let _ = app.emit(
                    APP_EVENT_TOOL_START,
                    ToolProgressDto {
                        tool_call_id,
                        tool_name,
                        message: "started".to_owned(),
                    },
                );
            }
            UnifiedAgentEvent::ToolCallProgress {
                tool_name,
                progress,
                ..
            } => {
                let _ = app.emit(
                    APP_EVENT_TOOL_PROGRESS,
                    ToolProgressDto {
                        tool_call_id: String::new(),
                        tool_name,
                        message: progress,
                    },
                );
            }
            UnifiedAgentEvent::ToolCallCompleted {
                tool_name, result, ..
            } => {
                let _ = app.emit(
                    APP_EVENT_TOOL_RESULT,
                    ToolResultDto {
                        tool_call_id: String::new(),
                        tool_name,
                        is_error: false,
                        output: result.to_string(),
                    },
                );
            }
            UnifiedAgentEvent::SubtaskStarted {
                task_id,
                description,
                ..
            } => {
                let _ = app.emit(
                    APP_EVENT_SUBTASK_STARTED,
                    SubtaskStartedDto {
                        session_id: session_id.to_string(),
                        task_id,
                        parent_task_id: None,
                        description,
                        depth: 0,
                    },
                );
            }
            UnifiedAgentEvent::SubtaskProgress {
                task_id, progress, ..
            } => {
                let _ = app.emit(
                    APP_EVENT_SUBTASK_PROGRESS,
                    SubtaskProgressDto {
                        session_id: session_id.to_string(),
                        task_id,
                        turn: 0,
                        max_turns: 0,
                        summary: progress,
                    },
                );
            }
            UnifiedAgentEvent::SubtaskCompleted {
                task_id, result, ..
            } => {
                let _ = app.emit(
                    APP_EVENT_SUBTASK_COMPLETED,
                    SubtaskCompletedDto {
                        session_id: session_id.to_string(),
                        task_id,
                        success: true,
                        output_preview: result.to_string(),
                        turns_used: 0,
                    },
                );
            }
            UnifiedAgentEvent::ContextUsage { used, total, .. } => {
                let _ = app.emit(
                    APP_EVENT_CONTEXT_USAGE,
                    serde_json::json!({
                        "session_id": session_id,
                        "used": used,
                        "total": total,
                    }),
                );
            }
            UnifiedAgentEvent::ContextOverflow { used, total, .. } => {
                let _ = app.emit(
                    APP_EVENT_CONTEXT_OVERFLOW,
                    serde_json::json!({
                        "session_id": session_id,
                        "used": used,
                        "total": total,
                    }),
                );
            }
            UnifiedAgentEvent::ContextCompacted {
                entries_removed,
                usage_ratio,
                ..
            } => {
                let _ = app.emit(
                    APP_EVENT_CONTEXT_COMPACTED,
                    serde_json::json!({
                        "session_id": session_id,
                        "entries_removed": entries_removed,
                        "usage_ratio": usage_ratio,
                    }),
                );
            }
            UnifiedAgentEvent::Completed { result, .. } => {
                if !result.response_text.is_empty() {
                    final_text = result.response_text;
                }
                if !result.tool_calls.is_empty() {
                    tool_calls = result
                        .tool_calls
                        .into_iter()
                        .map(|tc| ToolCallDto {
                            id: tc.id,
                            name: tc.name,
                            input: tc.input,
                        })
                        .collect();
                }
                if result.usage.input_tokens > 0
                    || result.usage.output_tokens > 0
                    || result.usage.cache_read > 0
                    || result.usage.cache_write > 0
                {
                    usage_info = result.usage;
                }
                break;
            }
            UnifiedAgentEvent::Error {
                message,
                recoverable,
                ..
            } => {
                tracing::warn!(error = %message, "Roo error event");
                if !recoverable {
                    return Err(message);
                }
            }
            UnifiedAgentEvent::Stopped => {
                stop_reason = "stopped".to_owned();
                break;
            }
            UnifiedAgentEvent::Started(_) | UnifiedAgentEvent::Ready => {
                tracing::debug!(event = ?event, "Roo lifecycle event");
            }
            // Ignore Codex-specific events that Roo won't emit
            _ => {}
        }
    }

    let _ = app.emit(
        APP_EVENT_PROMPT_DONE,
        PromptDoneDto {
            session_id: session_id.to_string(),
            is_error: false,
            error: None,
            result: Some(PromptResultDto {
                session_id: session_id.to_string(),
                text: final_text.clone(),
                tool_calls,
                usage: UsageDto {
                    input_tokens: usage_info.input_tokens,
                    output_tokens: usage_info.output_tokens,
                    total_tokens: usage_info.input_tokens + usage_info.output_tokens,
                },
                num_turns: 1,
                stop_reason,
            }),
        },
    );

    Ok(final_text)
}

#[tauri::command]
async fn init_app(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<InitResultDto, String> {
    let runtime = state.runtime.lock().await;
    emit_runtime_status(&app, &runtime.config);
    let sessions_count = runtime
        .session_store
        .list_active_sessions()
        .map_err(|error| format!("{error:#}"))?
        .len();
    Ok(InitResultDto {
        provider: Some(provider_info_from_runtime(&runtime.config.provider)),
        sessions_count,
    })
}

#[tauri::command]
async fn list_sessions(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<SessionSummaryDto>, String> {
    let runtime = state.runtime.lock().await;
    let sessions = runtime
        .session_store
        .list_active_sessions()
        .map_err(|error| format!("{error:#}"))?;
    Ok(sessions.into_iter().map(session_summary_to_dto).collect())
}

#[tauri::command]
async fn list_archived_sessions(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<SessionSummaryDto>, String> {
    let runtime = state.runtime.lock().await;
    let sessions = runtime
        .session_store
        .list_archived_sessions()
        .map_err(|error| format!("{error:#}"))?;
    Ok(sessions.into_iter().map(session_summary_to_dto).collect())
}

#[tauri::command]
async fn get_session_conversation(
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<Vec<ConversationEntryDto>, String> {
    let runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let conversation = runtime
        .session_store
        .load_conversation(session_id)
        .map_err(|error| format!("{error:#}"))?;
    Ok(conversation.iter().map(conversation_entry_to_dto).collect())
}

#[tauri::command]
async fn get_session_tasks(
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<Vec<SessionTaskDto>, String> {
    let runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    load_session_tasks_from_paths(&runtime.config.paths, session_id)
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn create_session(
    state: State<'_, AppState>,
    title: Option<String>,
    project_path: Option<String>,
    agent_type: Option<String>,
) -> std::result::Result<String, String> {
    // Fix #9: extract all needed data from runtime, then drop the lock before
    // acquiring agent_router — avoids potential deadlock from nested locks.
    let (mut config, session_store, projects) = {
        let runtime = state.runtime.lock().await;
        let config = runtime.config.clone();
        (
            config,
            Arc::clone(&runtime.session_store),
            runtime.projects.clone(),
        )
    };
    config.session_id = Uuid::new_v4();
    let project_path = project_path
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "请选择项目文件夹后再新建会话。".to_owned())?;
    let normalized_project_path =
        normalize_existing_path(Path::new(&project_path)).map_err(|error| format!("{error:#}"))?;
    let path_key = path_identity(&normalized_project_path);
    if !projects
        .iter()
        .any(|project| path_identity(&project.path) == path_key)
    {
        return Err("会话必须创建在已管理的项目文件夹下。".to_owned());
    }
    config.cwd = normalized_project_path.clone();

    // Parse and validate agent_type; default to "remote_claude" when not provided.
    let agent_type_str = agent_type.as_deref().unwrap_or("remote_claude").to_owned();
    let _parsed_agent_type: ProtocolAgentType =
        serde_json::from_str(&format!("\"{}\"", agent_type_str))
            .map_err(|e| format!("无效的 agent_type: {e}"))?;

    as_error(initialize_session_conversation(
        &session_store,
        &config,
        title.as_deref(),
    ))?;

    // Persist agent_type into session transcript as a named event.
    as_error(session_store.append_named_event(
        config.session_id,
        "agent_type",
        serde_json::json!({ "agent_type": agent_type_str }),
    ))?;

    // All Agent types use native in-process integration:
    // - Codex → CodexInProcessAdapter (rc-codex-adapter)
    // - Claude → QueryEngine (rc-query-engine)
    // - Roo → RooInProcessAdapter (rc-roo-adapter)
    // No subprocess/bridge mode is used.

    Ok(config.session_id.to_string())
}

#[tauri::command]
async fn send_prompt(
    app: AppHandle,
    state: State<'_, AppState>,
    prompt: String,
    session_id: Option<String>,
) -> std::result::Result<String, String> {
    let prompt = prompt.trim().to_owned();
    if prompt.is_empty() {
        return Err("prompt cannot be empty".to_owned());
    }

    let (
        mut config,
        provider,
        session_store,
        pending_permissions,
        provider_configs,
        gui_settings,
        agent_type_str,
    ) = {
        let runtime = state.runtime.lock().await;
        let mut config = runtime.config.clone();
        let selected_provider = config.provider.clone();
        let selected_permission_mode = config.permission_mode;
        let session_id =
            session_id.ok_or_else(|| "请先选择项目文件夹并创建会话，再发送消息。".to_owned())?;
        config.session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
        restore_session_context(&runtime.session_store, &mut config)
            .map_err(|error| format!("{error:#}"))?;
        let agent_type_str = get_session_agent_type(&runtime.session_store, config.session_id);
        config.provider = selected_provider;
        config.permission_mode = selected_permission_mode;
        (
            config,
            Arc::clone(&runtime.provider),
            Arc::clone(&runtime.session_store),
            Arc::clone(&state.pending_permissions),
            runtime.provider_configs.clone(),
            runtime.gui_settings.clone(),
            agent_type_str,
        )
    };

    apply_provider_credentials_from_configs(&mut config.provider, &provider_configs);
    configure_runtime_policy_for_config(&config).map_err(|error| format!("{error:#}"))?;

    let sid = config.session_id.to_string();

    let running_prompts = Arc::clone(&state.running_prompts);
    let sid_for_cleanup = sid.clone();

    // Atomically check for duplicate and reserve the slot to prevent TOCTOU races.
    {
        let mut running = state.running_prompts.lock().await;
        if running.contains_key(&sid) {
            return Err("该会话已有正在运行的提示，请等待完成或取消后再试。".to_owned());
        }

        // ── Branch based on agent_type ──────────────────────────────────
        match agent_type_str.as_str() {
            "remote_codex" => {
                // Native in-process path: Codex uses CodexInProcessAdapter
                // with isolated storage (no bridge binary needed).
                let codex_adapters = Arc::clone(&state.active_codex_adapters);
                let pending_codex_permissions = Arc::clone(&state.pending_codex_permissions);
                let sid_clone = sid.clone();
                let prompt_owned = prompt.clone();
                let codex_options = codex_adapter_options_from_runtime(&config, &gui_settings);

                let handle = tokio::spawn(async move {
                    let result = run_codex_in_process_prompt(
                        &app,
                        &codex_adapters,
                        &pending_codex_permissions,
                        &sid_clone,
                        &prompt_owned,
                        codex_options,
                    )
                    .await;

                    if let Err(error) = result {
                        let _ = app.emit(
                            APP_EVENT_PROMPT_DONE,
                            PromptDoneDto {
                                session_id: sid_clone.clone(),
                                is_error: true,
                                error: Some(error),
                                result: None,
                            },
                        );
                    }

                    // Clean up the running-prompts map entry.
                    {
                        let mut running = running_prompts.lock().await;
                        running.remove(&sid_for_cleanup);
                    }
                });
                running.insert(sid.clone(), handle);
            }
            "remote_roo" => {
                // Native in-process path: Roo uses RooInProcessAdapter
                // (no bridge binary needed).
                let roo_adapters = Arc::clone(&state.active_roo_adapters);
                let sid_clone = sid.clone();
                let prompt_owned = prompt.clone();
                let working_dir = config.cwd.clone();
                let model = config.provider.model.clone();
                let api_key = config.provider.api_key.clone();
                let provider_name = config.provider.name.clone();

                let handle = tokio::spawn(async move {
                    let result = run_roo_in_process_prompt(
                        &app,
                        &roo_adapters,
                        &sid_clone,
                        &prompt_owned,
                        working_dir,
                        model,
                        api_key,
                        provider_name,
                    )
                    .await;

                    if let Err(error) = result {
                        let _ = app.emit(
                            APP_EVENT_PROMPT_DONE,
                            PromptDoneDto {
                                session_id: sid_clone.clone(),
                                is_error: true,
                                error: Some(error),
                                result: None,
                            },
                        );
                    }

                    // Clean up the running-prompts map entry.
                    {
                        let mut running = running_prompts.lock().await;
                        running.remove(&sid_for_cleanup);
                    }
                });
                running.insert(sid.clone(), handle);
            }
            _ => {
                // Default path: Claude Code Agent (remote_claude / empty) uses
                // the in-process QueryEngine.
                let handle = tokio::spawn(async move {
                    let result = query_engine_gui::run_unified_prompt_with_provider(
                        &app,
                        config.clone(),
                        provider,
                        session_store,
                        pending_permissions,
                        &prompt,
                    )
                    .await;

                    match result {
                        Ok(outcome) => {
                            let _ = app.emit(
                                APP_EVENT_PROMPT_DONE,
                                PromptDoneDto {
                                    session_id: config.session_id.to_string(),
                                    is_error: false,
                                    error: None,
                                    result: Some(PromptResultDto {
                                        session_id: config.session_id.to_string(),
                                        text: outcome.text,
                                        tool_calls: outcome
                                            .tool_calls
                                            .iter()
                                            .map(tool_call_to_dto)
                                            .collect(),
                                        usage: usage_to_dto(&outcome.usage),
                                        num_turns: outcome.num_turns,
                                        stop_reason: outcome.stop_reason,
                                    }),
                                },
                            );
                        }
                        Err(error) => {
                            let _ = app.emit(
                                APP_EVENT_PROMPT_DONE,
                                PromptDoneDto {
                                    session_id: config.session_id.to_string(),
                                    is_error: true,
                                    error: Some(format!("{error:#}")),
                                    result: None,
                                },
                            );
                        }
                    }

                    // Clean up the running-prompts map entry.
                    {
                        let mut running = running_prompts.lock().await;
                        running.remove(&sid_for_cleanup);
                    }
                });
                running.insert(sid.clone(), handle);
            }
        }
    }

    Ok(sid)
}

#[tauri::command]
async fn cancel_prompt(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<bool, String> {
    let mut running = state.running_prompts.lock().await;
    if let Some(handle) = running.remove(&session_id) {
        handle.abort();
        drop(running);
        let _ = app.emit(
            APP_EVENT_PROMPT_DONE,
            PromptDoneDto {
                session_id,
                is_error: true,
                error: Some("已取消".to_owned()),
                result: None,
            },
        );
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
async fn get_provider_info(
    state: State<'_, AppState>,
) -> std::result::Result<Option<ProviderInfoDto>, String> {
    let runtime = state.runtime.lock().await;
    Ok(Some(provider_info_from_runtime(&runtime.config.provider)))
}

#[tauri::command]
async fn get_runtime_status(
    state: State<'_, AppState>,
) -> std::result::Result<UiRuntimeStatusSnapshot, String> {
    let runtime = state.runtime.lock().await;
    Ok(runtime_status_snapshot_from_config(&runtime.config))
}

#[tauri::command]
async fn run_doctor_report(
    state: State<'_, AppState>,
    probe_network: bool,
    probe_provider: bool,
    probe_mcp: bool,
    include_env_providers: bool,
) -> std::result::Result<GuiDoctorReportDto, String> {
    let runtime = state.runtime.lock().await;
    build_gui_doctor_report(
        &runtime.config,
        probe_network,
        probe_provider,
        probe_mcp,
        include_env_providers,
    )
    .await
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn export_session_bundle(
    state: State<'_, AppState>,
    session_id: String,
    format: SessionExportFormatDto,
) -> std::result::Result<SessionExportResultDto, String> {
    let runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    export_session_bundle_for_store(&runtime.session_store, session_id, format)
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn list_mcp_servers(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    connect: bool,
    include_disabled: bool,
) -> std::result::Result<McpServerListDto, String> {
    let runtime = state.runtime.lock().await;
    build_mcp_server_list(
        &runtime.config,
        scope,
        project_path.as_deref(),
        connect,
        include_disabled,
    )
    .await
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn list_runtime_mcp_inventory(
    state: State<'_, AppState>,
    project_path: Option<String>,
    connect: bool,
    include_disabled: bool,
) -> std::result::Result<RuntimeMcpInventoryDto, String> {
    let runtime = state.runtime.lock().await;
    build_runtime_mcp_inventory(
        &runtime.config,
        project_path.as_deref(),
        connect,
        include_disabled,
    )
    .await
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn save_mcp_server(
    state: State<'_, AppState>,
    request: McpServerUpsertRequestDto,
) -> std::result::Result<McpMutationResultDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(
        &runtime.config,
        request.scope,
        request.project_path.as_deref(),
    )
    .map_err(|error| format!("{error:#}"))?;
    save_managed_mcp_server_at_path(&config_path, request.scope, &request)
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn toggle_mcp_server(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    name: String,
    enabled: bool,
    if_exists: bool,
) -> std::result::Result<McpMutationResultDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(&runtime.config, scope, project_path.as_deref())
        .map_err(|error| format!("{error:#}"))?;
    toggle_managed_mcp_server_at_path(&config_path, scope, &name, enabled, if_exists)
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn remove_mcp_server(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    name: String,
    if_exists: bool,
) -> std::result::Result<McpMutationResultDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(&runtime.config, scope, project_path.as_deref())
        .map_err(|error| format!("{error:#}"))?;
    remove_managed_mcp_server_at_path(&config_path, scope, &name, if_exists)
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn reset_mcp_servers(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    if_exists: bool,
) -> std::result::Result<McpMutationResultDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(&runtime.config, scope, project_path.as_deref())
        .map_err(|error| format!("{error:#}"))?;
    reset_managed_mcp_config_at_path(&config_path, scope, if_exists)
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> std::result::Result<FullSettingsDto, String> {
    let runtime = state.runtime.lock().await;
    Ok(full_settings_from_runtime(
        &runtime.config,
        &runtime.gui_settings,
    ))
}

#[tauri::command]
async fn codex_list_threads(
    state: State<'_, AppState>,
    params: Option<CodexThreadListRequest>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_threads(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_read_thread(
    state: State<'_, AppState>,
    request: CodexThreadRefRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .read_thread(request.thread_id, request.include_turns)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_codex_params::<ThreadStartParams>(request.params, ThreadStartParams::default)
            .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_thread_with_params(Some(params)).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_resume_thread(
    state: State<'_, AppState>,
    request: CodexThreadRefRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .resume_thread(request.thread_id, request.include_turns)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_resume_thread_native(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadResumeParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.resume_thread_with_params(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fork_thread(
    state: State<'_, AppState>,
    request: CodexThreadRefRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .fork_thread(request.thread_id, request.include_turns)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fork_thread_native(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadForkParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fork_thread_with_params(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_archive_thread(
    state: State<'_, AppState>,
    request: CodexThreadArchiveRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.archive_thread(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_unarchive_thread(
    state: State<'_, AppState>,
    request: CodexThreadArchiveRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.unarchive_thread(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_unsubscribe(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.unsubscribe_thread(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_elicitation_increment(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .increment_thread_elicitation(request.thread_id)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_elicitation_decrement(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .decrement_thread_elicitation(request.thread_id)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_set_name(
    state: State<'_, AppState>,
    request: CodexThreadSetNameRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .set_thread_name(request.thread_id, request.name)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_metadata_update(
    state: State<'_, AppState>,
    request: CodexThreadMetadataUpdateRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = ThreadMetadataUpdateParams {
        thread_id: request.thread_id,
        git_info: Some(ThreadMetadataGitInfoUpdateParams {
            sha: request.sha,
            branch: request.branch,
            origin_url: request.origin_url,
        }),
    };
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.update_thread_metadata(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_goal_set(
    state: State<'_, AppState>,
    request: CodexThreadGoalSetUiRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .set_thread_goal(CodexThreadGoalSetRequest {
                        thread_id: request.thread_id,
                        text: request.text,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_goal_get(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.get_thread_goal(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_goal_clear(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.clear_thread_goal(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_compact_start(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.compact_thread(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_shell_command(
    state: State<'_, AppState>,
    request: CodexThreadShellCommandRequest,
) -> std::result::Result<serde_json::Value, String> {
    if request.command.trim().is_empty() {
        return Err("Codex thread shell command cannot be empty".to_owned());
    }
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .run_thread_shell_command(request.thread_id, request.command)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_background_terminals_clean(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .clean_thread_background_terminals(request.thread_id)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_guardian_denied_action_approve(
    state: State<'_, AppState>,
    request: CodexThreadNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let mut params =
        decode_required_codex_params::<ThreadApproveGuardianDeniedActionParams>(request.params)
            .map_err(|error| format!("{error:#}"))?;
    if params.thread_id.trim().is_empty() {
        params.thread_id = request.thread_id;
    }
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.approve_guardian_denied_action(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_rollback(
    state: State<'_, AppState>,
    request: CodexThreadRollbackUiRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .rollback_thread(CodexThreadRollbackRequest {
                        thread_id: request.thread_id,
                        num_turns: request.num_turns,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_turns_list(
    state: State<'_, AppState>,
    request: CodexThreadTurnsListRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .list_thread_turns(request.thread_id, request.cursor, request.limit)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_loaded_list(
    state: State<'_, AppState>,
    request: CodexThreadLoadedListRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .list_loaded_threads(request.cursor, request.limit)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_thread_inject_items(
    state: State<'_, AppState>,
    request: CodexThreadNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let mut params = decode_required_codex_params::<ThreadInjectItemsParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    if params.thread_id.trim().is_empty() {
        params.thread_id = request.thread_id;
    }
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.inject_thread_items(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_turn_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<TurnStartParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_turn(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_turn_steer(
    state: State<'_, AppState>,
    request: CodexTurnSteerUiRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .steer_turn(CodexTurnSteerRequest {
                        thread_id: request.thread_id,
                        expected_turn_id: request.expected_turn_id,
                        message: request.message,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_turn_interrupt(
    state: State<'_, AppState>,
    request: CodexTurnInterruptUiRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .interrupt_turn(CodexTurnInterruptRequest {
                        thread_id: request.thread_id,
                        turn_id: request.turn_id,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_model_list(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_models(Some(true)).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_collaboration_mode_list(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_collaboration_modes().await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_experimental_feature_list(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_experimental_features().await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_experimental_feature_set(
    state: State<'_, AppState>,
    request: CodexExperimentalFeatureSetRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .set_experimental_feature(request.feature, request.enabled)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_account_read(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_account().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_account_login(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<LoginAccountParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.login_account(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_account_login_cancel(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<CancelLoginAccountParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.cancel_login_account(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_account_logout(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.logout_account().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_account_rate_limits_read(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_account_rate_limits().await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_account_add_credits_nudge(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<SendAddCreditsNudgeEmailParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.send_add_credits_nudge_email(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_apps_list(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_apps().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_exec(
    state: State<'_, AppState>,
    request: CodexExecRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.exec_command(request).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_app_server_request(
    state: State<'_, AppState>,
    request: CodexAppServerRequest,
) -> std::result::Result<serde_json::Value, String> {
    if request.method.trim().is_empty() {
        return Err("Codex app-server method cannot be empty".to_owned());
    }
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            adapter
                .app_server_request(request.method, request.params)
                .await
        })
    })
    .await
}

#[tauri::command]
async fn codex_exec_write(
    state: State<'_, AppState>,
    request: CodexExecWriteRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .exec_write(CommandExecWriteParams {
                        process_id: request.process_id,
                        delta_base64: request.delta_base64,
                        close_stdin: request.close_stdin,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_exec_terminate(
    state: State<'_, AppState>,
    session_id: Option<String>,
    process_id: String,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.exec_terminate(process_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_exec_resize(
    state: State<'_, AppState>,
    request: CodexExecResizeRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .exec_resize(CommandExecResizeParams {
                        process_id: request.process_id,
                        size: CommandExecTerminalSize {
                            rows: request.rows,
                            cols: request.cols,
                        },
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_windows_sandbox_setup_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<WindowsSandboxSetupStartParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_windows_sandbox_setup(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_mcp_refresh(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.refresh_mcp().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_mcp_status(
    state: State<'_, AppState>,
    request: CodexMcpStatusRequest,
) -> std::result::Result<serde_json::Value, String> {
    let detail = parse_codex_mcp_detail(request.detail.as_deref());
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .list_mcp_status(detail, request.cursor, request.limit)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_mcp_read_resource(
    state: State<'_, AppState>,
    request: CodexMcpResourceReadRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .read_mcp_resource(request.server, request.uri)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_mcp_call_tool(
    state: State<'_, AppState>,
    request: CodexMcpToolCallRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .call_mcp_tool(
                        request.thread_id,
                        request.server,
                        request.tool,
                        request.arguments,
                        request.meta,
                    )
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_mcp_oauth_login(
    state: State<'_, AppState>,
    request: CodexMcpOAuthLoginRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.mcp_oauth_login(request.server).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_skills_list(
    state: State<'_, AppState>,
    request: CodexSkillsListRequest,
) -> std::result::Result<serde_json::Value, String> {
    let cwds = request
        .cwds
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_skills(cwds, request.force_reload).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_skills_config_write(
    state: State<'_, AppState>,
    request: CodexSkillsConfigWriteRequest,
) -> std::result::Result<serde_json::Value, String> {
    let path_candidate = PathBuf::from(&request.skill_id);
    let (path, name) = if path_candidate.components().count() > 1 || path_candidate.is_absolute() {
        (Some(path_candidate), None)
    } else {
        (None, Some(request.skill_id))
    };
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .write_skills_config(path, name, request.enabled)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_plugin_list(
    state: State<'_, AppState>,
    request: CodexPluginListRequest,
) -> std::result::Result<serde_json::Value, String> {
    let cwds = request
        .cwds
        .map(|paths| paths.into_iter().map(PathBuf::from).collect());
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_plugins(cwds).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_plugin_read(
    state: State<'_, AppState>,
    request: CodexPluginIdRequest,
) -> std::result::Result<serde_json::Value, String> {
    let request = parse_codex_plugin_ref(request.plugin_id);
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_plugin(request).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_plugin_install(
    state: State<'_, AppState>,
    request: CodexPluginInstallRequest,
) -> std::result::Result<serde_json::Value, String> {
    let request = parse_codex_plugin_ref(request.source);
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.install_plugin(request).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_plugin_uninstall(
    state: State<'_, AppState>,
    request: CodexPluginIdRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.uninstall_plugin(request.plugin_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_marketplace_add(
    state: State<'_, AppState>,
    request: CodexMarketplaceRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.add_marketplace(request.source).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_marketplace_remove(
    state: State<'_, AppState>,
    request: CodexMarketplaceRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.remove_marketplace(request.source).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_marketplace_upgrade(
    state: State<'_, AppState>,
    request: CodexMarketplaceRequest,
) -> std::result::Result<serde_json::Value, String> {
    let marketplace_name = trimmed_option(Some(request.source));
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.upgrade_marketplace(marketplace_name).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_review_start(
    state: State<'_, AppState>,
    request: CodexReviewStartRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .start_review(request.thread_id, request.prompt)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_read_config(
    state: State<'_, AppState>,
    include_layers: Option<bool>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_config(include_layers.unwrap_or(false)).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_config_requirements_read(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_config_requirements().await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_external_agent_config_detect(
    state: State<'_, AppState>,
    request: CodexExternalAgentConfigDetectRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = ExternalAgentConfigDetectParams {
        include_home: request.include_home,
        cwds: request
            .cwds
            .map(|paths| paths.into_iter().map(PathBuf::from).collect()),
    };
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.detect_external_agent_config(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_external_agent_config_import(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ExternalAgentConfigImportParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.import_external_agent_config(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_write_config_value(
    state: State<'_, AppState>,
    request: CodexConfigValueWriteRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .write_config_value(ConfigValueWriteParams {
                        key_path: request.key_path,
                        value: request.value,
                        merge_strategy: parse_codex_merge_strategy(
                            request.merge_strategy.as_deref(),
                        ),
                        file_path: request.file_path,
                        expected_version: request.expected_version,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_write_config_batch(
    state: State<'_, AppState>,
    request: CodexConfigBatchWriteRequest,
) -> std::result::Result<serde_json::Value, String> {
    let edits = request
        .edits
        .into_iter()
        .map(|edit| ConfigEdit {
            key_path: edit.key_path,
            value: edit.value,
            merge_strategy: parse_codex_merge_strategy(edit.merge_strategy.as_deref()),
        })
        .collect();
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .write_config_batch(ConfigBatchWriteParams {
                        edits,
                        file_path: request.file_path,
                        expected_version: request.expected_version,
                        reload_user_config: request.reload_user_config,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_upload_feedback(
    state: State<'_, AppState>,
    request: CodexFeedbackRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.upload_feedback(request).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_set_thread_memory_mode(
    state: State<'_, AppState>,
    request: CodexMemoryModeRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .set_thread_memory_mode(request.thread_id, request.enabled)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_reset_memories(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.reset_memories().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_realtime_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadRealtimeStartParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_realtime(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_realtime_append_audio(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadRealtimeAppendAudioParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.append_realtime_audio(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_realtime_append_text(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadRealtimeAppendTextParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.append_realtime_text(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_realtime_stop(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadRealtimeStopParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.stop_realtime(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_realtime_voices_list(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_realtime_voices().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_device_key_create(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<DeviceKeyCreateParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.create_device_key(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_device_key_public(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<DeviceKeyPublicParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.get_device_key_public(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_device_key_sign(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<DeviceKeySignParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.sign_device_key(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_read_file(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsReadFileParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_read_file(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_write_file(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsWriteFileParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_write_file(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_create_directory(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsCreateDirectoryParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_create_directory(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_get_metadata(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsGetMetadataParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_get_metadata(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_read_directory(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsReadDirectoryParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_read_directory(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_remove(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsRemoveParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_remove(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_copy(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsCopyParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_copy(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_watch(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsWatchParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_watch(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fs_unwatch(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsUnwatchParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_unwatch(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fuzzy_file_search(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FuzzyFileSearchParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fuzzy_file_search(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fuzzy_file_search_session_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FuzzyFileSearchSessionStartParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_fuzzy_file_search_session(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fuzzy_file_search_session_update(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FuzzyFileSearchSessionUpdateParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.update_fuzzy_file_search_session(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_fuzzy_file_search_session_stop(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FuzzyFileSearchSessionStopParams>(request.params)
        .map_err(|error| format!("{error:#}"))?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.stop_fuzzy_file_search_session(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
async fn codex_adapter_stop(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<(), String> {
    let key = codex_adapter_key(session_id);
    let mut adapters = state.active_codex_adapters.lock().await;
    if let Some(mut adapter) = adapters.remove(&key) {
        adapter.stop().await.map_err(|error| format!("{error:#}"))?;
    }
    Ok(())
}

#[tauri::command]
async fn codex_adapter_restart(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<(), String> {
    let key = codex_adapter_key(session_id);
    {
        let mut adapters = state.active_codex_adapters.lock().await;
        if let Some(mut adapter) = adapters.remove(&key) {
            let _ = adapter.stop().await;
        }
    }
    ensure_codex_adapter(&state, &key).await
}

#[tauri::command]
async fn update_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    request: UpdateProviderRequest,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;

    if let Some(provider_name) = request.provider_name.or(request.name) {
        let provider_name = provider_name.trim().to_owned();
        if let Some(index) = find_provider_config_index(&runtime.provider_configs, &provider_name) {
            let stored = runtime.provider_configs.providers[index].clone();
            runtime.config.provider = provider_config_to_runtime(&stored, &runtime.config.provider)
                .map_err(|error| format!("{error:#}"))?;
            runtime.provider_configs.active_provider = Some(provider_name.clone());
        } else if !provider_name.is_empty() {
            runtime.config.provider.name = provider_name.clone();
        }
        runtime.gui_settings.provider_name = Some(runtime.config.provider.name.clone());
    }

    if let Some(model) = request.provider_model.or(request.model) {
        let model = model.trim().to_owned();
        runtime.config.provider.model = if model.is_empty() {
            None
        } else {
            Some(model.clone())
        };
        runtime.gui_settings.provider_model = runtime.config.provider.model.clone();
    }

    if let Some(protocol) = parse_protocol(
        request
            .provider_protocol
            .as_deref()
            .or(request.protocol.as_deref()),
    ) {
        runtime.config.provider.protocol = protocol;
        runtime.config.provider.base_url =
            normalize_base_url(runtime.config.provider.base_url.clone(), protocol);
        runtime.gui_settings.provider_protocol = Some(protocol.as_str().to_owned());
    }

    if let Some(base_url) = request.provider_base_url.or(request.base_url) {
        runtime.config.provider.base_url =
            normalize_base_url(Some(base_url.clone()), runtime.config.provider.protocol);
        runtime.gui_settings.provider_base_url = runtime.config.provider.base_url.clone();
    }

    if let Some(api_key) = request.api_key {
        runtime.config.provider.api_key = trimmed_option(Some(api_key));
    }
    if let Some(max_output_tokens) = request.max_output_tokens {
        runtime.config.provider.max_output_tokens = max_output_tokens.max(256);
        runtime.gui_settings.max_output_tokens = Some(runtime.config.provider.max_output_tokens);
    }
    if let Some(thinking_budget) = request.thinking_budget {
        runtime.config.provider.thinking_budget = thinking_budget;
        runtime.gui_settings.thinking_budget = Some(thinking_budget);
    }
    if let Some(max_retries) = request.max_retries {
        runtime.config.provider.max_retries = max_retries;
        runtime.gui_settings.max_retries = Some(max_retries);
    }
    if let Some(timeout_ms) = request.timeout_ms {
        runtime.config.provider.timeout_ms = timeout_ms.max(1_000);
        runtime.gui_settings.timeout_ms = Some(runtime.config.provider.timeout_ms);
    }
    if let Some(backoff_ms) = request.retry_initial_backoff_ms {
        runtime.config.provider.retry_initial_backoff_ms = backoff_ms.max(50);
        runtime.gui_settings.retry_initial_backoff_ms =
            Some(runtime.config.provider.retry_initial_backoff_ms);
    }
    if let Some(backoff_ms) = request.retry_max_backoff_ms {
        runtime.config.provider.retry_max_backoff_ms =
            backoff_ms.max(runtime.config.provider.retry_initial_backoff_ms);
        runtime.gui_settings.retry_max_backoff_ms =
            Some(runtime.config.provider.retry_max_backoff_ms);
    }
    if let Some(respect_retry_after) = request.respect_retry_after {
        runtime.config.provider.respect_retry_after = respect_retry_after;
        runtime.gui_settings.respect_retry_after = Some(respect_retry_after);
    }
    if let Some(permission_mode) = parse_permission_mode(request.permission_mode.as_deref()) {
        runtime.config.permission_mode = permission_mode;
        runtime.gui_settings.permission_mode = Some(permission_mode.as_legacy_str().to_owned());
    }
    if let Some(verbose) = request.verbose {
        runtime.config.verbose = verbose;
        runtime.gui_settings.verbose = Some(verbose);
    }
    if request.codex_model_provider.is_some() {
        runtime.gui_settings.codex_model_provider = request
            .codex_model_provider
            .and_then(|value| trimmed_option(Some(value)));
    }
    if request.codex_approval_policy.is_some() {
        runtime.gui_settings.codex_approval_policy = request
            .codex_approval_policy
            .and_then(|value| trimmed_option(Some(value)));
    }
    if request.codex_sandbox_mode.is_some() {
        runtime.gui_settings.codex_sandbox_mode = request
            .codex_sandbox_mode
            .and_then(|value| trimmed_option(Some(value)));
    }
    if let Some(value) = request.codex_persist_extended_history {
        runtime.gui_settings.codex_persist_extended_history = Some(value);
    }
    if let Some(value) = request.codex_memories_enabled {
        runtime.gui_settings.codex_memories_enabled = Some(value);
    }
    if request.codex_thread_store_endpoint.is_some() {
        runtime.gui_settings.codex_thread_store_endpoint = request
            .codex_thread_store_endpoint
            .and_then(|value| trimmed_option(Some(value)));
    }
    if let Some(overrides) = request.codex_config_overrides {
        runtime.gui_settings.codex_config_overrides = overrides;
    }
    if let Some(profile) = request.codex_permission_profile {
        runtime.gui_settings.codex_permission_profile = Some(profile);
    }
    if let Some(tier) = request.codex_service_tier {
        runtime.gui_settings.codex_service_tier = trimmed_option(Some(tier));
    }
    if let Some(ephemeral) = request.codex_ephemeral {
        runtime.gui_settings.codex_ephemeral = Some(ephemeral);
    }

    if let Some(thinking_budget) = runtime.config.provider.thinking_budget {
        if thinking_budget >= runtime.config.provider.max_output_tokens {
            return Err("thinking budget must be lower than max output tokens".to_owned());
        }
    }

    let selected_provider = runtime.config.provider.clone();
    store_provider_selection(&mut runtime, &selected_provider);
    persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
async fn list_projects(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<ProjectInfoDto>, String> {
    let runtime = state.runtime.lock().await;
    let sessions = runtime
        .session_store
        .list_active_sessions()
        .map_err(|error| format!("{error:#}"))?;
    Ok(build_project_infos(&runtime.projects, &sessions))
}

#[tauri::command]
async fn add_project(
    state: State<'_, AppState>,
    path: String,
) -> std::result::Result<ProjectInfoDto, String> {
    let mut runtime = state.runtime.lock().await;
    let path = normalize_existing_path(Path::new(&path)).map_err(|error| format!("{error:#}"))?;
    if !path.exists() || !path.is_dir() {
        return Err(format!("project path does not exist: {}", path.display()));
    }
    if !runtime
        .projects
        .iter()
        .any(|project| path_identity(&project.path) == path_identity(&path))
    {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project")
            .to_owned();
        runtime.projects.push(ProjectEntry {
            path: path.clone(),
            name: name.clone(),
        });
        runtime
            .projects
            .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
        return Ok(ProjectInfoDto {
            path: path.display().to_string(),
            name,
            session_count: project_session_count(
                &path,
                &runtime
                    .session_store
                    .list_active_sessions()
                    .map_err(|error| format!("{error:#}"))?,
            ),
            is_auto_detected: false,
        });
    }
    let project = runtime
        .projects
        .iter()
        .find(|project| project.path == path)
        .ok_or_else(|| "failed to load project".to_owned())?;
    Ok(ProjectInfoDto {
        path: project.path.display().to_string(),
        name: project.name.clone(),
        session_count: 0,
        is_auto_detected: false,
    })
}

#[tauri::command]
async fn remove_project(
    state: State<'_, AppState>,
    path: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let path = normalize_existing_path(Path::new(&path)).unwrap_or_else(|_| PathBuf::from(path));
    let sessions = runtime
        .session_store
        .list_active_sessions()
        .map_err(|error| format!("{error:#}"))?;
    if project_session_count(&path, &sessions) > 0 {
        return Err("该项目下仍有会话，不能移除项目文件夹。".to_owned());
    }
    let path_key = path_identity(&path);
    runtime
        .projects
        .retain(|project| path_identity(&project.path) != path_key);
    persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
    Ok(())
}

#[tauri::command]
async fn archive_session(
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<(), String> {
    let runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    runtime
        .session_store
        .set_archived(session_id, true)
        .map_err(|error| format!("{error:#}"))?;
    Ok(())
}

#[tauri::command]
async fn restore_session(
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let summary = runtime
        .session_store
        .get_session_summary(session_id)
        .map_err(|error| format!("{error:#}"))?;
    runtime
        .session_store
        .set_archived(session_id, false)
        .map_err(|error| format!("{error:#}"))?;
    if ensure_project_entry(&mut runtime.projects, &summary.cwd) {
        persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
    }
    Ok(())
}

#[tauri::command]
async fn list_provider_configs(
    state: State<'_, AppState>,
) -> std::result::Result<ProviderConfigList, String> {
    let runtime = state.runtime.lock().await;
    let mut result = runtime.provider_configs.clone();
    // For each provider, set api_key_stored and mask api_key.
    for provider in &mut result.providers {
        let in_keychain = keyring_retrieve(&provider.name).is_some();
        let in_json = provider.api_key.is_some();
        provider.api_key_stored = in_keychain || in_json;
        // Never expose API keys to the frontend — mask to None.
        provider.api_key = None;
    }
    Ok(result)
}

#[tauri::command]
async fn save_provider_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: ProviderConfig,
    set_active: bool,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let mut config = normalize_provider_config(config).map_err(|error| format!("{error:#}"))?;

    // Store API key in OS keychain if provided; clear from JSON payload.
    if let Some(ref api_key) = config.api_key {
        keyring_store(&config.name, api_key);
        config.api_key = None;
    }
    // If api_key was None (frontend didn't change it), keep existing keychain entry.

    let index = find_provider_config_index(&runtime.provider_configs, &config.name);
    if let Some(index) = index {
        runtime.provider_configs.providers[index] = config.clone();
    } else {
        runtime.provider_configs.providers.push(config.clone());
        runtime
            .provider_configs
            .providers
            .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    }

    if set_active || runtime.provider_configs.active_provider.is_none() {
        runtime.provider_configs.active_provider = Some(config.name.clone());
        runtime.config.provider = provider_config_to_runtime(&config, &runtime.config.provider)
            .map_err(|error| format!("{error:#}"))?;
        let provider_configs_snapshot = runtime.provider_configs.clone();
        apply_provider_credentials_from_configs(
            &mut runtime.config.provider,
            &provider_configs_snapshot,
        );
        let selected_provider = runtime.config.provider.clone();
        store_provider_selection(&mut runtime, &selected_provider);
    }

    persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
async fn delete_provider_config(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;

    // Verify the provider exists before deleting the keychain entry.
    let exists = runtime
        .provider_configs
        .providers
        .iter()
        .any(|provider| provider.name == name);
    if !exists {
        return Err(format!("unknown provider config: {name}"));
    }

    // Remove API key from OS keychain only after confirming the provider exists.
    keyring_delete(&name);

    let removed_active = runtime.provider_configs.active_provider.as_deref() == Some(name.as_str());
    runtime
        .provider_configs
        .providers
        .retain(|provider| provider.name != name);

    if removed_active {
        runtime.provider_configs.active_provider = runtime
            .provider_configs
            .providers
            .first()
            .map(|provider| provider.name.clone());
        if let Some(active) = active_provider_config(&runtime.provider_configs).cloned() {
            runtime.config.provider = provider_config_to_runtime(&active, &runtime.config.provider)
                .map_err(|error| format!("{error:#}"))?;
            let selected_provider = runtime.config.provider.clone();
            store_provider_selection(&mut runtime, &selected_provider);
        } else {
            runtime.gui_settings.provider_name = None;
            runtime.gui_settings.provider_model = None;
            runtime.gui_settings.provider_base_url = None;
            runtime.gui_settings.provider_protocol = None;
            let mut fresh = load_base_runtime_config(profile_override_from_env())
                .map_err(|error| format!("{error:#}"))?;
            apply_gui_settings_to_runtime(&mut fresh, &runtime.gui_settings)
                .map_err(|error| format!("{error:#}"))?;
            runtime.config = fresh;
        }
    }

    persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
async fn set_active_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &name)
        .ok_or_else(|| format!("unknown provider config: {name}"))?;
    let config = runtime.provider_configs.providers[index].clone();
    runtime.provider_configs.active_provider = Some(config.name.clone());
    runtime.config.provider = provider_config_to_runtime(&config, &runtime.config.provider)
        .map_err(|error| format!("{error:#}"))?;
    let provider_configs_snapshot = runtime.provider_configs.clone();
    apply_provider_credentials_from_configs(
        &mut runtime.config.provider,
        &provider_configs_snapshot,
    );
    let selected_provider = runtime.config.provider.clone();
    store_provider_selection(&mut runtime, &selected_provider);
    persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
async fn switch_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_name: String,
    profile_name: Option<String>,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &provider_name)
        .ok_or_else(|| format!("unknown provider config: {provider_name}"))?;

    // Validate profile exists if specified.
    if let Some(ref pname) = profile_name {
        let config = &runtime.provider_configs.providers[index];
        if !config.profiles.iter().any(|p| p.name == *pname) {
            return Err(format!("unknown profile: {pname}"));
        }
    }

    runtime.provider_configs.providers[index].active_profile = profile_name;
    let config = runtime.provider_configs.providers[index].clone();

    // If this is the active provider, re-apply to runtime.
    if runtime.provider_configs.active_provider.as_deref() == Some(&provider_name) {
        runtime.config.provider = provider_config_to_runtime(&config, &runtime.config.provider)
            .map_err(|error| format!("{error:#}"))?;
        let provider_configs_snapshot = runtime.provider_configs.clone();
        apply_provider_credentials_from_configs(
            &mut runtime.config.provider,
            &provider_configs_snapshot,
        );
        let selected_provider = runtime.config.provider.clone();
        store_provider_selection(&mut runtime, &selected_provider);
    }

    persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn resolve_permission_request(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    allowed: bool,
    message: Option<String>,
    updated_input: Option<serde_json::Value>,
    permission_updates: Option<Vec<claude_permissions::PermissionUpdate>>,
    feedback: Option<String>,
    content_blocks: Option<Vec<serde_json::Value>>,
    codex_response: Option<serde_json::Value>,
    allow_all: Option<bool>,
) -> std::result::Result<bool, String> {
    let mut decision = if allowed {
        PermissionDecision::allow()
    } else {
        PermissionDecision::deny(
            message
                .clone()
                .unwrap_or_else(|| "Permission denied by GUI.".to_owned()),
        )
    };
    if allowed {
        decision.message = message.clone();
    }
    decision.updated_input = updated_input.clone();
    decision.permission_updates = permission_updates.clone().unwrap_or_default();
    decision.feedback = feedback.clone();
    decision.content_blocks = content_blocks.clone().unwrap_or_default();

    let sender = {
        let mut pending = state.pending_permissions.lock().await;
        pending.remove(&request_id)
    };
    if let Some(sender) = sender {
        let _ = sender.send(decision);
        Ok(true)
    } else {
        let pending_codex = {
            let pending = state.pending_codex_permissions.lock().await;
            pending.get(&request_id).cloned()
        };
        let Some(pending_codex) = pending_codex else {
            return Ok(false);
        };

        let permission_updates_for_emit = permission_updates.unwrap_or_default();
        let codex_decision = if allow_all.unwrap_or(false) && allowed {
            AgentPermissionDecision::AllowAll
        } else {
            codex_permission_decision(allowed, &permission_updates_for_emit)
        };

        {
            let mut adapters = state.active_codex_adapters.lock().await;
            let adapter = adapters
                .get_mut(&pending_codex.session_id)
                .ok_or_else(|| "Codex adapter not found for permission request".to_owned())?;
            adapter
                .resolve_codex_server_request(
                    &pending_codex.request_id,
                    codex_decision,
                    CodexServerRequestResolution {
                        allow_all: allow_all.unwrap_or(false),
                        response: codex_response,
                    },
                )
                .await
                .map_err(|error| {
                    format!("Failed to resolve Codex permission request: {error:#}")
                })?;
        }

        {
            let mut pending = state.pending_codex_permissions.lock().await;
            pending.remove(&request_id);
        }

        let _ = app.emit(
            APP_EVENT_PERMISSION_RESOLVED,
            PermissionDecisionDto {
                request_id,
                allowed,
                message,
                updated_input,
                permission_updates: permission_updates_for_emit,
                feedback,
                content_blocks: content_blocks.unwrap_or_default(),
            },
        );

        Ok(true)
    }
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> std::result::Result<Option<String>, String> {
    let picked = app.dialog().file().blocking_pick_folder();
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|error| error.to_string())?;
    Ok(Some(path.display().to_string()))
}

// ---------------------------------------------------------------------------
// Agent binary management helpers
// ---------------------------------------------------------------------------

/// Base directory for agent installations: `~/.remote-code/agents/`.
fn agents_base_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".remote-code").join("agents"))
        .unwrap_or_else(|| PathBuf::from(".remote-code").join("agents"))
}

/// Directory name for a given agent type (matches serde serialization).
fn agent_type_dir_name(agent_type: &ProtocolAgentType) -> &'static str {
    match agent_type {
        ProtocolAgentType::RemoteClaude => "remote_claude",
        ProtocolAgentType::RemoteRoo => "remote_roo",
        ProtocolAgentType::RemoteCodex => "remote_codex",
    }
}

/// Expected binary name for a given agent type.
fn agent_binary_name(agent_type: &ProtocolAgentType) -> String {
    let name = match agent_type {
        ProtocolAgentType::RemoteClaude => "remote-claude",
        ProtocolAgentType::RemoteRoo => "remote-roo",
        ProtocolAgentType::RemoteCodex => "remote-codex",
    };
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

#[tauri::command]
async fn list_available_agents(
    _state: State<'_, AppState>,
) -> std::result::Result<Vec<AgentTypeInfoDto>, String> {
    let specs: [(ProtocolAgentType, &str); 3] = [
        (ProtocolAgentType::RemoteClaude, "Remote Claude"),
        (ProtocolAgentType::RemoteRoo, "Remote Roo"),
        (ProtocolAgentType::RemoteCodex, "Remote Codex"),
    ];
    let agents = specs
        .iter()
        .map(|(agent_type, display_name)| {
            // All agents are now in-process — no external binary required.
            // RemoteClaude was always built-in; RemoteRoo and RemoteCodex are
            // now also built-in via callback-based in-process adapters.
            let installed = true;
            AgentTypeInfoDto {
                agent_type: agent_type_dir_name(agent_type).to_owned(),
                display_name: display_name.to_string(),
                available: true,
                installed,
            }
        })
        .collect();
    Ok(agents)
}

#[tauri::command]
async fn install_agent(agent_type: String) -> std::result::Result<(), String> {
    let parsed: ProtocolAgentType = serde_json::from_str(&format!("\"{}\"", agent_type))
        .map_err(|e| format!("无效的 agent_type: {e}"))?;

    // All agents are now in-process — no external binary to install.
    // RemoteClaude was always built-in; RemoteRoo and RemoteCodex are now
    // also built-in via callback-based in-process adapters.
    if matches!(parsed, ProtocolAgentType::RemoteClaude)
        || matches!(parsed, ProtocolAgentType::RemoteRoo)
        || matches!(parsed, ProtocolAgentType::RemoteCodex)
    {
        return Ok(());
    }

    // Fix #8: append platform-specific suffix to download URLs.
    let platform_suffix = if cfg!(windows) {
        "-windows.exe"
    } else if cfg!(target_os = "macos") {
        "-macos"
    } else {
        "-linux"
    };

    // Attempt to download from a configurable URL.
    let env_key = format!("REMOTE_CODE_{}_DOWNLOAD_URL", agent_type.to_uppercase());
    let download_url = env::var(&env_key).ok().or_else(|| match parsed {
        ProtocolAgentType::RemoteRoo => Some(format!(
            "https://github.com/roo-code/roo-code/releases/latest/download/roo-code-cli{platform_suffix}"
        )),
        ProtocolAgentType::RemoteCodex => Some(format!(
            "https://github.com/openai/codex/releases/latest/download/codex{platform_suffix}"
        )),
        _ => None,
    });

    let Some(url) = download_url else {
        return Err(format!(
            "未配置 {agent_type} 的下载地址。请手动安装到 ~/.remote-code/agents/{}/bin/",
            agent_type_dir_name(&parsed)
        ));
    };

    // Create installation directory.
    let install_dir = agents_base_dir()
        .join(agent_type_dir_name(&parsed))
        .join("bin");
    std::fs::create_dir_all(&install_dir).map_err(|e| format!("创建安装目录失败: {e}"))?;

    let target_path = install_dir.join(agent_binary_name(&parsed));

    // Download.
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("下载失败: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("下载失败: HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取下载内容失败: {e}"))?;

    // Validate: must be non-empty.
    if bytes.is_empty() {
        return Err("下载内容为空".to_owned());
    }

    std::fs::write(&target_path, &bytes).map_err(|e| format!("写入文件失败: {e}"))?;

    // Set executable permissions on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("设置执行权限失败: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
async fn uninstall_agent(
    _state: State<'_, AppState>,
    agent_type: String,
) -> std::result::Result<(), String> {
    let _parsed: ProtocolAgentType = serde_json::from_str(&format!("\"{}\"", agent_type))
        .map_err(|e| format!("无效的 agent_type: {e}"))?;

    // All agents are now built-in (in-process) — cannot uninstall any of them.
    Err("所有 Agent 均为内置进程内适配器，无法卸载".to_owned())
}

#[tauri::command]
async fn transcribe_audio(
    _app: AppHandle,
    state: State<'_, AppState>,
    audio_data: Vec<u8>,
    audio_format: Option<String>,
) -> std::result::Result<String, String> {
    if audio_data.is_empty() {
        return Err("音频数据为空".to_owned());
    }

    // Obtain the API key from the active provider configuration.
    let api_key = {
        let runtime = state.runtime.lock().await;
        runtime.config.provider.api_key.clone()
    };

    let api_key = api_key.ok_or_else(|| "未配置 API key，无法使用语音转录".to_string())?;

    let stt = claude_voice::WhisperStt::new(api_key);
    let format = audio_format.as_deref().unwrap_or("webm");

    match stt.transcribe(&audio_data, format).await {
        Ok(result) => Ok(result.text),
        Err(e) => {
            tracing::error!("STT transcription failed: {e}");
            Err(format!("语音转录失败: {e}"))
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime_state = build_runtime_state().unwrap_or_else(|error| {
        panic!("failed to initialize remote-code-gui runtime: {error:#}");
    });
    let pending_permissions = Arc::new(Mutex::new(HashMap::new()));
    let pending_codex_permissions = Arc::new(Mutex::new(HashMap::new()));
    let running_prompts = Arc::new(Mutex::new(HashMap::new()));
    let active_codex_adapters = Arc::new(Mutex::new(HashMap::new()));
    let active_roo_adapters = Arc::new(Mutex::new(HashMap::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            mobile::register_mobile_plugins(app.handle());
            Ok(())
        })
        .manage(AppState {
            runtime: Mutex::new(runtime_state),
            pending_permissions,
            pending_codex_permissions,
            running_prompts,
            active_codex_adapters,
            active_roo_adapters,
        })
        .invoke_handler(tauri::generate_handler![
            init_app,
            list_sessions,
            get_session_conversation,
            get_session_tasks,
            send_prompt,
            cancel_prompt,
            get_provider_info,
            get_runtime_status,
            run_doctor_report,
            export_session_bundle,
            list_mcp_servers,
            list_runtime_mcp_inventory,
            save_mcp_server,
            toggle_mcp_server,
            remove_mcp_server,
            reset_mcp_servers,
            create_session,
            get_settings,
            codex_list_threads,
            codex_read_thread,
            codex_thread_start,
            codex_resume_thread,
            codex_resume_thread_native,
            codex_fork_thread,
            codex_fork_thread_native,
            codex_archive_thread,
            codex_unarchive_thread,
            codex_thread_unsubscribe,
            codex_thread_elicitation_increment,
            codex_thread_elicitation_decrement,
            codex_thread_set_name,
            codex_thread_metadata_update,
            codex_thread_goal_set,
            codex_thread_goal_get,
            codex_thread_goal_clear,
            codex_thread_compact_start,
            codex_thread_shell_command,
            codex_thread_background_terminals_clean,
            codex_thread_guardian_denied_action_approve,
            codex_thread_rollback,
            codex_thread_turns_list,
            codex_thread_loaded_list,
            codex_thread_inject_items,
            codex_turn_start,
            codex_turn_steer,
            codex_turn_interrupt,
            codex_model_list,
            codex_collaboration_mode_list,
            codex_experimental_feature_list,
            codex_experimental_feature_set,
            codex_account_read,
            codex_account_login,
            codex_account_login_cancel,
            codex_account_logout,
            codex_account_rate_limits_read,
            codex_account_add_credits_nudge,
            codex_apps_list,
            codex_exec,
            codex_app_server_request,
            codex_exec_write,
            codex_exec_terminate,
            codex_exec_resize,
            codex_windows_sandbox_setup_start,
            codex_mcp_refresh,
            codex_mcp_status,
            codex_mcp_read_resource,
            codex_mcp_call_tool,
            codex_mcp_oauth_login,
            codex_skills_list,
            codex_skills_config_write,
            codex_plugin_list,
            codex_plugin_read,
            codex_plugin_install,
            codex_plugin_uninstall,
            codex_marketplace_add,
            codex_marketplace_remove,
            codex_marketplace_upgrade,
            codex_review_start,
            codex_read_config,
            codex_config_requirements_read,
            codex_external_agent_config_detect,
            codex_external_agent_config_import,
            codex_write_config_value,
            codex_write_config_batch,
            codex_upload_feedback,
            codex_set_thread_memory_mode,
            codex_reset_memories,
            codex_realtime_start,
            codex_realtime_append_audio,
            codex_realtime_append_text,
            codex_realtime_stop,
            codex_realtime_voices_list,
            codex_device_key_create,
            codex_device_key_public,
            codex_device_key_sign,
            codex_fs_read_file,
            codex_fs_write_file,
            codex_fs_create_directory,
            codex_fs_get_metadata,
            codex_fs_read_directory,
            codex_fs_remove,
            codex_fs_copy,
            codex_fs_watch,
            codex_fs_unwatch,
            codex_fuzzy_file_search,
            codex_fuzzy_file_search_session_start,
            codex_fuzzy_file_search_session_update,
            codex_fuzzy_file_search_session_stop,
            codex_adapter_stop,
            codex_adapter_restart,
            update_provider,
            list_projects,
            add_project,
            remove_project,
            archive_session,
            restore_session,
            list_archived_sessions,
            list_provider_configs,
            save_provider_config,
            delete_provider_config,
            set_active_provider,
            switch_profile,
            resolve_permission_request,
            pick_folder,
            list_available_agents,
            install_agent,
            uninstall_agent,
            transcribe_audio,
            mobile::mobile_platform,
            mobile::mobile_is_mobile,
            mobile::mobile_haptic_impact,
            mobile::mobile_haptic_notification,
            mobile::mobile_haptic_selection,
            mobile::mobile_biometric_check_availability,
            mobile::mobile_biometric_authenticate,
            mobile::mobile_secure_store_get,
            mobile::mobile_secure_store_set,
            mobile::mobile_secure_store_remove,
            mobile::mobile_download_artifact,
            mobile::mobile_share_file
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| panic!("error while running tauri application: {error}"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;
    use uuid::Uuid;

    fn runtime_policy_test_mutex() -> &'static Mutex<()> {
        static RUNTIME_POLICY_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
        RUNTIME_POLICY_TEST_MUTEX.get_or_init(|| Mutex::new(()))
    }

    fn test_runtime_config(project_dir: &Path, profile_dir: &Path) -> RuntimeConfig {
        load_runtime_config(
            Some(project_dir.to_path_buf()),
            Some(profile_dir.to_path_buf()),
            None,
            PermissionMode::AcceptEdits,
            claude_core::InputFormat::Text,
            claude_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            8,
            ProviderOverrides {
                provider: Some("glm-coding".to_owned()),
                base_url: Some("https://open.bigmodel.cn/api/anthropic".to_owned()),
                api_key: Some("secret".to_owned()),
                model: Some("glm-5.1".to_owned()),
                protocol: Some(ProviderProtocol::Anthropic),
            },
            RuntimeOverrides {
                session_name: Some("Parity".to_owned()),
                system_prompt: None,
                append_system_prompt: None,
                settings_files: Vec::new(),
                show_setting_sources: true,
                allowed_setting_sources: None,
                allowed_tools: vec!["read_file".to_owned()],
                disallowed_tools: vec!["bash_command".to_owned()],
                structured_output_schema: None,
                mcp_config_paths: Vec::new(),
                strict_mcp_config: false,
                effort: Some("medium".to_owned()),
                fallback_model: Some("glm-5-turbo".to_owned()),
                output_style: Some("Explanatory".to_owned()),
                language: Some("Chinese".to_owned()),
                brief_enabled: Some(true),
                proactive_active: Some(true),
            },
        )
        .expect("config should load")
    }

    fn sample_stdio_mcp_server(name: &str, enabled: bool, command: &str) -> McpServerConfig {
        McpServerConfig {
            name: name.to_owned(),
            enabled,
            transport: McpTransportConfig::Stdio {
                command: command.to_owned(),
                args: vec!["--serve".to_owned()],
                cwd: None,
                env: BTreeMap::new(),
            },
            capabilities: claude_mcp::McpCapabilityMatrix::default(),
            startup_timeout_secs: Some(5),
            request_timeout_secs: Some(30),
            metadata: BTreeMap::new(),
            oauth: None,
        }
    }

    #[test]
    fn anthropic_base_url_is_normalized_from_gui_input() {
        let config = normalize_provider_config(ProviderConfig {
            name: "glm".to_owned(),
            protocol: "anthropic".to_owned(),
            base_url: Some("https://open.bigmodel.cn/api/anthropic".to_owned()),
            api_key: Some("secret".to_owned()),
            model: Some("glm-5.1".to_owned()),
            profiles: vec![],
            active_profile: None,
            api_key_stored: false,
        })
        .expect("provider config should normalize");

        assert_eq!(
            config.base_url.as_deref(),
            Some("https://open.bigmodel.cn/api/anthropic/v1/messages")
        );
    }

    #[test]
    fn permission_mode_aliases_map_to_runtime_modes() {
        assert_eq!(
            parse_permission_mode(Some("suggest")),
            Some(PermissionMode::Default)
        );
        assert_eq!(
            parse_permission_mode(Some("auto-edit")),
            Some(PermissionMode::AcceptEdits)
        );
        assert_eq!(
            parse_permission_mode(Some("full-auto")),
            Some(PermissionMode::BypassPermissions)
        );
        assert_eq!(
            parse_permission_mode(Some("yolo")),
            Some(PermissionMode::BypassPermissions)
        );
    }

    #[test]
    fn permission_request_dto_preserves_permission_suggestions() {
        let request = PermissionRequest {
            tool_name: "read_file".to_owned(),
            permission_class: Some(claude_permissions::PermissionClass::Read),
            tool_input: json!({"path": "..\\outside.txt"}),
            working_directory: None,
            tool_use_id: Some("tool-1".to_owned()),
            title: Some("Read outside workspace".to_owned()),
            description: Some("Read requires approval".to_owned()),
            blocked_path: Some("C:\\outside.txt".to_owned()),
            permission_suggestions: vec![json!({
                "action": "addRules",
                "toolPattern": "Read(C:\\outside.txt)",
            })],
        };

        let dto = permission_request_dto("request-1".to_owned(), &request);

        assert_eq!(dto.request_id, "request-1");
        assert_eq!(dto.tool_use_id, "tool-1");
        assert_eq!(dto.blocked_path.as_deref(), Some("C:\\outside.txt"));
        assert_eq!(dto.permission_suggestions.len(), 1);
        assert_eq!(dto.permission_suggestions[0]["action"], "addRules");
        assert_eq!(
            dto.permission_suggestions[0]["toolPattern"],
            "Read(C:\\outside.txt)"
        );
    }

    #[test]
    fn codex_permission_decision_preserves_session_scope() {
        let session_update = PermissionUpdate::SetMode {
            destination: PermissionUpdateDestination::Session,
            mode: claude_permissions::ExtendedPermissionMode::AcceptEdits,
        };
        let user_update = PermissionUpdate::SetMode {
            destination: PermissionUpdateDestination::UserSettings,
            mode: claude_permissions::ExtendedPermissionMode::AcceptEdits,
        };

        assert_eq!(
            codex_permission_decision(true, &[session_update]),
            AgentPermissionDecision::AllowAll
        );
        assert_eq!(
            codex_permission_decision(true, &[user_update]),
            AgentPermissionDecision::Allow
        );
        assert_eq!(
            codex_permission_decision(false, &[]),
            AgentPermissionDecision::Deny
        );
    }

    #[test]
    fn usage_info_from_codex_token_usage_handles_progress_payload() {
        let payload = json!({
            "method": "thread/tokenUsage/updated",
            "params": {
                "tokenUsage": {
                    "total": {
                        "inputTokens": 12,
                        "cachedInputTokens": 3,
                        "outputTokens": 5,
                        "totalTokens": 20,
                        "reasoningOutputTokens": 0
                    }
                }
            }
        });

        let usage = usage_info_from_codex_token_usage(&payload).expect("usage should parse");

        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.output_tokens, 5);
        assert_eq!(usage.cache_read, 3);
        assert_eq!(usage.cache_write, 0);
    }

    #[test]
    fn provider_config_sanitizer_trims_blank_fields() {
        let config = normalize_provider_config(ProviderConfig {
            name: "  minimax  ".to_owned(),
            protocol: "anthropic".to_owned(),
            base_url: Some(" https://api.minimaxi.com/anthropic/ ".to_owned()),
            api_key: Some("  token  ".to_owned()),
            model: Some(" minimax-m2.7 ".to_owned()),
            profiles: vec![],
            active_profile: None,
            api_key_stored: false,
        })
        .expect("provider config should sanitize");

        assert_eq!(config.name, "minimax");
        assert_eq!(config.api_key.as_deref(), Some("token"));
        assert_eq!(config.model.as_deref(), Some("minimax-m2.7"));
    }

    #[test]
    fn runtime_status_snapshot_includes_auth_and_tool_filters() {
        let temp = std::env::temp_dir().join(format!("remote-code-gui-status-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp).expect("temp dir should work");
        let config = test_runtime_config(&temp, &temp.join(".remote-code-rust"));

        let snapshot = runtime_status_snapshot_from_config(&config);
        assert_eq!(snapshot.provider.name, "glm-coding");
        assert_eq!(snapshot.provider.effort.as_deref(), Some("medium"));
        assert_eq!(snapshot.permission_mode, "acceptEdits");
        assert_eq!(snapshot.output_style.as_deref(), Some("Explanatory"));
        assert_eq!(snapshot.language.as_deref(), Some("Chinese"));
        assert!(snapshot.brief_enabled);
        assert!(snapshot.proactive_active);
        assert_eq!(
            snapshot.allowed_setting_sources,
            vec!["user", "project", "local"]
        );
        assert_eq!(snapshot.allowed_tools, vec!["read_file"]);
        assert_eq!(snapshot.disallowed_tools, vec!["bash_command"]);
        assert_eq!(snapshot.mcp.total_servers, 0);
        assert_eq!(snapshot.mcp.enabled_servers, 0);
        assert_eq!(snapshot.mcp.disabled_servers, 0);
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[tokio::test]
    async fn build_runtime_mcp_inventory_uses_project_override_and_runtime_origins() {
        let temp = tempdir().expect("tempdir should work");
        let project_dir = temp.path().join("project");
        let profile_dir = temp.path().join(".remote-code-rust");
        let plugin_root = profile_dir.join("plugins").join("sample");
        fs::create_dir_all(&project_dir).expect("project dir should exist");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("plugin dir");
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{"name":"sample","version":"0.1.0","mcp":"./mcp.toml"}"#,
        )
        .expect("plugin manifest write");

        let mut plugin_mcp = McpConfig::default();
        plugin_mcp.servers.insert(
            "plugin-demo".to_owned(),
            sample_stdio_mcp_server("plugin-demo", true, "plugin-cmd"),
        );
        plugin_mcp
            .save(plugin_root.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("plugin MCP config should save");

        let mut profile_mcp = McpConfig::default();
        profile_mcp.servers.insert(
            "profile-demo".to_owned(),
            sample_stdio_mcp_server("profile-demo", true, "profile-cmd"),
        );
        profile_mcp
            .save(profile_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("profile MCP config should save");

        let mut project_mcp = McpConfig::default();
        project_mcp.servers.insert(
            "project-demo".to_owned(),
            sample_stdio_mcp_server("project-demo", true, "project-cmd"),
        );
        project_mcp.servers.insert(
            "disabled-demo".to_owned(),
            sample_stdio_mcp_server("disabled-demo", false, "disabled-cmd"),
        );
        project_mcp
            .save(project_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("project MCP config should save");

        let config = test_runtime_config(temp.path(), &profile_dir);
        let inventory = build_runtime_mcp_inventory(
            &config,
            Some(project_dir.to_str().expect("utf8 project path")),
            false,
            true,
        )
        .await
        .expect("runtime inventory should build");

        assert_eq!(inventory.effective_cwd, project_dir.display().to_string());
        assert_eq!(inventory.warnings.len(), 0);
        assert_eq!(inventory.summary.total_servers, 4);
        assert_eq!(inventory.summary.status_counts.pending, 3);
        assert_eq!(inventory.summary.status_counts.disabled, 1);
        assert_eq!(inventory.servers.len(), 4);
        assert!(
            inventory
                .servers
                .iter()
                .any(|server| server.name == "project-demo"
                    && server.status == "pending"
                    && server.origin_kind == "cwd")
        );
        assert!(
            inventory
                .servers
                .iter()
                .any(|server| server.name == "disabled-demo"
                    && server.status == "disabled"
                    && server.origin_kind == "cwd")
        );
        assert!(
            inventory
                .servers
                .iter()
                .any(|server| server.name == "profile-demo" && server.origin_kind == "profile")
        );
        assert!(
            inventory
                .servers
                .iter()
                .any(|server| server.name == "plugin-demo"
                    && server.origin_kind == "plugin"
                    && server.origin_name == "sample")
        );

        let snapshot = runtime_status_snapshot_from_config(&config);
        assert_eq!(snapshot.mcp.total_servers, 2);
        assert_eq!(snapshot.mcp.enabled_servers, 2);
        assert_eq!(snapshot.mcp.disabled_servers, 0);
        assert_eq!(snapshot.mcp.origins.profile, 1);
        assert_eq!(snapshot.mcp.origins.plugin, 1);
    }

    #[test]
    fn configure_runtime_policy_for_config_populates_runtime_mcp_inventory() {
        let _runtime_policy_guard = runtime_policy_test_mutex()
            .lock()
            .expect("runtime policy test mutex");
        let original_policy = claude_tools::current_tool_runtime_policy();
        let temp = tempdir().expect("tempdir should work");
        let project_dir = temp.path().join("project");
        let profile_dir = temp.path().join(".remote-code-rust");
        let plugin_root = profile_dir.join("plugins").join("sample");
        fs::create_dir_all(&project_dir).expect("project dir should exist");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("plugin dir");
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{"name":"sample","version":"0.1.0","mcp":"./mcp.toml"}"#,
        )
        .expect("plugin manifest write");

        let mut plugin_mcp = McpConfig::default();
        plugin_mcp.servers.insert(
            "plugin-demo".to_owned(),
            sample_stdio_mcp_server("plugin-demo", true, "plugin-cmd"),
        );
        plugin_mcp
            .save(plugin_root.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("plugin MCP config should save");

        let mut profile_mcp = McpConfig::default();
        profile_mcp.servers.insert(
            "profile-demo".to_owned(),
            sample_stdio_mcp_server("profile-demo", true, "profile-cmd"),
        );
        profile_mcp
            .save(profile_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("profile MCP config should save");

        let mut project_mcp = McpConfig::default();
        project_mcp.servers.insert(
            "project-demo".to_owned(),
            sample_stdio_mcp_server("project-demo", true, "project-cmd"),
        );
        project_mcp
            .save(project_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("project MCP config should save");

        let config = test_runtime_config(&project_dir, &profile_dir);
        configure_runtime_policy_for_config(&config).expect("runtime policy should configure");

        let policy = claude_tools::current_tool_runtime_policy();
        let names = policy
            .mcp_servers
            .iter()
            .map(|entry| entry.server.name.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            names,
            std::collections::BTreeSet::from(["plugin-demo", "profile-demo", "project-demo"])
        );
        assert!(
            policy
                .mcp_servers
                .iter()
                .any(|entry| entry.server.name == "plugin-demo"
                    && entry.origin_kind == "plugin"
                    && entry.origin_name == "sample")
        );

        claude_tools::configure_tool_runtime_policy(original_policy)
            .expect("runtime policy should restore");
    }

    #[tokio::test]
    async fn build_gui_doctor_report_counts_managed_mcp_servers() {
        let temp = tempdir().expect("tempdir should work");
        let project_dir = temp.path().join("project");
        let profile_dir = temp.path().join(".remote-code-rust");
        fs::create_dir_all(&project_dir).expect("project dir should exist");

        let config = test_runtime_config(&project_dir, &profile_dir);

        let mut profile_mcp = McpConfig::default();
        profile_mcp.servers.insert(
            "profile-demo".to_owned(),
            sample_stdio_mcp_server("profile-demo", true, "profile-cmd"),
        );
        profile_mcp
            .save(profile_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("profile MCP config should save");

        let mut project_mcp = McpConfig::default();
        project_mcp.servers.insert(
            "project-demo".to_owned(),
            sample_stdio_mcp_server("project-demo", true, "project-cmd"),
        );
        project_mcp
            .save(project_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("project MCP config should save");

        let report = build_gui_doctor_report(&config, false, false, false, false)
            .await
            .expect("doctor report should build");

        assert!(report.ok);
        assert_eq!(report.extensions.managed_mcp_servers, 2);
        assert_eq!(report.extensions.plugin_mcp_servers, 0);
        assert_eq!(report.mcp_runtime.summary.total_servers, 2);
        assert_eq!(report.mcp_runtime.summary.status_counts.pending, 2);
        assert_eq!(report.mcp_runtime.summary.status_counts.failed, 0);
        assert!(report.network.is_empty());
        assert!(report.env_providers.is_empty());
    }

    #[tokio::test]
    async fn build_gui_doctor_report_respects_setting_sources() {
        let temp = tempdir().expect("tempdir should work");
        let project_dir = temp.path().join("project");
        let profile_dir = temp.path().join(".remote-code-rust");
        let plugin_root = profile_dir.join("plugins").join("sample");
        fs::create_dir_all(&project_dir).expect("project dir should exist");
        fs::create_dir_all(profile_dir.join("skills").join("demo")).expect("profile skills");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("plugin dir");
        fs::write(
            profile_dir.join("skills").join("demo").join("SKILL.md"),
            "# Demo\n\nSummary.\n",
        )
        .expect("profile skill write");
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{"name":"sample","version":"0.1.0","mcp":"./mcp.toml"}"#,
        )
        .expect("plugin manifest write");
        let mut plugin_mcp = McpConfig::default();
        plugin_mcp.servers.insert(
            "plugin-demo".to_owned(),
            sample_stdio_mcp_server("plugin-demo", true, "plugin-cmd"),
        );
        plugin_mcp
            .save(plugin_root.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("plugin MCP config should save");

        let mut profile_mcp = McpConfig::default();
        profile_mcp.servers.insert(
            "profile-demo".to_owned(),
            sample_stdio_mcp_server("profile-demo", true, "profile-cmd"),
        );
        profile_mcp
            .save(profile_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("profile MCP config should save");

        let mut project_mcp = McpConfig::default();
        project_mcp.servers.insert(
            "project-demo".to_owned(),
            sample_stdio_mcp_server("project-demo", true, "project-cmd"),
        );
        project_mcp
            .save(project_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("project MCP config should save");

        let mut project_only = test_runtime_config(&project_dir, &profile_dir);
        project_only.allowed_setting_sources = vec![SettingSource::Project];
        let report = build_gui_doctor_report(&project_only, false, false, false, false)
            .await
            .expect("project-only doctor report");
        assert_eq!(report.runtime.allowed_setting_sources, vec!["project"]);
        assert_eq!(report.extensions.skills, 0);
        assert_eq!(report.extensions.plugins, 0);
        assert_eq!(report.extensions.disabled_plugins, 0);
        assert_eq!(report.extensions.managed_mcp_servers, 1);
        assert_eq!(report.extensions.plugin_mcp_servers, 0);
        assert_eq!(report.mcp_runtime.summary.total_servers, 1);

        let mut user_only = test_runtime_config(&project_dir, &profile_dir);
        user_only.allowed_setting_sources = vec![SettingSource::User];
        let report = build_gui_doctor_report(&user_only, false, false, false, false)
            .await
            .expect("user-only doctor report");
        assert_eq!(report.runtime.allowed_setting_sources, vec!["user"]);
        assert_eq!(report.extensions.skills, 1);
        assert_eq!(report.extensions.plugins, 1);
        assert_eq!(report.extensions.managed_mcp_servers, 1);
        assert_eq!(report.extensions.plugin_mcp_servers, 1);
        assert_eq!(report.mcp_runtime.summary.total_servers, 2);
    }

    #[tokio::test]
    async fn build_mcp_server_list_respects_setting_sources() {
        let temp = tempdir().expect("tempdir should work");
        let project_dir = temp.path().join("project");
        let profile_dir = temp.path().join(".remote-code-rust");
        fs::create_dir_all(&project_dir).expect("project dir should exist");

        let mut profile_mcp = McpConfig::default();
        profile_mcp.servers.insert(
            "profile-demo".to_owned(),
            sample_stdio_mcp_server("profile-demo", true, "profile-cmd"),
        );
        profile_mcp
            .save(profile_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("profile MCP config should save");

        let mut project_mcp = McpConfig::default();
        project_mcp.servers.insert(
            "project-demo".to_owned(),
            sample_stdio_mcp_server("project-demo", true, "project-cmd"),
        );
        project_mcp
            .save(project_dir.join(DEFAULT_MCP_CONFIG_FILE))
            .expect("project MCP config should save");

        let mut user_only = test_runtime_config(&project_dir, &profile_dir);
        user_only.allowed_setting_sources = vec![SettingSource::User];
        let list = build_mcp_server_list(
            &user_only,
            ConfigScopeDto::Project,
            Some(project_dir.to_str().expect("utf8 project path")),
            false,
            false,
        )
        .await
        .expect("project list should build");
        assert!(list.servers.is_empty());
        assert_eq!(list.warnings.len(), 1);

        let mut project_only = test_runtime_config(&project_dir, &profile_dir);
        project_only.allowed_setting_sources = vec![SettingSource::Project];
        let list =
            build_mcp_server_list(&project_only, ConfigScopeDto::Profile, None, false, false)
                .await
                .expect("profile list should build");
        assert!(list.servers.is_empty());
        assert_eq!(list.warnings.len(), 1);
    }

    #[tokio::test]
    async fn managed_mcp_helpers_round_trip_and_reset() {
        let temp = tempdir().expect("tempdir should work");
        let project_dir = temp.path().join("project");
        let profile_dir = temp.path().join(".remote-code-rust");
        fs::create_dir_all(&project_dir).expect("project dir should exist");

        let config = test_runtime_config(&project_dir, &profile_dir);
        let config_path =
            mcp_config_path_for_scope(&config, ConfigScopeDto::Profile, None).expect("path");

        let request = McpServerUpsertRequestDto {
            scope: ConfigScopeDto::Profile,
            project_path: None,
            name: "demo".to_owned(),
            transport: "stdio".to_owned(),
            command: Some("demo-mcp".to_owned()),
            url: None,
            args: vec!["serve".to_owned()],
            cwd: Some(project_dir.display().to_string()),
            env: BTreeMap::from([("TOKEN".to_owned(), "secret".to_owned())]),
            headers: BTreeMap::new(),
            metadata: BTreeMap::from([("team".to_owned(), "gui".to_owned())]),
            disabled: false,
            startup_timeout_secs: Some(10),
            request_timeout_secs: Some(20),
        };

        let saved =
            save_managed_mcp_server_at_path(&config_path, ConfigScopeDto::Profile, &request)
                .expect("save should succeed");
        assert_eq!(saved.status, "created");
        assert_eq!(saved.enabled, Some(true));

        let listed = build_mcp_server_list(&config, ConfigScopeDto::Profile, None, false, true)
            .await
            .expect("list should succeed");
        assert_eq!(listed.servers.len(), 1);
        assert_eq!(listed.servers[0].name, "demo");
        assert_eq!(listed.servers[0].command.as_deref(), Some("demo-mcp"));
        assert_eq!(listed.servers[0].env_keys, vec!["TOKEN"]);
        assert_eq!(listed.servers[0].metadata_keys, vec!["team"]);

        let toggled = toggle_managed_mcp_server_at_path(
            &config_path,
            ConfigScopeDto::Profile,
            "demo",
            false,
            false,
        )
        .expect("toggle should succeed");
        assert_eq!(toggled.status, "disabled");
        assert_eq!(toggled.enabled, Some(false));

        let enabled_only =
            build_mcp_server_list(&config, ConfigScopeDto::Profile, None, false, false)
                .await
                .expect("filtered list should succeed");
        assert!(enabled_only.servers.is_empty());

        let removed =
            remove_managed_mcp_server_at_path(&config_path, ConfigScopeDto::Profile, "demo", false)
                .expect("remove should succeed");
        assert_eq!(removed.status, "removed");

        save_managed_mcp_server_at_path(&config_path, ConfigScopeDto::Profile, &request)
            .expect("save after remove should succeed");
        let reset = reset_managed_mcp_config_at_path(&config_path, ConfigScopeDto::Profile, false)
            .expect("reset should succeed");
        assert_eq!(reset.status, "reset");
        assert!(!config_path.exists());
    }

    #[test]
    fn export_session_bundle_helper_writes_requested_formats() {
        let temp = tempdir().expect("tempdir should work");
        let paths = AppPaths::discover(Some(temp.path().join(".remote-code-rust"))).expect("paths");
        let store = SessionStore::open(paths).expect("store should open");
        let session_id = Uuid::new_v4();
        store
            .ensure_session(
                session_id,
                temp.path(),
                "glm-coding",
                Some("glm-5.1"),
                Some("Export parity"),
            )
            .expect("session should be created");
        store
            .append_named_event(
                session_id,
                "result",
                json!({
                    "is_error": false,
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 5, "output_tokens": 8}
                }),
            )
            .expect("event should append");

        let json_export =
            export_session_bundle_for_store(&store, session_id, SessionExportFormatDto::Json)
                .expect("json export should succeed");
        assert!(Path::new(&json_export.path).exists());
        assert!(json_export.path.ends_with(".json"));

        let ndjson_export =
            export_session_bundle_for_store(&store, session_id, SessionExportFormatDto::Ndjson)
                .expect("ndjson export should succeed");
        assert!(Path::new(&ndjson_export.path).exists());
        assert!(ndjson_export.path.ends_with(".ndjson"));
    }

    #[test]
    fn normalize_project_entries_deduplicates_equivalent_paths() {
        let projects = normalize_project_entries(vec![
            ProjectEntry {
                path: PathBuf::from(r"C:\Work\Alpha"),
                name: "Alpha".to_owned(),
            },
            ProjectEntry {
                path: PathBuf::from(r"C:\Work\Alpha\"),
                name: "Alpha Duplicate".to_owned(),
            },
        ]);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Alpha");
    }

    #[test]
    fn ensure_sessions_have_projects_promotes_orphan_session_folders() {
        let mut managed = vec![ProjectEntry {
            path: PathBuf::from(r"C:\Work\Alpha"),
            name: "Alpha".to_owned(),
        }];
        let sessions = vec![
            SessionSummary {
                session_id: Uuid::new_v4(),
                parent_session_id: None,
                title: "alpha-session".to_owned(),
                cwd: PathBuf::from(r"C:\Work\Alpha"),
                provider_name: "glm".to_owned(),
                model: Some("glm-5.1".to_owned()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                transcript_path: PathBuf::from("alpha.ndjson"),
                archived: false,
            },
            SessionSummary {
                session_id: Uuid::new_v4(),
                parent_session_id: None,
                title: "orphan-session".to_owned(),
                cwd: PathBuf::from(r"C:\Work\Beta"),
                provider_name: "minimax".to_owned(),
                model: Some("minimax-m2.7".to_owned()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                transcript_path: PathBuf::from("beta.ndjson"),
                archived: false,
            },
        ];

        assert!(ensure_sessions_have_projects(&mut managed, &sessions));
        assert_eq!(managed.len(), 2);
        assert!(managed.iter().any(|project| project.name == "Alpha"));
        assert!(managed.iter().any(|project| project.name == "Beta"));
    }

    #[test]
    fn build_project_infos_groups_sessions_under_project_nodes() {
        let managed = vec![ProjectEntry {
            path: PathBuf::from(r"C:\Work\Alpha"),
            name: "Alpha".to_owned(),
        }];
        let sessions = vec![
            SessionSummary {
                session_id: Uuid::new_v4(),
                parent_session_id: None,
                title: "alpha-session".to_owned(),
                cwd: PathBuf::from(r"C:\Work\Alpha"),
                provider_name: "glm".to_owned(),
                model: Some("glm-5.1".to_owned()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                transcript_path: PathBuf::from("alpha.ndjson"),
                archived: false,
            },
            SessionSummary {
                session_id: Uuid::new_v4(),
                parent_session_id: None,
                title: "orphan-session".to_owned(),
                cwd: PathBuf::from(r"C:\Work\Beta"),
                provider_name: "minimax".to_owned(),
                model: Some("minimax-m2.7".to_owned()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                transcript_path: PathBuf::from("beta.ndjson"),
                archived: false,
            },
        ];

        let projects = build_project_infos(&managed, &sessions);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Alpha");
        assert_eq!(projects[0].session_count, 1);
    }
}
