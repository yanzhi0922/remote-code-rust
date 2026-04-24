export interface ProviderInfo {
  name: string;
  model: string | null;
  protocol: string;
  base_url: string | null;
}

export interface RuntimeProviderStatus {
  name: string;
  model: string | null;
  protocol: string;
  base_url: string | null;
  auth_source: string | null;
  effort: string | null;
  fallback_model: string | null;
}

export interface RuntimeMcpOriginCounts {
  cwd: number;
  profile: number;
  explicit: number;
  plugin: number;
}

export interface RuntimeMcpInventorySummary {
  total_servers: number;
  enabled_servers: number;
  disabled_servers: number;
  unique_server_names: number;
  ambiguous_server_names: number;
  warning_count: number;
  origins: RuntimeMcpOriginCounts;
}

export interface RuntimeStatusInfo {
  session_name: string | null;
  provider: RuntimeProviderStatus;
  permission_mode: string;
  setting_sources: string[];
  allowed_setting_sources: string[];
  allowed_tools: string[];
  disallowed_tools: string[];
  mcp: RuntimeMcpInventorySummary;
}

export type ConfigScope = 'profile' | 'project';
export type SessionExportFormat = 'json' | 'ndjson';

export interface SessionExportResult {
  session_id: string;
  format: SessionExportFormat;
  path: string;
}

export type DoctorProbeOutcome =
  | 'reachable'
  | 'auth_rejected'
  | 'rate_limited'
  | 'server_error'
  | 'transport_error';

export interface DoctorProbeInfo {
  label: string;
  url: string;
  outcome: DoctorProbeOutcome;
  status_code: number | null;
  latency_ms: number;
  detail: string;
}

export interface DoctorRuntimeInfo {
  version: string;
  cwd: string;
  profile_dir: string;
  session_id: string;
  session_name: string | null;
  permission_mode: string;
  setting_sources: string[];
  allowed_setting_sources: string[];
  settings_files: string[];
}

export interface DoctorProviderInfo {
  name: string;
  protocol: string;
  base_url: string | null;
  model: string | null;
  api_key_present: boolean;
  auth_source: string | null;
  effort: string | null;
  fallback_model: string | null;
  context_window_tokens: number;
  output_reserve_tokens: number;
  multimodal: boolean;
  reasoning: boolean;
  validation_ok: boolean;
  validation_issues: string[];
  probe: DoctorProbeInfo | null;
}

export interface DoctorToolsInfo {
  builtin_tools: number;
  allowed_tools: string[];
  disallowed_tools: string[];
}

export interface DoctorRuleSourceInfo {
  source: string;
  count: number;
}

export interface DoctorPermissionsInfo {
  layered_rules: number;
  rule_sources: DoctorRuleSourceInfo[];
}

export interface DoctorExtensionsInfo {
  skills: number;
  plugins: number;
  disabled_plugins: number;
  managed_mcp_servers: number;
  plugin_mcp_servers: number;
}

export interface DoctorEnvProviderInfo {
  name: string;
  protocol: string;
  base_url: string | null;
  model: string | null;
  api_key_present: boolean;
}

export interface DoctorReportInfo {
  ok: boolean;
  runtime: DoctorRuntimeInfo;
  provider: DoctorProviderInfo;
  tools: DoctorToolsInfo;
  permissions: DoctorPermissionsInfo;
  extensions: DoctorExtensionsInfo;
  network: DoctorProbeInfo[];
  env_providers: DoctorEnvProviderInfo[];
  issues: string[];
  warnings: string[];
}

export interface McpToolInfo {
  name: string;
  description: string | null;
  inputSchema?: unknown;
}

export interface McpServerLiveInfo {
  status: string;
  protocol_version: string | null;
  peer_name: string | null;
  peer_version: string | null;
  tool_count: number;
  tools: McpToolInfo[];
  error: string | null;
}

export interface McpServerInfo {
  name: string;
  enabled: boolean;
  transport: string;
  config_path: string;
  command: string | null;
  url: string | null;
  args: string[];
  cwd: string | null;
  env_keys: string[];
  metadata_keys: string[];
  startup_timeout_secs: number | null;
  request_timeout_secs: number | null;
  live: McpServerLiveInfo | null;
}

export interface McpServerListInfo {
  scope: ConfigScope;
  config_path: string;
  warnings: string[];
  servers: McpServerInfo[];
}

