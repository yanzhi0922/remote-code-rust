import type {
  BatchProgressInfo,
  ContextCompactedInfo,
  ContextOverflowInfo,
  ContextUsageInfo,
  ConversationEntry,
  PermissionRequestInfo,
  SessionSubtask,
  SessionSummary,
  ToolProgressInfo,
  ToolResultInfo,
} from '../../lib/types';
import type { ToolCallInfo } from '../../lib/types';
import type {
  ApprovalItemVm,
  BatchTimelineItemVm,
  ContextTimelineItemVm,
  MessageTimelineItemVm,
  SessionBundleVm,
  SessionConnectionVm,
  SessionSummaryVm,
  SubtaskTimelineItemVm,
  TaskNodeVm,
  TimelineItemVm,
  ToolCallVm,
  ToolTimelineItemVm,
} from '../contracts';
import { summarizeToolCallInput } from '../contracts';
import { buildSessionBundleVm } from '../view-model';

export function normalizeLocalSessionSummary(session: SessionSummary): SessionSummaryVm {
  return {
    id: session.id,
    source: 'local',
    title: session.title,
    workspaceLabel: session.cwd,
    providerName: session.provider_name,
    model: session.model,
    runnerId: null,
    runnerAvailable: null,
    runnerState: null,
    runnerLastSeenAt: null,
    state: session.archived ? 'archived' : 'active',
    metadata: {},
    createdAt: session.created_at,
    updatedAt: session.updated_at,
  };
}

export function normalizeLocalConversationEntry(
  entry: ConversationEntry,
  index: number,
  sessionId: string,
): MessageTimelineItemVm {
  return {
    id: `local-message-${index}`,
    source: 'local',
    sessionId,
    order: index,
    timestamp: null,
    kind: 'message',
    role: entry.role,
    text: entry.text,
    status: 'committed',
    thinkingBlocks: extractThinkingBlocks(entry),
    toolCalls: entry.tool_calls.map(normalizeLocalToolCall),
    isError: entry.is_error,
  };
}

export function normalizeLocalToolProgress(
  progress: ToolProgressInfo,
  index: number,
  sessionId: string,
): ToolTimelineItemVm {
  return {
    id: `local-tool-progress-${index}-${progress.tool_call_id}`,
    source: 'local',
    sessionId,
    order: 10_000 + index,
    timestamp: null,
    kind: 'tool',
    stage: 'progress',
    toolCallId: progress.tool_call_id,
    toolName: progress.tool_name,
    summary: progress.active_form ?? progress.message,
    elapsedTimeSeconds: null,
    isError: false,
  };
}

export function normalizeLocalToolResult(
  result: ToolResultInfo,
  index: number,
  sessionId: string,
): ToolTimelineItemVm {
  return {
    id: `local-tool-result-${index}-${result.tool_call_id}`,
    source: 'local',
    sessionId,
    order: 20_000 + index,
    timestamp: null,
    kind: 'tool',
    stage: 'finished',
    toolCallId: result.tool_call_id,
    toolName: result.tool_name,
    summary: result.output,
    elapsedTimeSeconds: null,
    isError: result.is_error,
  };
}

export function normalizeLocalPermissionRequest(
  permission: PermissionRequestInfo,
): ApprovalItemVm {
  return {
    id: permission.request_id,
    source: 'local',
    sessionId: 'active',
    runnerId: null,
    state: 'pending',
    title: permission.title,
    description: permission.description,
    metadata:
      permission.permission_suggestions.length > 0
        ? { permission_suggestions_count: String(permission.permission_suggestions.length) }
        : {},
    blockedPath: permission.blocked_path,
    createdAt: null,
    updatedAt: null,
    respondedAt: null,
    responder: null,
    note: null,
  };
}

export function normalizeLocalTask(task: SessionSubtask): TaskNodeVm {
  return {
    id: task.task_id,
    sessionId: task.session_id,
    parentTaskId: task.parent_task_id,
    description: task.description,
    depth: task.depth,
    status: task.status,
    summary: task.summary,
    outputPreview: task.output_preview,
    turnsUsed: task.turns_used,
    kind: task.kind ?? 'unknown',
    outputPath: task.output_path ?? null,
    createdAt: task.created_at ?? null,
    updatedAt: task.updated_at ?? null,
  };
}

export function normalizeLocalTasks(tasks: SessionSubtask[]): TaskNodeVm[] {
  return tasks.map(normalizeLocalTask);
}

export function normalizeLocalBatchProgress(
  progress: BatchProgressInfo,
  order: number,
): BatchTimelineItemVm {
  return {
    id: `local-batch-${progress.session_id}-${order}`,
    source: 'local',
    sessionId: progress.session_id,
    order,
    timestamp: null,
    kind: 'batch',
    total: progress.total,
    completed: progress.completed,
    running: progress.running,
  };
}

export function normalizeLocalContextUsage(
  usage: ContextUsageInfo,
  order: number,
): ContextTimelineItemVm {
  return {
    id: `local-context-usage-${usage.session_id}-${order}`,
    source: 'local',
    sessionId: usage.session_id,
    order,
    timestamp: null,
    kind: 'context',
    event: 'usage',
    estimatedTokens: usage.estimated_tokens,
    maxInputTokens: usage.max_input_tokens,
    thresholdTokens: usage.threshold_tokens,
    ratio: usage.ratio,
    entriesRemoved: null,
    usageRatio: null,
  };
}

