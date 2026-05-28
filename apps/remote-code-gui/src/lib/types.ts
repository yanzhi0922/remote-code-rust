export interface ProviderInfo {
  name: string;
  model: string | null;
  protocol: string;
  base_url: string | null;
}

interface RuntimeProviderStatus {
  name: string;
  model: string | null;
  protocol: string;
  base_url: string | null;
  auth_source: string | null;
  effort: string | null;
  fallback_model: string | null;
}

interface RuntimeMcpOriginCounts {
  cwd: number;
  profile: number;
  explicit: number;
  plugin: number;
}

interface RuntimeMcpStatusCounts {
  connected: number;
  failed: number;
  needs_auth: number;
  pending: number;
  disabled: number;
}

interface RuntimeMcpInventorySummary {
  total_servers: number;
  enabled_servers: number;
  disabled_servers: number;
  unique_server_names: number;
  ambiguous_server_names: number;
  warning_count: number;
  origins: RuntimeMcpOriginCounts;
  status_counts: RuntimeMcpStatusCounts;
}

export interface RuntimeStatusInfo {
  session_name: string | null;
  provider: RuntimeProviderStatus;
  permission_mode: string;
  output_style: string | null;
  language: string | null;
  brief_enabled: boolean;
  proactive_active: boolean;
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

type DoctorProbeOutcome =
  | 'reachable'
  | 'auth_rejected'
  | 'rate_limited'
  | 'server_error'
  | 'transport_error';


interface DoctorProbeInfo {
  label: string;
  url: string;
  outcome: DoctorProbeOutcome;
  status_code: number | null;
  latency_ms: number;
  detail: string;
}

interface DoctorRuntimeInfo {
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

interface DoctorProviderInfo {
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

interface DoctorToolsInfo {
  builtin_tools: number;
  allowed_tools: string[];
  disallowed_tools: string[];
}

interface DoctorRuleSourceInfo {
  source: string;
  count: number;
}

interface DoctorPermissionsInfo {
  layered_rules: number;
  rule_sources: DoctorRuleSourceInfo[];
}

interface DoctorExtensionsInfo {
  skills: number;
  plugins: number;
  disabled_plugins: number;
  managed_mcp_servers: number;
  plugin_mcp_servers: number;
}

interface DoctorMcpRuntimeServerInfo {
  name: string;
  status: string;
  enabled: boolean;
  origin_kind: string;
  origin_name: string;
  config_path: string;
  tool_count: number;
  error: string | null;
}

interface DoctorMcpRuntimeInfo {
  probed: boolean;
  summary: RuntimeMcpInventorySummary;
  servers: DoctorMcpRuntimeServerInfo[];
}

interface DoctorEnvProviderInfo {
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
  mcp_runtime: DoctorMcpRuntimeInfo;
  network: DoctorProbeInfo[];
  env_providers: DoctorEnvProviderInfo[];
  issues: string[];
  warnings: string[];
}

interface McpToolInfo {
  name: string;
  description: string | null;
  inputSchema?: unknown;
}

interface McpServerLiveInfo {
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
  summary: RuntimeMcpInventorySummary;
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
  agent_type: AgentType;
  created_at: string;
  updated_at: string;
  archived: boolean;
}

type ConversationRole = 'system' | 'user' | 'assistant' | 'tool';

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

interface UsageInfo {
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
  max_output_tokens: number;
  thinking_budget: number | null;
  max_retries: number;
  timeout_ms: number;
  retry_initial_backoff_ms: number;
  retry_max_backoff_ms: number;
  respect_retry_after: boolean;
  permission_mode: string;
  max_turns: number;
  verbose: boolean;
  codex_model_provider: string | null;
  codex_approval_policy: string | null;
  codex_sandbox_mode: string | null;
  codex_persist_extended_history: boolean;
  codex_memories_enabled: boolean;
  codex_thread_store_endpoint: string | null;
  codex_config_overrides: Record<string, string>;
  codex_permission_profile: unknown | null;
  codex_service_tier: string | null;
  codex_ephemeral: boolean | null;
  runtime_paths: RuntimePathsInfo;
}

export interface RuntimePathsInfo {
  profile_dir: string;
  sessions_dir: string;
  artifacts_dir: string;
  logs_dir: string;
  cache_dir: string;
  agents_dir: string;
  remote_control_file: string;
  gui_projects_file: string;
  gui_providers_file: string;
  gui_settings_file: string;
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
  max_output_tokens?: number;
  thinking_budget?: number | null;
  max_retries?: number;
  timeout_ms?: number;
  retry_initial_backoff_ms?: number;
  retry_max_backoff_ms?: number;
  respect_retry_after?: boolean;
  permission_mode?: string;
  verbose?: boolean;
  codex_model_provider?: string | null;
  codex_approval_policy?: string | null;
  codex_sandbox_mode?: string | null;
  codex_persist_extended_history?: boolean;
  codex_memories_enabled?: boolean;
  codex_thread_store_endpoint?: string | null;
  codex_config_overrides?: Record<string, string>;
  codex_permission_profile?: unknown | null;
  codex_service_tier?: string | null;
  codex_ephemeral?: boolean | null;
}

export interface CodexThreadListRequest {
  cursor?: string | null;
  limit?: number | null;
  sortKey?: 'created_at' | 'updated_at' | 'createdAt' | 'updatedAt' | null;
  sortDirection?: 'asc' | 'desc' | null;
  modelProviders?: string[] | null;
  sourceKinds?: string[] | null;
  archived?: boolean | null;
  cwd?: string | string[] | null;
  useStateDbOnly?: boolean;
  searchTerm?: string | null;
}

export interface CodexThreadRefRequest {
  sessionId?: string | null;
  threadId: string;
  includeTurns?: boolean;
}

export interface CodexThreadArchiveRequest {
  sessionId?: string | null;
  threadId: string;
}

export interface CodexExecRequest {
  command: string[];
  processId?: string | null;
  tty?: boolean;
  streamStdin?: boolean;
  streamStdoutStderr?: boolean;
  outputBytesCap?: number | null;
  disableOutputCap?: boolean;
  disableTimeout?: boolean;
  timeoutMs?: number | null;
  cwd?: string | null;
  env?: Record<string, string | null> | null;
  sandboxPolicy?: unknown;
  permissionProfile?: unknown;
}

export interface CodexAppServerRequest {
  sessionId?: string | null;
  method: string;
  params?: unknown;
}

export type CodexJsonValue = object | unknown[] | string | number | boolean | null;

export interface CodexThreadSetNameRequest {
  sessionId?: string | null;
  threadId: string;
  name: string;
}

export interface CodexThreadGoalRequest {
  sessionId?: string | null;
  threadId: string;
}

export interface CodexThreadGoalSetRequest extends CodexThreadGoalRequest {
  text: string;
  status?: string | null;
  tokenBudget?: number | null;
}

/** ThreadGoal 数据模型 — 与 Rust ThreadGoal (protocol v2) 对齐 */
export interface CodexThreadGoalInfo {
  threadId: string;
  objective: string;
  status: 'Active' | 'Paused' | 'BudgetLimited' | 'Complete';
  tokenBudget: number | null;
  tokensUsed: number;
  timeUsedSeconds: number;
  createdAt: number;
  updatedAt: number;
}

/** Goal 状态信息 — 用于 useAppStore */
export interface CodexGoalState {
  goal: CodexThreadGoalInfo;
  lastUpdated: number;
}

export interface CodexThreadRollbackRequest extends CodexThreadGoalRequest {
  numTurns: number;
}

export interface CodexThreadTurnsListRequest extends CodexThreadGoalRequest {
  cursor?: string | null;
  limit?: number | null;
}

export interface CodexTurnSteerRequest extends CodexThreadGoalRequest {
  expectedTurnId: string;
  message: string;
}

export interface CodexTurnInterruptRequest extends CodexThreadGoalRequest {
  turnId?: string | null;
}

export interface CodexExperimentalFeatureSetRequest {
  feature: string;
  enabled: boolean;
}

export interface CodexSkillsListRequest {
  cwds?: string[] | null;
  forceReload?: boolean | null;
}

type CodexNativeParams = Record<string, unknown>;

export interface CodexThreadNativeRequest {
  sessionId?: string | null;
  threadId?: string | null;
  params?: CodexNativeParams | null;
}

export interface CodexTurnStartRequest extends CodexThreadNativeRequest {
  prompt?: string | null;
}

export interface CodexThreadShellCommandRequest extends CodexThreadNativeRequest {
  command?: string | string[] | null;
  cwd?: string | null;
}

export interface CodexThreadMetadataUpdateRequest extends CodexThreadNativeRequest {
  sha?: string | null;
  branch?: string | null;
  originUrl?: string | null;
}

export interface CodexAccountLoginRequest {
  params?: CodexNativeParams | null;
}

export interface CodexExternalAgentConfigImportRequest {
  params?: CodexNativeParams | null;
}

export interface CodexRealtimeRequest {
  params?: CodexNativeParams | null;
}

export interface CodexRealtimeAppendTextRequest {
  text: string;
  params?: CodexNativeParams | null;
}

export interface CodexDeviceKeySignRequest {
  payload: string;
  params?: CodexNativeParams | null;
}

export interface CodexFsPathRequest {
  path: string;
  params?: CodexNativeParams | null;
}

export interface CodexFsWriteFileRequest extends CodexFsPathRequest {
  contents: string;
}

export interface CodexFsCopyRequest {
  from: string;
  to: string;
  params?: CodexNativeParams | null;
}

export interface CodexFuzzyFileSearchRequest {
  query: string;
  cwd?: string | null;
  params?: CodexNativeParams | null;
}

export interface CodexSkillsConfigWriteRequest {
  skillId: string;
  enabled: boolean;
  cwd?: string | null;
}

export interface CodexPluginListRequest {
  cwds?: string[] | null;
}

export interface CodexPluginReadRequest {
  pluginId: string;
}

export interface CodexPluginInstallRequest {
  source: string;
}

export interface CodexPluginUninstallRequest {
  pluginId: string;
}

export interface CodexMarketplaceRequest {
  source: string;
}

export interface CodexMcpOAuthLoginRequest {
  sessionId?: string | null;
  server: string;
}

export interface CodexReviewStartRequest {
  sessionId?: string | null;
  threadId: string;
  prompt?: string | null;
}

export interface CodexExecWriteRequest {
  sessionId?: string | null;
  processId: string;
  deltaBase64?: string | null;
  closeStdin?: boolean;
}

export interface CodexExecResizeRequest {
  sessionId?: string | null;
  processId: string;
  rows: number;
  cols: number;
}

export interface CodexMcpStatusRequest {
  sessionId?: string | null;
  detail?: 'full' | 'toolsAndAuthOnly' | null;
  cursor?: string | null;
  limit?: number | null;
}

export interface CodexMcpResourceReadRequest {
  sessionId?: string | null;
  server: string;
  uri: string;
}

export interface CodexMcpToolCallRequest {
  sessionId?: string | null;
  threadId: string;
  server: string;
  tool: string;
  arguments?: unknown;
  meta?: unknown;
}

export interface CodexConfigValueWriteRequest {
  keyPath: string;
  value: unknown;
  mergeStrategy?: 'replace' | 'upsert' | null;
  filePath?: string | null;
  expectedVersion?: string | null;
}

interface CodexConfigBatchEditRequest {
  keyPath: string;
  value: unknown;
  mergeStrategy?: 'replace' | 'upsert' | null;
}

export interface CodexConfigBatchWriteRequest {
  edits: CodexConfigBatchEditRequest[];
  filePath?: string | null;
  expectedVersion?: string | null;
  reloadUserConfig?: boolean;
}

export interface CodexFeedbackRequest {
  classification: string;
  reason?: string | null;
  threadId?: string | null;
  includeLogs?: boolean;
  extraLogFiles?: string[] | null;
  tags?: Record<string, string> | null;
}

export interface CodexMemoryModeRequest {
  sessionId?: string | null;
  threadId: string;
  enabled: boolean;
}

export interface CodexThreadSummary {
  id: string;
  forkedFromId?: string | null;
  preview: string;
  ephemeral?: boolean;
  modelProvider: string;
  createdAt: number;
  updatedAt: number;
  status: string | Record<string, unknown>;
  path?: string | null;
  cwd: string;
  cliVersion?: string;
  source?: string | Record<string, unknown>;
  agentNickname?: string | null;
  agentRole?: string | null;
  gitInfo?: unknown | null;
  name: string | null;
  turns?: unknown[];
}

export interface CodexThreadListResponse {
  data: CodexThreadSummary[];
  nextCursor?: string | null;
}

export interface CodexThreadReadResponse {
  thread: CodexThreadSummary;
}

export interface CodexThreadSessionResponse extends CodexThreadReadResponse {
  model: string;
  modelProvider: string;
  serviceTier: unknown | null;
  cwd: string;
  instructionSources: string[];
  approvalPolicy: string | Record<string, unknown>;
  approvalsReviewer: string | Record<string, unknown>;
  sandbox: unknown;
  permissionProfile: unknown | null;
  reasoningEffort: string | null;
}

export type CodexThreadArchiveResponse = Record<string, never>;

export interface CodexExecResponse {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export type CodexEmptyResponse = Record<string, never>;

export interface CodexMcpStatusResponse {
  data: unknown[];
  nextCursor: string | null;
}

export interface CodexMcpResourceReadResponse {
  contents: unknown[];
}

export interface CodexMcpToolCallResponse {
  content: unknown[];
  structuredContent?: unknown;
  isError?: boolean;
  _meta?: unknown;
}

export interface CodexConfigReadResponse {
  config: Record<string, unknown>;
  origins: Record<string, unknown>;
  layers: unknown[] | null;
}

export interface CodexConfigWriteResponse {
  status: string;
  version: string;
  filePath: string;
  overriddenMetadata: unknown | null;
}

export interface CodexFeedbackResponse {
  threadId: string;
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
  codex_response?: unknown;
  allow_all?: boolean;
}

export interface ToolProgressInfo {
  tool_call_id: string;
  tool_name: string;
  message: string;
  active_form?: string;
}

export interface ToolResultInfo {
  tool_call_id: string;
  tool_name: string;
  is_error: boolean;
  output: string;
}

export interface CodexAppServerNotificationInfo {
  session_id: string;
  method: string;
  params: unknown;
}

export interface CodexRecoverableErrorInfo {
  session_id: string;
  message: string;
  timestamp: number;
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

type SubtaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'stopped';

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

// ── Context Visualization ────────────────────────────────────────────

interface ContextCategory {
  name: string;
  tokens: number;
  color: string;
}

interface ContextData {
  categories: ContextCategory[];
  totalTokens: number;
  maxTokens: number;
  percentage: number;
  model: string;
}

// ── Task List V2 ─────────────────────────────────────────────────────

type TaskStatus = 'pending' | 'in_progress' | 'completed';

interface TaskItem {
  id: string;
  title: string;
  status: TaskStatus;
  owner?: string;
  blockedBy: string[];
}

// ── File Edit Diff ───────────────────────────────────────────────────

interface FileEdit {
  old_string: string;
  new_string: string;
  replace_all?: boolean;
}

interface DiffLine {
  type: 'add' | 'remove' | 'context';
  content: string;
  lineNumber?: number;
}

// ── Agent Progress ───────────────────────────────────────────────────

interface AgentProgressInfo {
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

interface CoordinatorTask {
  id: string;
  name?: string;
  status: TaskStatus;
  description: string;
  startTime: number;
  endTime?: number;
  tokenCount?: number;
}

// ── Teammate View ────────────────────────────────────────────────────

interface TeammateInfo {
  agentName: string;
  color?: string;
  prompt?: string;
}

// ── Session Preview ──────────────────────────────────────────────────

interface SessionInfo {
  id: string;
  title: string;
  messageCount: number;
  modified: string;
  gitBranch?: string;
  messages: unknown[];
}

// ── Quick Open ───────────────────────────────────────────────────────

interface QuickOpenResult {
  path: string;
  label: string;
}

// ── Compact Summary ──────────────────────────────────────────────────

interface SummarizeMetadata {
  messagesSummarized: number;
  direction: 'up_to' | 'from_here';
  userContext?: string;
}

// ── Memory Usage ─────────────────────────────────────────────────────

type MemoryStatus = 'normal' | 'high' | 'critical';

interface MemoryUsageData {
  heapUsed: number;
  status: MemoryStatus;
}

// ── Multi-Agent ──────────────────────────────────────────────────────

/** Agent 类型 */
export type AgentType = 'remote_claude' | 'remote_roo' | 'remote_codex';

/** Agent 类型信息 — 与 Rust AgentTypeInfoDto 对齐 (serde rename_all = camelCase) */
export interface AgentTypeInfo {
  agentType: string;
  displayName: string;
  available: boolean;
  installed: boolean;
}

/** Agent 状态变化事件 — 与 Rust AgentStatusChangedDto 对齐 */
export interface AgentStatusChangedInfo {
  sessionId: string;
  agentType: string;
  status: string;
}
