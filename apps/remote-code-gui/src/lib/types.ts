export interface ProviderInfo {
  name: string;
  model: string | null;
  protocol: string;
  base_url: string | null;
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
}

export interface PermissionDecisionInfo {
  request_id: string;
  allowed: boolean;
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

export type SubtaskStatus = 'running' | 'completed' | 'failed';

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
}
