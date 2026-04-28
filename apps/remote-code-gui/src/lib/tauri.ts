import { invoke } from '@tauri-apps/api/core';
import { listen, type EventCallback, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AgentStatusChangedInfo,
  AgentTypeInfo,
  AgentType,
  BatchProgressInfo,
  ConfigScope,
  ConversationEntry,
  ContextCompactedInfo,
  ContextOverflowInfo,
  ContextUsageInfo,
  DoctorReportInfo,
  FullSettings,
  InitResult,
  McpMutationResult,
  McpServerDraft,
  McpServerListInfo,
  PermissionDecisionInfo,
  PermissionRequestInfo,
  ProjectInfo,
  PromptDoneInfo,
  ProviderConfig,
  ProviderConfigList,
  ProviderInfo,
  RuntimeMcpInventoryInfo,
  RuntimeStatusInfo,
  SessionExportFormat,
  SessionExportResult,
  SessionSummary,
  SessionSubtask,
  SubtaskCompletedInfo,
  SubtaskProgressInfo,
  SubtaskStartedInfo,
  TaskSnapshotInfo,
  StreamingDeltaInfo,
  ToolProgressInfo,
  ToolResultInfo,
  UpdateProviderRequest,
} from './types';

export function initApp(): Promise<InitResult> {
  return invoke<InitResult>('init_app');
}

export function listSessions(): Promise<SessionSummary[]> {
  return invoke<SessionSummary[]>('list_sessions');
}

export function listArchivedSessions(): Promise<SessionSummary[]> {
  return invoke<SessionSummary[]>('list_archived_sessions');
}

export function getSessionConversation(sessionId: string): Promise<ConversationEntry[]> {
  return invoke<ConversationEntry[]>('get_session_conversation', { sessionId });
}

export function getSessionTasks(sessionId: string): Promise<SessionSubtask[]> {
  return invoke<SessionSubtask[]>('get_session_tasks', { sessionId });
}

export function createSession(title?: string, projectPath?: string, agentType?: AgentType): Promise<string> {
  return invoke<string>('create_session', {
    title: title ?? null,
    projectPath: projectPath ?? null,
    agentType: agentType ?? null,
  });
}

export function getProviderInfo(): Promise<ProviderInfo | null> {
  return invoke<ProviderInfo | null>('get_provider_info');
}

export function getRuntimeStatus(): Promise<RuntimeStatusInfo> {
  return invoke<RuntimeStatusInfo>('get_runtime_status');
}

export function runDoctorReport(
  probeNetwork = false,
  probeProvider = false,
  probeMcp = false,
  includeEnvProviders = false,
): Promise<DoctorReportInfo> {
  return invoke<DoctorReportInfo>('run_doctor_report', {
    probeNetwork,
    probeProvider,
    probeMcp,
    includeEnvProviders,
  });
}

export function exportSessionBundle(
  sessionId: string,
  format: SessionExportFormat,
): Promise<SessionExportResult> {
  return invoke<SessionExportResult>('export_session_bundle', { sessionId, format });
}

export function listMcpServers(
  scope: ConfigScope,
  projectPath: string | null,
  connect = false,
  includeDisabled = true,
): Promise<McpServerListInfo> {
  return invoke<McpServerListInfo>('list_mcp_servers', {
    scope,
    projectPath: projectPath ?? null,
    connect,
    includeDisabled,
  });
}

export function listRuntimeMcpInventory(
  projectPath: string | null,
  connect = false,
  includeDisabled = true,
): Promise<RuntimeMcpInventoryInfo> {
  return invoke<RuntimeMcpInventoryInfo>('list_runtime_mcp_inventory', {
    projectPath: projectPath ?? null,
    connect,
    includeDisabled,
  });
}

export function saveMcpServer(request: McpServerDraft): Promise<McpMutationResult> {
  return invoke<McpMutationResult>('save_mcp_server', { request });
}