export function normalizeLocalContextOverflow(
  overflow: ContextOverflowInfo,
  order: number,
): ContextTimelineItemVm {
  return {
    id: `local-context-overflow-${overflow.session_id}-${order}`,
    source: 'local',
    sessionId: overflow.session_id,
    order,
    timestamp: null,
    kind: 'context',
    event: 'overflow',
    estimatedTokens: overflow.estimated_tokens,
    maxInputTokens: overflow.max_input_tokens,
    thresholdTokens: overflow.threshold_tokens,
    ratio: overflow.ratio,
    entriesRemoved: null,
    usageRatio: null,
  };
}

export function normalizeLocalContextCompaction(
  compacted: ContextCompactedInfo,
  order: number,
): ContextTimelineItemVm {
  return {
    id: `local-context-compacted-${compacted.session_id}-${order}`,
    source: 'local',
    sessionId: compacted.session_id,
    order,
    timestamp: null,
    kind: 'context',
    event: 'compacted',
    estimatedTokens: null,
    maxInputTokens: null,
    thresholdTokens: null,
    ratio: null,
    entriesRemoved: compacted.entries_removed,
    usageRatio: compacted.usage_ratio,
  };
}

export function normalizeLocalSubtaskTimeline(
  task: SessionSubtask,
  order: number,
): SubtaskTimelineItemVm {
  const stage =
    task.status === 'completed' || task.status === 'failed' || task.status === 'stopped'
      ? 'completed'
      : 'started';
  return {
    id: `local-subtask-${task.task_id}-${order}`,
    source: 'local',
    sessionId: task.session_id,
    order,
    timestamp: task.updated_at ?? task.created_at ?? null,
    kind: 'subtask',
    taskId: task.task_id,
    parentTaskId: task.parent_task_id,
    description: task.description,
    depth: task.depth,
    stage,
    status: task.status,
    summary: task.summary,
    turnsUsed: task.turns_used,
  };
}

export function buildLocalSessionBundle(input: {
  session: SessionSummary | null;
  conversation: ConversationEntry[];
  liveToolProgress?: ToolProgressInfo[];
  liveToolResults?: ToolResultInfo[];
  pendingPermission?: PermissionRequestInfo | null;
  tasks?: SessionSubtask[];
  batchProgress?: BatchProgressInfo | null;
  contextUsage?: ContextUsageInfo | null;
  contextOverflow?: ContextOverflowInfo | null;
  contextCompaction?: ContextCompactedInfo | null;
  connection?: SessionConnectionVm;
  composerValue?: string;
  composerDisabled?: boolean;
  composerBusy?: boolean;
  composerPlaceholder?: string | null;
}): SessionBundleVm {
  const sessionId = input.session?.id ?? 'active';
  const timeline: TimelineItemVm[] = [
    ...input.conversation.map((entry, index) =>
      normalizeLocalConversationEntry(entry, index, sessionId),
    ),
    ...(input.liveToolProgress ?? []).map((item, index) =>
      normalizeLocalToolProgress(item, index, sessionId),
    ),
    ...(input.liveToolResults ?? []).map((item, index) =>
      normalizeLocalToolResult(item, index, sessionId),
    ),
    ...(input.tasks ?? []).map((task, index) =>
      normalizeLocalSubtaskTimeline(task, 30_000 + index),
    ),
  ];

  if (input.batchProgress) {
    timeline.push(normalizeLocalBatchProgress(input.batchProgress, 40_000));
  }
  if (input.contextUsage) {
    timeline.push(normalizeLocalContextUsage(input.contextUsage, 50_000));
  }
  if (input.contextOverflow) {
    timeline.push(normalizeLocalContextOverflow(input.contextOverflow, 50_100));
  }
  if (input.contextCompaction) {
    timeline.push(normalizeLocalContextCompaction(input.contextCompaction, 50_200));
  }

  return buildSessionBundleVm({
    session: input.session ? normalizeLocalSessionSummary(input.session) : null,
    timeline,
    approvals: input.pendingPermission ? [normalizeLocalPermissionRequest(input.pendingPermission)] : [],
    artifacts: [],
    taskTree: normalizeLocalTasks(input.tasks ?? []),
    connection:
      input.connection ?? {
        state: 'local',
        canSendPrompt: true,
        canInterrupt: true,
        notice: null,
      },
    composer: {
      value: input.composerValue ?? '',
      disabled: input.composerDisabled ?? false,
      busy: input.composerBusy ?? false,
      placeholder: input.composerPlaceholder ?? null,
    },
    latestCursor: null,
  });
}

function extractThinkingBlocks(entry: ConversationEntry): string[] {
  return entry.content_blocks
    .filter((block): block is Record<string, unknown> => Boolean(block) && typeof block === 'object')
    .filter((block) => block.type === 'thinking' && typeof block.thinking === 'string')
    .map((block) => block.thinking as string);
}

function normalizeLocalToolCall(toolCall: ToolCallInfo): ToolCallVm {
  return {
    id: toolCall.id,
    name: toolCall.name,
    input: toolCall.input,
    summary: summarizeToolCallInput(toolCall),
  };
}
