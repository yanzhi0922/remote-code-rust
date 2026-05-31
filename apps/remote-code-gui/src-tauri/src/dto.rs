use std::collections::BTreeMap;
use std::path::PathBuf;

use claude_ui_bridge::{UiRuntimeMcpInventorySummary, UiRuntimeMcpServerStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ProjectEntry {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ProjectListFile {
    #[serde(default)]
    pub(crate) projects: Vec<ProjectEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ModelProfile {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderConfig {
    pub(crate) name: String,
    pub(crate) protocol: String,
    #[serde(default)]
    pub(crate) base_url: Option<String>,
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) profiles: Vec<ModelProfile>,
    #[serde(default)]
    pub(crate) active_profile: Option<String>,
    /// Read-only: true when an API key exists in the OS keychain for this provider.
    /// Not persisted in JSON — computed at runtime when listing configs.
    #[serde(default, skip_deserializing)]
    pub(crate) api_key_stored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ProviderConfigList {
    #[serde(default)]
    pub(crate) providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub(crate) active_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GuiSettingsFile {
    #[serde(default)]
    pub(crate) provider_name: Option<String>,
    #[serde(default)]
    pub(crate) provider_model: Option<String>,
    #[serde(default)]
    pub(crate) provider_base_url: Option<String>,
    #[serde(default)]
    pub(crate) provider_protocol: Option<String>,
    #[serde(default)]
    pub(crate) max_output_tokens: Option<u32>,
    #[serde(default)]
    pub(crate) thinking_budget: Option<Option<u32>>,
    #[serde(default)]
    pub(crate) max_retries: Option<u32>,
    #[serde(default)]
    pub(crate) timeout_ms: Option<u64>,
    #[serde(default)]
    pub(crate) retry_initial_backoff_ms: Option<u64>,
    #[serde(default)]
    pub(crate) retry_max_backoff_ms: Option<u64>,
    #[serde(default)]
    pub(crate) respect_retry_after: Option<bool>,
    #[serde(default)]
    pub(crate) permission_mode: Option<String>,
    #[serde(default)]
    pub(crate) verbose: Option<bool>,
    #[serde(default)]
    pub(crate) codex_model_provider: Option<String>,
    #[serde(default)]
    pub(crate) codex_approval_policy: Option<String>,
    #[serde(default)]
    pub(crate) codex_sandbox_mode: Option<String>,
    #[serde(default)]
    pub(crate) codex_persist_extended_history: Option<bool>,
    #[serde(default)]
    pub(crate) codex_memories_enabled: Option<bool>,
    #[serde(default)]
    pub(crate) codex_thread_store_endpoint: Option<String>,
    #[serde(default)]
    pub(crate) codex_config_overrides: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) codex_permission_profile: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) codex_service_tier: Option<String>,
    #[serde(default)]
    pub(crate) codex_ephemeral: Option<bool>,
    #[serde(default)]
    pub(crate) roo_mode: Option<String>,
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
            roo_mode: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProviderInfoDto {
    pub(crate) name: String,
    pub(crate) model: Option<String>,
    pub(crate) protocol: String,
    pub(crate) base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionSummaryDto {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) cwd: String,
    pub(crate) provider_name: String,
    pub(crate) model: Option<String>,
    pub(crate) agent_type: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) archived: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ToolCallDto {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ConversationEntryDto {
    pub(crate) role: String,
    pub(crate) text: String,
    pub(crate) content_blocks: Vec<serde_json::Value>,
    pub(crate) tool_calls: Vec<ToolCallDto>,
    pub(crate) tool_call_id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PromptResultDto {
    pub(crate) session_id: String,
    pub(crate) text: String,
    pub(crate) tool_calls: Vec<ToolCallDto>,
    pub(crate) usage: UsageDto,
    pub(crate) num_turns: u32,
    pub(crate) stop_reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UsageDto {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) total_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InitResultDto {
    pub(crate) provider: Option<ProviderInfoDto>,
    pub(crate) sessions_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FullSettingsDto {
    pub(crate) provider_name: String,
    pub(crate) provider_model: Option<String>,
    pub(crate) provider_base_url: Option<String>,
    pub(crate) provider_protocol: String,
    pub(crate) provider_api_key_set: bool,
    pub(crate) max_output_tokens: u32,
    pub(crate) thinking_budget: Option<u32>,
    pub(crate) max_retries: u32,
    pub(crate) timeout_ms: u64,
    pub(crate) retry_initial_backoff_ms: u64,
    pub(crate) retry_max_backoff_ms: u64,
    pub(crate) respect_retry_after: bool,
    pub(crate) permission_mode: String,
    pub(crate) max_turns: usize,
    pub(crate) verbose: bool,
    pub(crate) codex_model_provider: Option<String>,
    pub(crate) codex_approval_policy: Option<String>,
    pub(crate) codex_sandbox_mode: Option<String>,
    pub(crate) codex_persist_extended_history: bool,
    pub(crate) codex_memories_enabled: bool,
    pub(crate) codex_thread_store_endpoint: Option<String>,
    pub(crate) codex_config_overrides: BTreeMap<String, String>,
    pub(crate) codex_permission_profile: Option<serde_json::Value>,
    pub(crate) codex_service_tier: Option<String>,
    pub(crate) codex_ephemeral: Option<bool>,
    pub(crate) roo_mode: Option<String>,
    pub(crate) runtime_paths: RuntimePathsDto,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimePathsDto {
    pub(crate) profile_dir: String,
    pub(crate) sessions_dir: String,
    pub(crate) artifacts_dir: String,
    pub(crate) logs_dir: String,
    pub(crate) cache_dir: String,
    pub(crate) agents_dir: String,
    pub(crate) remote_control_file: String,
    pub(crate) gui_projects_file: String,
    pub(crate) gui_providers_file: String,
    pub(crate) gui_settings_file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UpdateProviderRequest {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) provider_name: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) provider_model: Option<String>,
    #[serde(default)]
    pub(crate) base_url: Option<String>,
    #[serde(default)]
    pub(crate) provider_base_url: Option<String>,
    #[serde(default)]
    pub(crate) protocol: Option<String>,
    #[serde(default)]
    pub(crate) provider_protocol: Option<String>,
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    #[serde(default)]
    pub(crate) max_output_tokens: Option<u32>,
    #[serde(default)]
    pub(crate) thinking_budget: Option<Option<u32>>,
    #[serde(default)]
    pub(crate) max_retries: Option<u32>,
    #[serde(default)]
    pub(crate) timeout_ms: Option<u64>,
    #[serde(default)]
    pub(crate) retry_initial_backoff_ms: Option<u64>,
    #[serde(default)]
    pub(crate) retry_max_backoff_ms: Option<u64>,
    #[serde(default)]
    pub(crate) respect_retry_after: Option<bool>,
    #[serde(default)]
    pub(crate) permission_mode: Option<String>,
    #[serde(default)]
    pub(crate) verbose: Option<bool>,
    #[serde(default)]
    pub(crate) codex_model_provider: Option<String>,
    #[serde(default)]
    pub(crate) codex_approval_policy: Option<String>,
    #[serde(default)]
    pub(crate) codex_sandbox_mode: Option<String>,
    #[serde(default)]
    pub(crate) codex_persist_extended_history: Option<bool>,
    #[serde(default)]
    pub(crate) codex_memories_enabled: Option<bool>,
    #[serde(default)]
    pub(crate) codex_thread_store_endpoint: Option<String>,
    #[serde(default)]
    pub(crate) codex_config_overrides: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub(crate) codex_permission_profile: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) codex_service_tier: Option<String>,
    #[serde(default)]
    pub(crate) codex_ephemeral: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProjectInfoDto {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) session_count: usize,
    pub(crate) is_auto_detected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PermissionRequestDto {
    pub(crate) request_id: String,
    pub(crate) tool_name: String,
    pub(crate) tool_use_id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) input: serde_json::Value,
    pub(crate) blocked_path: Option<String>,
    pub(crate) permission_suggestions: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PermissionDecisionDto {
    pub(crate) request_id: String,
    pub(crate) allowed: bool,
    pub(crate) message: Option<String>,
    pub(crate) updated_input: Option<serde_json::Value>,
    pub(crate) permission_updates: Vec<claude_permissions::PermissionUpdate>,
    pub(crate) feedback: Option<String>,
    pub(crate) content_blocks: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ToolProgressDto {
    pub(crate) tool_call_id: String,
    pub(crate) tool_name: String,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) active_form: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ToolResultDto {
    pub(crate) tool_call_id: String,
    pub(crate) tool_name: String,
    pub(crate) is_error: bool,
    pub(crate) output: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StreamingDeltaDto {
    pub(crate) session_id: String,
    pub(crate) delta: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PromptDoneDto {
    pub(crate) session_id: String,
    pub(crate) is_error: bool,
    pub(crate) error: Option<String>,
    pub(crate) result: Option<PromptResultDto>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubtaskStartedDto {
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) parent_task_id: Option<String>,
    pub(crate) description: String,
    pub(crate) depth: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubtaskProgressDto {
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) turn: u32,
    pub(crate) max_turns: u32,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubtaskCompletedDto {
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) success: bool,
    pub(crate) output_preview: String,
    pub(crate) turns_used: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BatchProgressDto {
    pub(crate) session_id: String,
    pub(crate) total: usize,
    pub(crate) completed: usize,
    pub(crate) running: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionTaskDto {
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) parent_task_id: Option<String>,
    pub(crate) description: String,
    pub(crate) depth: u32,
    pub(crate) status: String,
    pub(crate) summary: String,
    pub(crate) output_preview: Option<String>,
    pub(crate) turns_used: Option<u32>,
    pub(crate) kind: String,
    pub(crate) output_path: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TaskSnapshotDto {
    pub(crate) session_id: String,
    pub(crate) tasks: Vec<SessionTaskDto>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContextUsageDto {
    pub(crate) session_id: String,
    pub(crate) estimated_tokens: u64,
    pub(crate) max_input_tokens: u64,
    pub(crate) threshold_tokens: u64,
    pub(crate) ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContextOverflowDto {
    pub(crate) session_id: String,
    pub(crate) estimated_tokens: u64,
    pub(crate) max_input_tokens: u64,
    pub(crate) threshold_tokens: u64,
    pub(crate) ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContextCompactedDto {
    pub(crate) session_id: String,
    pub(crate) entries_removed: usize,
    pub(crate) usage_ratio: f64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTypeInfoDto {
    pub(crate) agent_type: String,
    pub(crate) display_name: String,
    pub(crate) available: bool,
    pub(crate) installed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConfigScopeDto {
    Profile,
    Project,
}

impl ConfigScopeDto {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Project => "project",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionExportFormatDto {
    Json,
    Ndjson,
}

impl SessionExportFormatDto {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Ndjson => "ndjson",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionExportResultDto {
    pub(crate) session_id: String,
    pub(crate) format: String,
    pub(crate) path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticBundleRequestDto {
    #[serde(default = "default_true")]
    pub(crate) include_logs: bool,
    #[serde(default)]
    pub(crate) include_settings: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticBundleResultDto {
    pub(crate) path: String,
    pub(crate) files: usize,
    pub(crate) bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FrontendLogEventDto {
    pub(crate) level: String,
    pub(crate) source: String,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) details: Option<String>,
    #[serde(default)]
    pub(crate) stack: Option<String>,
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) line: Option<u32>,
    #[serde(default)]
    pub(crate) column: Option<u32>,
    #[serde(default)]
    pub(crate) user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorReportDto {
    pub(crate) ok: bool,
    pub(crate) runtime: GuiDoctorRuntimeDto,
    pub(crate) provider: GuiDoctorProviderDto,
    pub(crate) tools: GuiDoctorToolsDto,
    pub(crate) permissions: GuiDoctorPermissionsDto,
    pub(crate) extensions: GuiDoctorExtensionsDto,
    pub(crate) mcp_runtime: GuiDoctorMcpRuntimeDto,
    pub(crate) network: Vec<GuiDoctorProbeDto>,
    pub(crate) env_providers: Vec<GuiDoctorEnvProviderDto>,
    pub(crate) issues: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorRuntimeDto {
    pub(crate) version: String,
    pub(crate) cwd: String,
    pub(crate) profile_dir: String,
    pub(crate) session_id: String,
    pub(crate) session_name: Option<String>,
    pub(crate) permission_mode: String,
    pub(crate) setting_sources: Vec<String>,
    pub(crate) allowed_setting_sources: Vec<String>,
    pub(crate) settings_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorProviderDto {
    pub(crate) name: String,
    pub(crate) protocol: String,
    pub(crate) base_url: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) api_key_present: bool,
    pub(crate) auth_source: Option<String>,
    pub(crate) effort: Option<String>,
    pub(crate) fallback_model: Option<String>,
    pub(crate) context_window_tokens: u64,
    pub(crate) output_reserve_tokens: u64,
    pub(crate) multimodal: bool,
    pub(crate) reasoning: bool,
    pub(crate) validation_ok: bool,
    pub(crate) validation_issues: Vec<String>,
    pub(crate) probe: Option<GuiDoctorProbeDto>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorToolsDto {
    pub(crate) builtin_tools: usize,
    pub(crate) allowed_tools: Vec<String>,
    pub(crate) disallowed_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorRuleSourceDto {
    pub(crate) source: String,
    pub(crate) count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorPermissionsDto {
    pub(crate) layered_rules: usize,
    pub(crate) rule_sources: Vec<GuiDoctorRuleSourceDto>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorExtensionsDto {
    pub(crate) skills: usize,
    pub(crate) plugins: usize,
    pub(crate) disabled_plugins: usize,
    pub(crate) managed_mcp_servers: usize,
    pub(crate) plugin_mcp_servers: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorMcpRuntimeDto {
    pub(crate) probed: bool,
    pub(crate) summary: UiRuntimeMcpInventorySummary,
    pub(crate) servers: Vec<GuiDoctorMcpRuntimeServerDto>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorMcpRuntimeServerDto {
    pub(crate) name: String,
    pub(crate) status: UiRuntimeMcpServerStatus,
    pub(crate) enabled: bool,
    pub(crate) origin_kind: String,
    pub(crate) origin_name: String,
    pub(crate) config_path: String,
    pub(crate) tool_count: usize,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorEnvProviderDto {
    pub(crate) name: String,
    pub(crate) protocol: String,
    pub(crate) base_url: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) api_key_present: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GuiDoctorProbeOutcomeDto {
    Reachable,
    AuthRejected,
    RateLimited,
    ServerError,
    TransportError,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GuiDoctorProbeDto {
    pub(crate) label: String,
    pub(crate) url: String,
    pub(crate) outcome: GuiDoctorProbeOutcomeDto,
    pub(crate) status_code: Option<u16>,
    pub(crate) latency_ms: u128,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct McpServerListDto {
    pub(crate) scope: String,
    pub(crate) config_path: String,
    pub(crate) warnings: Vec<String>,
    pub(crate) servers: Vec<McpServerDto>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct McpServerDto {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) transport: String,
    pub(crate) config_path: String,
    pub(crate) command: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) env_keys: Vec<String>,
    pub(crate) metadata_keys: Vec<String>,
    pub(crate) startup_timeout_secs: Option<u64>,
    pub(crate) request_timeout_secs: Option<u64>,
    pub(crate) live: Option<McpServerLiveDto>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct McpServerLiveDto {
    pub(crate) status: String,
    pub(crate) protocol_version: Option<String>,
    pub(crate) peer_name: Option<String>,
    pub(crate) peer_version: Option<String>,
    pub(crate) tool_count: usize,
    pub(crate) tools: Vec<McpToolInfoDto>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct McpToolInfoDto {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub(crate) input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeMcpInventoryDto {
    pub(crate) effective_cwd: String,
    pub(crate) warnings: Vec<String>,
    pub(crate) summary: UiRuntimeMcpInventorySummary,
    pub(crate) servers: Vec<RuntimeMcpServerDto>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeMcpServerDto {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) enabled: bool,
    pub(crate) origin_kind: String,
    pub(crate) origin_name: String,
    pub(crate) config_path: String,
    pub(crate) transport: String,
    pub(crate) command: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) env_keys: Vec<String>,
    pub(crate) metadata_keys: Vec<String>,
    pub(crate) startup_timeout_secs: Option<u64>,
    pub(crate) request_timeout_secs: Option<u64>,
    pub(crate) live: Option<McpServerLiveDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct McpServerUpsertRequestDto {
    pub(crate) scope: ConfigScopeDto,
    #[serde(default)]
    pub(crate) project_path: Option<String>,
    pub(crate) name: String,
    pub(crate) transport: String,
    #[serde(default)]
    pub(crate) command: Option<String>,
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    #[serde(default)]
    pub(crate) cwd: Option<String>,
    #[serde(default)]
    pub(crate) env: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) headers: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) metadata: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) disabled: bool,
    #[serde(default)]
    pub(crate) startup_timeout_secs: Option<u64>,
    #[serde(default)]
    pub(crate) request_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct McpMutationResultDto {
    pub(crate) status: String,
    pub(crate) scope: String,
    pub(crate) config_path: String,
    pub(crate) name: Option<String>,
    pub(crate) enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadRefRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    #[serde(default = "default_true")]
    pub(crate) include_turns: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexNativeParamsRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadNativeParamsRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    #[serde(default)]
    pub(crate) params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadArchiveRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadSetNameRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadMetadataUpdateRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    #[serde(default)]
    pub(crate) sha: Option<Option<String>>,
    #[serde(default)]
    pub(crate) branch: Option<Option<String>>,
    #[serde(default)]
    pub(crate) origin_url: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadShellCommandRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    pub(crate) command: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadLoadedListRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadGoalRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadGoalSetUiRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) thread_id: String,
    #[serde(default)]
    pub(crate) text: String,
    pub(crate) status: Option<String>,
    pub(crate) token_budget: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadRollbackUiRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    pub(crate) num_turns: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadTurnsListRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    #[serde(default)]
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexTurnSteerUiRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    pub(crate) expected_turn_id: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexTurnInterruptUiRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    #[serde(default)]
    pub(crate) turn_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexExperimentalFeatureSetRequest {
    pub(crate) feature: String,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexSkillsListRequest {
    #[serde(default)]
    pub(crate) cwds: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) force_reload: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexSkillsConfigWriteRequest {
    pub(crate) skill_id: String,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexPluginListRequest {
    #[serde(default)]
    pub(crate) cwds: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexPluginIdRequest {
    pub(crate) plugin_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexPluginInstallRequest {
    pub(crate) source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexMarketplaceRequest {
    pub(crate) source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexMcpOAuthLoginRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) server: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexReviewStartRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    #[serde(default)]
    pub(crate) prompt: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexExecWriteRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) process_id: String,
    #[serde(default)]
    pub(crate) delta_base64: Option<String>,
    #[serde(default)]
    pub(crate) close_stdin: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexExecResizeRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) process_id: String,
    pub(crate) rows: u16,
    pub(crate) cols: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexMcpStatusRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) detail: Option<String>,
    #[serde(default)]
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexMcpResourceReadRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) server: String,
    pub(crate) uri: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexMcpToolCallRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    pub(crate) server: String,
    pub(crate) tool: String,
    #[serde(default)]
    pub(crate) arguments: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexConfigValueWriteRequest {
    pub(crate) key_path: String,
    pub(crate) value: serde_json::Value,
    #[serde(default)]
    pub(crate) merge_strategy: Option<String>,
    #[serde(default)]
    pub(crate) file_path: Option<String>,
    #[serde(default)]
    pub(crate) expected_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexConfigBatchEditRequest {
    pub(crate) key_path: String,
    pub(crate) value: serde_json::Value,
    #[serde(default)]
    pub(crate) merge_strategy: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexConfigBatchWriteRequest {
    pub(crate) edits: Vec<CodexConfigBatchEditRequest>,
    #[serde(default)]
    pub(crate) file_path: Option<String>,
    #[serde(default)]
    pub(crate) expected_version: Option<String>,
    #[serde(default)]
    pub(crate) reload_user_config: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexExternalAgentConfigDetectRequest {
    #[serde(default)]
    pub(crate) include_home: bool,
    #[serde(default)]
    pub(crate) cwds: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexMemoryModeRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) thread_id: String,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexAppServerRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) method: String,
    #[serde(default)]
    pub(crate) params: Option<serde_json::Value>,
}

pub(crate) fn default_true() -> bool {
    true
}