export interface RuntimeMcpServerInfo {
  name: string;
  status: string;
  enabled: boolean;
  origin_kind: string;
  origin_name: string;
  config_path: string;
  transport: string;
  command: string | null;
  url: string | null;
  args: string[];
  cwd: string | null;
  env_keys: string[];
  metadata_keys: string[];
  startup_timeout_secs: number | null;
  request_timeout_secs: number | null;
  live: McpServerLiveInfo | null;
}

export interface RuntimeMcpInventoryInfo {
  effective_cwd: string;
  warnings: string[];
  servers: RuntimeMcpServerInfo[];
}

export interface McpMutationResult {
  status: string;
  scope: ConfigScope;
  config_path: string;
  name: string | null;
  enabled: boolean | null;
}

export interface McpServerDraft {
  scope: ConfigScope;
  project_path?: string | null;
  name: string;
  transport: 'stdio' | 'http' | 'websocket';
  command?: string | null;
  url?: string | null;
  args?: string[];
  cwd?: string | null;
  env?: Record<string, string>;
  headers?: Record<string, string>;
  metadata?: Record<string, string>;
  disabled?: boolean;
  startup_timeout_secs?: number | null;
  request_timeout_secs?: number | null;
}

export interface SessionSummary {
  id: string;
  title: string;
  cwd: string;
  provider_name: string;
  model: string | null;
  created_at: string;
  updated_at: string;
  archived: boolean;
}

export type ConversationRole = 'system' | 'user' | 'assistant' | 'tool';

export interface ToolCallInfo {
  id: string;
  name: string;
  input: unknown;
}

export interface ConversationEntry {
  role: ConversationRole;
  text: string;
  content_blocks: unknown[];
  tool_calls: ToolCallInfo[];
  tool_call_id: string | null;
  name: string | null;
  is_error: boolean;
}

export interface UsageInfo {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
}

export interface PromptResult {
  session_id: string;
  text: string;
  tool_calls: ToolCallInfo[];
  usage: UsageInfo;
  num_turns: number;
  stop_reason: string;
}

export interface InitResult {
  provider: ProviderInfo | null;
  sessions_count: number;
}

export interface FullSettings {
  provider_name: string;
  provider_model: string | null;
  provider_base_url: string | null;
  provider_protocol: string;
  provider_api_key_set: boolean;
  max_retries: number;
  timeout_ms: number;
  retry_initial_backoff_ms: number;
  retry_max_backoff_ms: number;
  respect_retry_after: boolean;
  permission_mode: string;
  verbose: boolean;
}

export interface UpdateProviderRequest {
  name?: string;
  provider_name?: string;
  model?: string | null;
  provider_model?: string | null;
  base_url?: string | null;
  provider_base_url?: string | null;
  protocol?: string;
  provider_protocol?: string;
  api_key?: string;
  max_retries?: number;
  timeout_ms?: number;
  retry_initial_backoff_ms?: number;
  retry_max_backoff_ms?: number;
  respect_retry_after?: boolean;
  permission_mode?: string;
  verbose?: boolean;
}

export interface ModelProfile {
  name: string;
  model?: string;
}

export interface ProviderConfig {
  name: string;
  protocol: string;
  base_url?: string;
  api_key?: string;
  model?: string;
  profiles?: ModelProfile[];
  active_profile?: string;
  /** True when an API key is securely stored in the OS keychain. */
  api_key_stored?: boolean;
}

export interface ProviderConfigList {
  providers: ProviderConfig[];
  active_provider?: string;
}

export interface ProjectInfo {
  path: string;
  name: string;
  session_count: number;
  is_auto_detected: boolean;
}

export interface PermissionRequestInfo {
  request_id: string;
  tool_name: string;
  tool_use_id: string;
  title: string;
  description: string;
  input: unknown;
  blocked_path: string | null;
  permission_suggestions: unknown[];
}

export interface PermissionDecisionInfo {
  request_id: string;
  allowed: boolean;
  message?: string | null;
  updated_input?: unknown;
  permission_updates?: unknown[];
  feedback?: string | null;
  content_blocks?: unknown[];
}

export interface ToolProgressInfo {
  tool_call_id: string;
  tool_name: string;
  message: string;
}

export interface ToolResultInfo {
  tool_call_id: string;
  tool_name: string;
  is_error: boolean;
  output: string;
}

export interface StreamingDeltaInfo {
  session_id: string;
  delta: string;
}