export function toggleMcpServer(
  scope: ConfigScope,
  projectPath: string | null,
  name: string,
  enabled: boolean,
  ifExists = true,
): Promise<McpMutationResult> {
  return invoke<McpMutationResult>('toggle_mcp_server', {
    scope,
    projectPath: projectPath ?? null,
    name,
    enabled,
    ifExists,
  });
}

export function removeMcpServer(
  scope: ConfigScope,
  projectPath: string | null,
  name: string,
  ifExists = true,
): Promise<McpMutationResult> {
  return invoke<McpMutationResult>('remove_mcp_server', {
    scope,
    projectPath: projectPath ?? null,
    name,
    ifExists,
  });
}

export function resetMcpServers(
  scope: ConfigScope,
  projectPath: string | null,
  ifExists = true,
): Promise<McpMutationResult> {
  return invoke<McpMutationResult>('reset_mcp_servers', {
    scope,
    projectPath: projectPath ?? null,
    ifExists,
  });
}

export function sendPrompt(prompt: string, sessionId?: string): Promise<string> {
  return invoke<string>('send_prompt', {
    prompt,
    sessionId: sessionId ?? null,
  });
}

export function cancelPrompt(sessionId: string): Promise<boolean> {
  return invoke<boolean>('cancel_prompt', { sessionId });
}

export function getSettings(): Promise<FullSettings> {
  return invoke<FullSettings>('get_settings');
}

export function updateProvider(request: UpdateProviderRequest): Promise<void> {
  return invoke('update_provider', { request });
}

export function listProjects(): Promise<ProjectInfo[]> {
  return invoke<ProjectInfo[]>('list_projects');
}

export function addProject(path: string): Promise<ProjectInfo> {
  return invoke<ProjectInfo>('add_project', { path });
}

export function removeProject(path: string): Promise<void> {
  return invoke('remove_project', { path });
}

export function archiveSession(sessionId: string): Promise<void> {
  return invoke('archive_session', { sessionId });
}

export function restoreSession(sessionId: string): Promise<void> {
  return invoke('restore_session', { sessionId });
}

export function pickFolder(): Promise<string | null> {
  return invoke<string | null>('pick_folder');
}

export function listProviderConfigs(): Promise<ProviderConfigList> {
  return invoke<ProviderConfigList>('list_provider_configs');
}

export function saveProviderConfig(config: ProviderConfig, setActive: boolean): Promise<void> {
  return invoke<void>('save_provider_config', { config, setActive });
}

export function deleteProviderConfig(name: string): Promise<void> {
  return invoke<void>('delete_provider_config', { name });
}

export function setActiveProvider(name: string): Promise<void> {
  return invoke<void>('set_active_provider', { name });
}

export function switchProfile(
  providerName: string,
  profileName: string | null,
): Promise<void> {
  return invoke<void>('switch_profile', {
    providerName,
    profileName: profileName ?? null,
  });
}

export interface PermissionResolutionRequest {
  allowed: boolean;
  message?: string | null;
  updated_input?: unknown;
  permission_updates?: unknown[];
  feedback?: string | null;
  content_blocks?: unknown[];
}

export function resolvePermissionRequest(
  requestId: string,
  resolution: PermissionResolutionRequest,
): Promise<boolean> {
  return invoke<boolean>('resolve_permission_request', { requestId, ...resolution });
}

export function onPermissionRequest(
  callback: EventCallback<PermissionRequestInfo>,
): Promise<UnlistenFn> {
  return listen<PermissionRequestInfo>('gui://permission-request', callback);
}

export function onPermissionResolved(
  callback: EventCallback<PermissionDecisionInfo>,
): Promise<UnlistenFn> {
  return listen<PermissionDecisionInfo>('gui://permission-resolved', callback);
}

export function onToolStart(callback: EventCallback<ToolProgressInfo>): Promise<UnlistenFn> {
  return listen<ToolProgressInfo>('gui://tool-start', callback);
}

export function onToolProgress(callback: EventCallback<ToolProgressInfo>): Promise<UnlistenFn> {
  return listen<ToolProgressInfo>('gui://tool-progress', callback);
}

