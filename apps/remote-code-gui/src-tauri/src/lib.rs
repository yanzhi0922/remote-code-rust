use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use rc_config::{
    load_runtime_config, normalize_base_url, validate_provider_config, AppPaths,
    ProviderConfig as RuntimeProviderConfig, ProviderOverrides, RuntimeConfig, RuntimeOverrides,
};
use rc_core::{
    default_system_prompt, ConversationEntry, ConversationRole, PermissionMode, ProviderProtocol,
    ProviderResponse, SubAgentCompletion, ToolCall, UsageSummary,
};
use rc_permissions::{
    auto_allows, classify_tool, load_layered_rules, LayeredPermissionBroker, PermissionBroker,
    PermissionDecision, PermissionRequest,
};
use rc_provider::context::ContextWindowManager;
use rc_provider::streaming::StreamingCallbacks;
use rc_provider::ProviderClient;
use rc_session::{SessionStore, SessionSummary};
use rc_tools::shell::ShellExecutionPolicy;
use rc_tools::{
    agent::{parse_delegate_progress_event, DelegateProgressEvent},
    configure_tool_runtime_policy, execute_tool_call, runtime_builtin_tool_specs,
    tasks::load_persisted_ui_task_snapshots,
    ToolExecutionContext, ToolRuntimePolicy,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};
use uuid::Uuid;

const APP_EVENT_PERMISSION_REQUEST: &str = "gui://permission-request";
const APP_EVENT_PERMISSION_RESOLVED: &str = "gui://permission-resolved";
const APP_EVENT_TOOL_START: &str = "gui://tool-start";
const APP_EVENT_TOOL_RESULT: &str = "gui://tool-result";
const APP_EVENT_TOOL_PROGRESS: &str = "gui://tool-progress";
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
    #[serde(default, skip)]
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
}