export interface PromptDoneInfo {
  session_id: string;
  is_error: boolean;
  error: string | null;
  result: PromptResult | null;
}

export interface SubtaskStartedInfo {
  session_id: string;
  task_id: string;
  parent_task_id: string | null;
  description: string;
  depth: number;
}

export interface SubtaskProgressInfo {
  session_id: string;
  task_id: string;
  turn: number;
  max_turns: number;
  summary: string;
}

export interface SubtaskCompletedInfo {
  session_id: string;
  task_id: string;
  success: boolean;
  output_preview: string;
  turns_used: number;
}

export interface BatchProgressInfo {
  session_id: string;
  total: number;
  completed: number;
  running: number;
}

export interface ContextUsageInfo {
  session_id: string;
  estimated_tokens: number;
  max_input_tokens: number;
  threshold_tokens: number;
  ratio: number;
}

export interface ContextOverflowInfo {
  session_id: string;
  estimated_tokens: number;
  max_input_tokens: number;
  threshold_tokens: number;
  ratio: number;
}

export interface ContextCompactedInfo {
  session_id: string;
  entries_removed: number;
  usage_ratio: number;
}

export type SubtaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'stopped';

export interface SessionSubtask {
  session_id: string;
  task_id: string;
  parent_task_id: string | null;
  description: string;
  depth: number;
  status: SubtaskStatus;
  summary: string;
  output_preview: string | null;
  turns_used: number | null;
  kind?: 'background' | 'delegation' | 'batch';
  output_path?: string | null;
  created_at?: string;
  updated_at?: string;
}

export interface TaskSnapshotInfo {
  session_id: string;
  tasks: SessionSubtask[];
}

// ── Scheduled Tasks ──────────────────────────────────────────────────

export type ScheduledTaskWizardData = {
  name?: string;
  description?: string;
  prompt?: string;
  model?: string;
  permissionMode?: string;
  folder?: string;
  worktree?: boolean;
  frequency?: string;
  scheduledTime?: string;
  cron?: string;
};

// ── Context Visualization ────────────────────────────────────────────

export interface ContextCategory {
  name: string;
  tokens: number;
  color: string;
}

export interface ContextData {
  categories: ContextCategory[];
  totalTokens: number;
  maxTokens: number;
  percentage: number;
  model: string;
}

// ── Task List V2 ─────────────────────────────────────────────────────

export type TaskStatus = 'pending' | 'in_progress' | 'completed';

export interface TaskItem {
  id: string;
  title: string;
  status: TaskStatus;
  owner?: string;
  blockedBy: string[];
}

// ── File Edit Diff ───────────────────────────────────────────────────

export interface FileEdit {
  old_string: string;
  new_string: string;
  replace_all?: boolean;
}

export interface DiffLine {
  type: 'add' | 'remove' | 'context';
  content: string;
  lineNumber?: number;
}

// ── Agent Progress ───────────────────────────────────────────────────

export interface AgentProgressInfo {
  agentType: string;
  description?: string;
  name?: string;
  toolUseCount: number;
  tokens: number | null;
  isResolved: boolean;
  isError: boolean;
  isAsync?: boolean;
  lastToolInfo?: string | null;
}

// ── Coordinator Agent ────────────────────────────────────────────────

export interface CoordinatorTask {
  id: string;
  name?: string;
  status: TaskStatus;
  description: string;
  startTime: number;
  endTime?: number;
  tokenCount?: number;
}

// ── Teammate View ────────────────────────────────────────────────────

export interface TeammateInfo {
  agentName: string;
  color?: string;
  prompt?: string;
}

// ── Session Preview ──────────────────────────────────────────────────

export interface SessionInfo {
  id: string;
  title: string;
  messageCount: number;
  modified: string;
  gitBranch?: string;
  messages: unknown[];
}

// ── Quick Open ───────────────────────────────────────────────────────

export interface QuickOpenResult {
  path: string;
  label: string;
}

// ── Compact Summary ──────────────────────────────────────────────────

export interface SummarizeMetadata {
  messagesSummarized: number;
  direction: 'up_to' | 'from_here';
  userContext?: string;
}

// ── Memory Usage ─────────────────────────────────────────────────────

export type MemoryStatus = 'normal' | 'high' | 'critical';

export interface MemoryUsageData {
  heapUsed: number;
  status: MemoryStatus;
}