export function onToolResult(callback: EventCallback<ToolResultInfo>): Promise<UnlistenFn> {
  return listen<ToolResultInfo>('gui://tool-result', callback);
}

export function onStreamingDelta(
  callback: EventCallback<StreamingDeltaInfo>,
): Promise<UnlistenFn> {
  return listen<StreamingDeltaInfo>('gui://streaming-delta', callback);
}

export function onPromptDone(callback: EventCallback<PromptDoneInfo>): Promise<UnlistenFn> {
  return listen<PromptDoneInfo>('gui://prompt-done', callback);
}

export function onSubtaskStarted(
  callback: EventCallback<SubtaskStartedInfo>,
): Promise<UnlistenFn> {
  return listen<SubtaskStartedInfo>('gui://subtask-started', callback);
}

export function onSubtaskProgress(
  callback: EventCallback<SubtaskProgressInfo>,
): Promise<UnlistenFn> {
  return listen<SubtaskProgressInfo>('gui://subtask-progress', callback);
}

export function onSubtaskCompleted(
  callback: EventCallback<SubtaskCompletedInfo>,
): Promise<UnlistenFn> {
  return listen<SubtaskCompletedInfo>('gui://subtask-completed', callback);
}

export function onBatchProgress(
  callback: EventCallback<BatchProgressInfo>,
): Promise<UnlistenFn> {
  return listen<BatchProgressInfo>('gui://batch-progress', callback);
}

export function onTaskSnapshot(
  callback: EventCallback<TaskSnapshotInfo>,
): Promise<UnlistenFn> {
  return listen<TaskSnapshotInfo>('gui://task-snapshot', callback);
}

export function onContextUsage(
  callback: EventCallback<ContextUsageInfo>,
): Promise<UnlistenFn> {
  return listen<ContextUsageInfo>('gui://context-usage', callback);
}

export function onContextOverflow(
  callback: EventCallback<ContextOverflowInfo>,
): Promise<UnlistenFn> {
  return listen<ContextOverflowInfo>('gui://context-overflow', callback);
}

export function onContextCompacted(
  callback: EventCallback<ContextCompactedInfo>,
): Promise<UnlistenFn> {
  return listen<ContextCompactedInfo>('gui://context-compacted', callback);
}

export function onRuntimeStatus(
  callback: EventCallback<RuntimeStatusInfo>,
): Promise<UnlistenFn> {
  return listen<RuntimeStatusInfo>('gui://runtime-status', callback);
}

// ── Multi-Agent APIs ────────────────────────────────────────────────

export function listAvailableAgents(): Promise<AgentTypeInfo[]> {
  return invoke<AgentTypeInfo[]>('list_available_agents');
}

export function installAgent(agentType: AgentType): Promise<void> {
  return invoke<void>('install_agent', { agentType });
}

export function uninstallAgent(agentType: AgentType): Promise<void> {
  return invoke<void>('uninstall_agent', { agentType });
}

export function onAgentStatusChanged(
  callback: EventCallback<AgentStatusChangedInfo>,
): Promise<UnlistenFn> {
  return listen<AgentStatusChangedInfo>('gui://agent-status-changed', callback);
}

// ── Voice / STT APIs ────────────────────────────────────────────────

/**
 * Transcribe audio data via the Rust STT backend (OpenAI Whisper API).
 *
 * @param audioData - Raw audio bytes (0–255).  Accepts `Uint8Array` for type
 *   safety; the data is spread into a plain array before crossing the
 *   Tauri bridge because the command expects `Vec<u8>`.
 * @param audioFormat - Audio container format (e.g. `"webm"`, `"mp4"`, `"ogg"`, `"wav"`).
 *   Derived from the MIME type when omitted.
 */
export function transcribeAudio(
  audioData: Uint8Array,
  audioFormat?: string,
): Promise<string> {
  return invoke<string>('transcribe_audio', {
    audioData: Array.from(audioData),
    audioFormat: audioFormat || 'webm',
  });
}