#[derive(Debug, Clone, Serialize)]
struct PermissionDecisionDto {
    request_id: String,
    allowed: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedProviderContext {
    name: String,
    base_url: Option<String>,
    model: Option<String>,
    protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSessionContext {
    cwd: PathBuf,
    permission_mode: String,
    provider: PersistedProviderContext,
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
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    running_prompts: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
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
    let raw = path.to_string_lossy().replace('/', "\\");
    let normalized = raw.trim_end_matches('\\').to_owned();
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
    configure_tool_runtime_policy(ToolRuntimePolicy {
        allowed_tools: config.allowed_tools.clone(),
        disallowed_tools: config.disallowed_tools.clone(),
        task_output_dir: Some(task_dir_for_paths(&config.paths, config.session_id)),
        shell_policy: ShellExecutionPolicy {
            block_inline_cwd: true,
            allow_background: true,
            block_destructive_git: true,
            max_capture_chars: 16_000,
            output_dir: Some(shell_output_dir_for_paths(&config.paths, config.session_id)),
        },
    })
}

fn ui_task_node_to_dto(session_id: &str, task: rc_ui_bridge::UiTaskNode) -> SessionTaskDto {
    SessionTaskDto {
        session_id: session_id.to_owned(),
        task_id: task.id,
        parent_task_id: task.parent_task_id,
        description: task.title,
        depth: task.depth,
        status: match task.status {
            rc_ui_bridge::UiTaskStatus::Pending => "pending",
            rc_ui_bridge::UiTaskStatus::Running => "running",
            rc_ui_bridge::UiTaskStatus::Completed => "completed",
            rc_ui_bridge::UiTaskStatus::Failed => "failed",
            rc_ui_bridge::UiTaskStatus::Stopped => "stopped",
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
            rc_ui_bridge::UiTaskKind::Background => "background",
            rc_ui_bridge::UiTaskKind::Delegation => "delegation",
            rc_ui_bridge::UiTaskKind::Batch => "batch",
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
        rc_core::InputFormat::Text,
        rc_core::OutputFormat::Text,
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
    store.append_named_event(
        config.session_id,
        "session_context",
        serde_json::to_value(PersistedSessionContext {
            cwd: config.cwd.clone(),
            permission_mode: config.permission_mode.as_legacy_str().to_owned(),
            provider: PersistedProviderContext {
                name: config.provider.name.clone(),
                base_url: config.provider.base_url.clone(),
                model: config.provider.model.clone(),
                protocol: config.provider.protocol.as_str().to_owned(),
            },
        })?,
    )
}

fn restore_session_context(store: &SessionStore, config: &mut RuntimeConfig) -> Result<()> {
    if let Ok(summary) = store.get_session_summary(config.session_id) {
        config.cwd = summary.cwd;
        config.provider.name = summary.provider_name;
        config.provider.model = summary.model;
    }
    let events = store.load_events(config.session_id).unwrap_or_default();
    let payload = events.into_iter().rev().find_map(|event| {
        (event.event_type == "session_context")
            .then_some(event.payload)
            .flatten()
    });
    let Some(payload) = payload else {
        return Ok(());
    };
    let persisted: PersistedSessionContext = serde_json::from_value(payload)?;
    config.cwd = persisted.cwd;
    config.provider.name = persisted.provider.name;
    config.provider.base_url = persisted.provider.base_url;
    config.provider.model = persisted.provider.model;
    if let Some(protocol) = parse_protocol(Some(&persisted.provider.protocol)) {
        config.provider.protocol = protocol;
        config.provider.base_url =
            normalize_base_url(config.provider.base_url.clone(), config.provider.protocol);
    }
    if let Some(permission_mode) = parse_permission_mode(Some(&persisted.permission_mode)) {
        config.permission_mode = permission_mode;
    }
    Ok(())
}

fn initialize_session_conversation(
    store: &SessionStore,
    config: &RuntimeConfig,
    title_hint: Option<&str>,
) -> Result<Vec<ConversationEntry>> {
    store.ensure_session(
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        title_hint,
    )?;
    persist_session_context(store, config)?;
    let mut conversation = store
        .load_conversation(config.session_id)
        .unwrap_or_default();
    if conversation.is_empty() {
        let system = ConversationEntry::system(default_system_prompt(&config.cwd));
        store.append_conversation_entry(config.session_id, &system)?;
        conversation.push(system);
    }
    Ok(conversation)
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

struct GuiSubAgent {
    client: Arc<ProviderClient>,
    provider: RuntimeProviderConfig,
}

impl GuiSubAgent {
    fn new(client: Arc<ProviderClient>, provider: &RuntimeProviderConfig) -> Self {
        Self {
            client,
            provider: provider.clone(),
        }
    }
}

#[async_trait]
impl SubAgentCompletion for GuiSubAgent {
    async fn complete(
        &self,
        conversation: &[ConversationEntry],
    ) -> anyhow::Result<ProviderResponse> {
        self.client.complete(&self.provider, conversation).await
    }
}

struct GuiPermissionBroker {
    mode: PermissionMode,
    app: AppHandle,
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
}

#[async_trait]
impl PermissionBroker for GuiPermissionBroker {
    fn mode(&self) -> PermissionMode {
        self.mode
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        if auto_allows(self.mode, classify_tool(&request.tool_name)) {
            return PermissionDecision::allow();
        }

        match self.mode {
            PermissionMode::DontAsk => {
                return PermissionDecision::deny(format!(
                    "Permission mode {} denied {}.",
                    self.mode.as_legacy_str(),
                    request.tool_name
                ));
            }
            PermissionMode::Plan => {
                return PermissionDecision::deny("Plan mode does not execute tools.");
            }
            PermissionMode::Default
            | PermissionMode::AcceptEdits
            | PermissionMode::BypassPermissions => {}
        }

        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_permissions.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        let payload = PermissionRequestDto {
            request_id: request_id.clone(),
            tool_name: request.tool_name.clone(),
            tool_use_id: request.tool_use_id.clone(),
            title: request.title.clone(),
            description: request.description.clone(),
            input: request.input.clone(),
            blocked_path: request.blocked_path.clone(),
        };

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
        let allowed = match decision {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => false,
            Err(_) => false,
        };

        let _ = self.app.emit(
            APP_EVENT_PERMISSION_RESOLVED,
            PermissionDecisionDto {
                request_id,
                allowed,
            },
        );

        if allowed {
            PermissionDecision::allow()
        } else {
            PermissionDecision::deny(format!("Permission denied for {}.", request.tool_name))
        }
    }
}

#[derive(Debug)]
struct PromptRunOutcome {
    text: String,
    tool_calls: Vec<ToolCall>,
    usage: UsageSummary,
    num_turns: u32,
    stop_reason: String,
}

async fn run_gui_prompt(
    app: AppHandle,
    config: RuntimeConfig,
    provider: Arc<ProviderClient>,
    store: Arc<SessionStore>,
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    prompt: &str,
) -> Result<PromptRunOutcome> {
    let mut conversation = initialize_session_conversation(&store, &config, Some(prompt))?;
    store.append_named_event(
        config.session_id,
        "prompt_started",
        serde_json::json!({
            "prompt": prompt,
            "provider": config.provider.name,
            "model": config.provider.model,
            "protocol": config.provider.protocol.as_str(),
        }),
    )?;

    let user_entry = ConversationEntry::user(prompt);
    store.append_conversation_entry(config.session_id, &user_entry)?;
    conversation.push(user_entry);

    let context_manager =
        ContextWindowManager::for_model(config.provider.model.as_deref().unwrap_or("unknown"));
    let task_paths = config.paths.clone();
    let session_id = config.session_id;
    let session_id_string = session_id.to_string();

    let tool_context = ToolExecutionContext {
        cwd: config.cwd.clone(),
        timeout_ms: config.provider.timeout_ms,
        sub_agent: Some(Arc::new(GuiSubAgent::new(
            Arc::clone(&provider),
            &config.provider,
        ))),
        progress_cb: Some(Arc::new({
            let app = app.clone();
            let sid = session_id_string.clone();
            let paths = task_paths.clone();
            move |message: &str| {
                let Some(event) = parse_delegate_progress_event(message) else {
                    let _ = app.emit(
                        APP_EVENT_TOOL_PROGRESS,
                        ToolProgressDto {
                            tool_call_id: String::new(),
                            tool_name: "agent".to_owned(),
                            message: message.to_owned(),
                        },
                    );
                    return;
                };

                match event {
                    DelegateProgressEvent::SubtaskStarted {
                        task_id,
                        parent_task_id,
                        description,
                        depth,
                    } => {
                        let _ = app.emit(
                            APP_EVENT_SUBTASK_STARTED,
                            SubtaskStartedDto {
                                session_id: sid.clone(),
                                task_id,
                                parent_task_id,
                                description,
                                depth,
                            },
                        );
                        emit_task_snapshot_for_session(&app, &paths, session_id);
                    }
                    DelegateProgressEvent::SubtaskProgress {
                        task_id,
                        turn,
                        max_turns,
                        summary,
                    } => {
                        let _ = app.emit(
                            APP_EVENT_SUBTASK_PROGRESS,
                            SubtaskProgressDto {
                                session_id: sid.clone(),
                                task_id: task_id.clone(),
                                turn,
                                max_turns,
                                summary: summary.clone(),
                            },
                        );
                        let _ = app.emit(
                            APP_EVENT_TOOL_PROGRESS,
                            ToolProgressDto {
                                tool_call_id: task_id,
                                tool_name: "agent".to_owned(),
                                message: summary,
                            },
                        );
                        emit_task_snapshot_for_session(&app, &paths, session_id);
                    }
                    DelegateProgressEvent::SubtaskCompleted {
                        task_id,
                        success,
                        output_preview,
                        turns_used,
                    } => {
                        let _ = app.emit(
                            APP_EVENT_SUBTASK_COMPLETED,
                            SubtaskCompletedDto {
                                session_id: sid.clone(),
                                task_id,
                                success,
                                output_preview,
                                turns_used,
                            },
                        );
                        emit_task_snapshot_for_session(&app, &paths, session_id);
                    }
                    DelegateProgressEvent::BatchProgress {
                        total,
                        completed,
                        running,
                    } => {
                        let _ = app.emit(
                            APP_EVENT_BATCH_PROGRESS,
                            BatchProgressDto {
                                session_id: sid.clone(),
                                total,
                                completed,
                                running,
                            },
                        );
                        emit_task_snapshot_for_session(&app, &paths, session_id);
                    }
                }
            }
        })),
        task_stack: Arc::new(std::sync::Mutex::new(
            rc_core::task_stack::TaskStack::default(),
        )),
    };

    let broker = LayeredPermissionBroker::new(
        GuiPermissionBroker {
            mode: config.permission_mode,
            app: app.clone(),
            pending_permissions,
        },
        load_layered_rules(
            &config.cwd,
            &config.paths.profile_dir,
            &config.settings_files,
        )?,
    );

    let mut usage = UsageSummary::default();
    let started = Instant::now();

    for turn in 0..config.max_turns.max(DEFAULT_MAX_TURNS) {
        let budget_snapshot = context_manager.budget_snapshot(&conversation);
        let _ = app.emit(
            APP_EVENT_CONTEXT_USAGE,
            ContextUsageDto {
                session_id: session_id_string.clone(),
                estimated_tokens: budget_snapshot.estimated_tokens,
                max_input_tokens: budget_snapshot.max_input_tokens,
                threshold_tokens: budget_snapshot.threshold_tokens(),
                ratio: budget_snapshot.usage_ratio,
            },
        );

        if budget_snapshot.exceeds_threshold() {
            let _ = app.emit(
                APP_EVENT_CONTEXT_OVERFLOW,
                ContextOverflowDto {
                    session_id: session_id_string.clone(),
                    estimated_tokens: budget_snapshot.estimated_tokens,
                    max_input_tokens: budget_snapshot.max_input_tokens,
                    threshold_tokens: budget_snapshot.threshold_tokens(),
                    ratio: budget_snapshot.usage_ratio,
                },
            );
            store.append_named_event(
                config.session_id,
                "context_overflow",
                serde_json::json!({
                    "turn": turn + 1,
                    "estimated_tokens": budget_snapshot.estimated_tokens,
                    "max_input_tokens": budget_snapshot.max_input_tokens,
                    "threshold_tokens": budget_snapshot.threshold_tokens(),
                    "usage_ratio": budget_snapshot.usage_ratio,
                }),
            )?;

            let compacted = context_manager.compact(&conversation);
            let removed = conversation.len().saturating_sub(compacted.len());
            conversation = compacted;
            let compacted_snapshot = context_manager.budget_snapshot(&conversation);

            if removed > 0 {
                store.append_named_event(
                    config.session_id,
                    "context_compacted",
                    serde_json::json!({
                        "turn": turn + 1,
                        "entries_removed": removed,
                        "usage_ratio_before": budget_snapshot.usage_ratio,
                        "usage_ratio_after": compacted_snapshot.usage_ratio,
                        "estimated_tokens_before": budget_snapshot.estimated_tokens,
                        "estimated_tokens_after": compacted_snapshot.estimated_tokens,
                    }),
                )?;
                let _ = app.emit(
                    APP_EVENT_CONTEXT_COMPACTED,
                    ContextCompactedDto {
                        session_id: session_id_string.clone(),
                        entries_removed: removed,
                        usage_ratio: compacted_snapshot.usage_ratio,
                    },
                );
                let _ = app.emit(
                    APP_EVENT_CONTEXT_USAGE,
                    ContextUsageDto {
                        session_id: session_id_string.clone(),
                        estimated_tokens: compacted_snapshot.estimated_tokens,
                        max_input_tokens: compacted_snapshot.max_input_tokens,
                        threshold_tokens: compacted_snapshot.threshold_tokens(),
                        ratio: compacted_snapshot.usage_ratio,
                    },
                );
            }
        }

        let streaming_callbacks = StreamingCallbacks {
            on_text_delta: Some(Box::new({
                let app = app.clone();
                let sid = config.session_id.to_string();
                move |delta: &str| {
                    let _ = app.emit(
                        APP_EVENT_STREAMING_DELTA,
                        StreamingDeltaDto {
                            session_id: sid.clone(),
                            delta: delta.to_owned(),
                        },
                    );
                }
            })),
            ..Default::default()
        };

        let response = provider
            .complete_streaming_with_callbacks(
                &config.provider,
                &conversation,
                Some(streaming_callbacks),
            )
            .await?;
        usage.input_tokens += response.usage.input_tokens;
        usage.output_tokens += response.usage.output_tokens;
        let mut content_blocks = response.content_blocks.clone();
        if let Some(thinking) = response.thinking.clone() {
            content_blocks.push(serde_json::json!({
                "type": "thinking",
                "thinking": thinking,
            }));
        }
        let assistant_entry = ConversationEntry {
            role: ConversationRole::Assistant,
            text: response.text.clone(),
            history_text: response.history_text.clone(),
            content_blocks,
            tool_calls: response.tool_calls.clone(),
            attachments: Vec::new(),
            tool_call_id: None,
            name: None,
            is_error: false,
        };
        store.append_conversation_entry(config.session_id, &assistant_entry)?;
        conversation.push(assistant_entry);

        store.append_named_event(
            config.session_id,
            "assistant_turn",
            serde_json::json!({
                "turn": turn + 1,
                "stop_reason": response.stop_reason,
                "usage": {
                    "input_tokens": response.usage.input_tokens,
                    "output_tokens": response.usage.output_tokens,
                },
                "tool_calls": response.tool_calls.len(),
            }),
        )?;

        if response.tool_calls.is_empty() {
            let _elapsed_ms = started.elapsed().as_millis() as u64;
            store.append_named_event(
                config.session_id,
                "result",
                serde_json::json!({
                "is_error": false,
                    "stop_reason": response.stop_reason,
                    "usage": usage_to_dto(&usage),
                    "num_turns": turn + 1,
                }),
            )?;
            return Ok(PromptRunOutcome {
                text: response.text,
                tool_calls: response.tool_calls,
                usage,
                num_turns: (turn + 1) as u32,
                stop_reason: response.stop_reason,
            });
        }

        for tool_call in &response.tool_calls {
            let _ = runtime_builtin_tool_specs()
                .into_iter()
                .find(|spec| spec.name == tool_call.name)
                .ok_or_else(|| anyhow!("unknown tool {}", tool_call.name))?;

            let _ = app.emit(
                APP_EVENT_TOOL_START,
                ToolProgressDto {
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    message: "running".to_owned(),
                },
            );

            let tool_result = match execute_tool_call(tool_call, &tool_context, &broker).await {
                Ok(result) => result,
                Err(error) => rc_core::ToolResult {
                    content: format!("Tool execution error: {error}"),
                    is_error: true,
                },
            };

            let output_for_context =
                context_manager.truncate_tool_output_default(&tool_result.content);
            let tool_entry = ConversationEntry::tool(
                tool_call.id.clone(),
                tool_call.name.clone(),
                output_for_context,
                tool_result.is_error,
            );
            store.append_conversation_entry(config.session_id, &tool_entry)?;
            conversation.push(tool_entry.clone());

            store.append_named_event(
                config.session_id,
                "tool_result",
                serde_json::json!({
                    "tool_name": tool_call.name,
                    "tool_use_id": tool_call.id,
                    "is_error": tool_entry.is_error,
                }),
            )?;

            let _ = app.emit(
                APP_EVENT_TOOL_RESULT,
                ToolResultDto {
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    is_error: tool_result.is_error,
                    output: tool_result.content.clone(),
                },
            );
        }
    }

    Err(anyhow!(
        "Internal GUI safety limit reached ({}) without a final assistant reply.",
        config.max_turns.max(DEFAULT_MAX_TURNS)
    ))
}

fn as_error<T>(result: Result<T>) -> std::result::Result<T, String> {
    result.map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn init_app(state: State<'_, AppState>) -> std::result::Result<InitResultDto, String> {
    let runtime = state.runtime.lock().await;
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
) -> std::result::Result<String, String> {
    let runtime = state.runtime.lock().await;
    let mut config = runtime.config.clone();
    config.session_id = Uuid::new_v4();
    let project_path = project_path
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "请选择项目文件夹后再新建会话。".to_owned())?;
    let normalized_project_path =
        normalize_existing_path(Path::new(&project_path)).map_err(|error| format!("{error:#}"))?;
    let path_key = path_identity(&normalized_project_path);
    if !runtime
        .projects
        .iter()
        .any(|project| path_identity(&project.path) == path_key)
    {
        return Err("会话必须创建在已管理的项目文件夹下。".to_owned());
    }
    config.cwd = normalized_project_path;
    as_error(initialize_session_conversation(
        &runtime.session_store,
        &config,
        title.as_deref(),
    ))?;
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

    let (mut config, provider, session_store, pending_permissions, provider_configs) = {
        let runtime = state.runtime.lock().await;
        let mut config = runtime.config.clone();
        let selected_provider = config.provider.clone();
        let selected_permission_mode = config.permission_mode;
        let session_id =
            session_id.ok_or_else(|| "请先选择项目文件夹并创建会话，再发送消息。".to_owned())?;
        config.session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
        restore_session_context(&runtime.session_store, &mut config)
            .map_err(|error| format!("{error:#}"))?;
        config.provider = selected_provider;
        config.permission_mode = selected_permission_mode;
        (
            config,
            Arc::clone(&runtime.provider),
            Arc::clone(&runtime.session_store),
            Arc::clone(&state.pending_permissions),
            runtime.provider_configs.clone(),
        )
    };

    apply_provider_credentials_from_configs(&mut config.provider, &provider_configs);
    configure_runtime_policy_for_config(&config).map_err(|error| format!("{error:#}"))?;

    let sid = config.session_id.to_string();

    // Reject if this session already has a running prompt.
    {
        let running = state.running_prompts.lock().await;
        if running.contains_key(&sid) {
            return Err("该会话已有正在运行的提示，请等待完成或取消后再试。".to_owned());
        }
    }

    let running_prompts = Arc::clone(&state.running_prompts);
    let sid_for_cleanup = sid.clone();

    let handle = tokio::spawn(async move {
        let result = run_gui_prompt(
            app.clone(),
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
                            tool_calls: outcome.tool_calls.iter().map(tool_call_to_dto).collect(),
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

    {
        let mut running = state.running_prompts.lock().await;
        running.insert(sid.clone(), handle);
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
async fn get_settings(state: State<'_, AppState>) -> std::result::Result<FullSettingsDto, String> {
    let runtime = state.runtime.lock().await;
    Ok(full_settings_from_runtime(
        &runtime.config,
        &runtime.gui_settings,
    ))
}

#[tauri::command]
async fn update_provider(
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

    if let Some(thinking_budget) = runtime.config.provider.thinking_budget {
        if thinking_budget >= runtime.config.provider.max_output_tokens {
            return Err("thinking budget must be lower than max output tokens".to_owned());
        }
    }

    let selected_provider = runtime.config.provider.clone();
    store_provider_selection(&mut runtime, &selected_provider);
    persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
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
    Ok(())
}

#[tauri::command]
async fn delete_provider_config(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    // Remove API key from OS keychain.
    keyring_delete(&name);

    let mut runtime = state.runtime.lock().await;
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
    Ok(())
}

#[tauri::command]
async fn set_active_provider(
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
    Ok(())
}

#[tauri::command]
async fn switch_profile(
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
    Ok(())
}

#[tauri::command]
async fn resolve_permission_request(
    state: State<'_, AppState>,
    request_id: String,
    allowed: bool,
) -> std::result::Result<bool, String> {
    let sender = {
        let mut pending = state.pending_permissions.lock().await;
        pending.remove(&request_id)
    };
    if let Some(sender) = sender {
        let _ = sender.send(allowed);
        Ok(true)
    } else {
        Ok(false)
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

pub fn run() {
    let runtime_state = build_runtime_state().unwrap_or_else(|error| {
        panic!("failed to initialize remote-code-gui runtime: {error:#}");
    });
    let pending_permissions = Arc::new(Mutex::new(HashMap::new()));
    let running_prompts = Arc::new(Mutex::new(HashMap::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            runtime: Mutex::new(runtime_state),
            pending_permissions,
            running_prompts,
        })
        .invoke_handler(tauri::generate_handler![
            init_app,
            list_sessions,
            get_session_conversation,
            get_session_tasks,
            send_prompt,
            cancel_prompt,
            get_provider_info,
            create_session,
            get_settings,
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
            pick_folder
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| panic!("error while running tauri application: {error}"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

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
