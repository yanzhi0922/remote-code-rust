import type { ConversationRole, ToolCallInfo } from '../lib/types';
import type {
  RemoteApprovalState,
  RemoteDaemonPresenceState,
  RemoteMessageRole,
  RemoteRunnerState,
  RemoteSessionState,
} from '../remote/types';

export type SessionVmSource = 'local' | 'remote';

export type SessionConnectionStateVm =
  | 'local'
  | 'idle'
  | 'connecting'
  | 'open'
  | 'reconnecting'
  | 'error';

export interface SessionSummaryVm {
  id: string;
  source: SessionVmSource;
  title: string;
  workspaceLabel: string;
  providerName: string | null;
  model: string | null;
  runnerId: string | null;
  runnerAvailable: boolean | null;
  runnerState: RemoteRunnerState | null;
  runnerLastSeenAt: string | null;
  state: string | null;
  metadata: Record<string, string>;
  createdAt: string;
  updatedAt: string;
}

export interface SessionConnectionVm {
  state: SessionConnectionStateVm;
  canSendPrompt: boolean;
  canInterrupt: boolean;
  notice: string | null;
}

export interface ComposerVm {
  value: string;
  disabled: boolean;
  busy: boolean;
  placeholder: string | null;
}

export interface ToolCallVm {
  id: string;
  name: string;
  input: unknown;
  summary: string;
}

export interface ApprovalItemVm {
  id: string;
  source: SessionVmSource;
  sessionId: string;
  runnerId: string | null;
  state: RemoteApprovalState | 'pending';
  title: string;
  description: string;
  metadata: Record<string, string>;
  blockedPath: string | null;
  createdAt: string | null;
  updatedAt: string | null;
  respondedAt: string | null;
  responder: string | null;
  note: string | null;
}

export interface ArtifactItemVm {
  id: string;
  source: SessionVmSource;
  sessionId: string;
  runnerId: string | null;
  name: string;
  fileName: string;
  mediaType: string;
  sizeBytes: number;
  metadata: Record<string, string>;
  createdAt: string | null;
}

export interface TaskNodeVm {
  id: string;
  sessionId: string;
  parentTaskId: string | null;
  description: string;
  depth: number;
  status: string;
  summary: string;
  outputPreview: string | null;
  turnsUsed: number | null;
  kind: 'background' | 'delegation' | 'batch' | 'unknown';
  outputPath: string | null;
  createdAt: string | null;
  updatedAt: string | null;
}

interface TimelineItemBase {
  id: string;
  source: SessionVmSource;
  sessionId: string | null;
  order: number;
  timestamp: string | null;
}

export interface MessageTimelineItemVm extends TimelineItemBase {
  kind: 'message';
  role: ConversationRole | RemoteMessageRole;
  text: string;
  status: 'streaming' | 'committed';
  thinkingBlocks: string[];
  toolCalls: ToolCallVm[];
  isError: boolean;
}

export interface ToolTimelineItemVm extends TimelineItemBase {
  kind: 'tool';
  stage: 'started' | 'progress' | 'finished';
  toolCallId: string | null;
  toolName: string | null;
  summary: string;
  elapsedTimeSeconds: number | null;
  isError: boolean;
}

export interface ApprovalTimelineItemVm extends TimelineItemBase {
  kind: 'approval';
  stage: 'requested' | 'resolved';
  approvalId: string;
  title: string | null;
  state: RemoteApprovalState | 'pending';
  responder: string | null;
}

export interface ArtifactTimelineItemVm extends TimelineItemBase {
  kind: 'artifact';
  stage: 'created' | 'manifest';
  artifactId: string | null;
  artifactIds: string[];
  name: string | null;
  fileName: string | null;
  mediaType: string | null;
  sizeBytes: number | null;
}

export interface SessionEventTimelineItemVm extends TimelineItemBase {
  kind: 'session';
  event: 'created' | 'state_changed';
  state: RemoteSessionState | string | null;
  previousState: RemoteSessionState | string | null;
  workspaceId: string | null;
  ownerRunnerId: string | null;
}

export interface RunnerTimelineItemVm extends TimelineItemBase {
  kind: 'runner';
  event: 'registered' | 'heartbeat';
  runnerId: string | null;
  state: RemoteRunnerState | null;
  workspaceIds: string[];
  leaseTtlSecs: number | null;
  activeSessions: number | null;
  queuedSessions: number | null;
}

export interface RuntimeErrorTimelineItemVm extends TimelineItemBase {
  kind: 'runtime_error';
  message: string;
}

export interface DaemonTimelineItemVm extends TimelineItemBase {
  kind: 'daemon';
  state: RemoteDaemonPresenceState;
}

export interface SubtaskTimelineItemVm extends TimelineItemBase {
  kind: 'subtask';
  taskId: string;
  parentTaskId: string | null;
  description: string;
  depth: number;
  stage: 'started' | 'progress' | 'completed';
  status: string;
  summary: string;
  turnsUsed: number | null;
}

export interface BatchTimelineItemVm extends TimelineItemBase {
  kind: 'batch';
  total: number;
  completed: number;
  running: number;
}

export interface ContextTimelineItemVm extends TimelineItemBase {
  kind: 'context';
  event: 'usage' | 'overflow' | 'compacted';
  estimatedTokens: number | null;
  maxInputTokens: number | null;
  thresholdTokens: number | null;
  ratio: number | null;
  entriesRemoved: number | null;
  usageRatio: number | null;
}

export type TimelineItemVm =
  | MessageTimelineItemVm
  | ToolTimelineItemVm
  | ApprovalTimelineItemVm
  | ArtifactTimelineItemVm
  | SessionEventTimelineItemVm
  | RunnerTimelineItemVm
  | RuntimeErrorTimelineItemVm
  | DaemonTimelineItemVm
  | SubtaskTimelineItemVm
  | BatchTimelineItemVm
  | ContextTimelineItemVm;

export interface SessionBundleVm {
  session: SessionSummaryVm | null;
  timeline: TimelineItemVm[];
  approvals: ApprovalItemVm[];
  artifacts: ArtifactItemVm[];
  taskTree: TaskNodeVm[];
  connection: SessionConnectionVm;
  composer: ComposerVm;
  latestCursor: number | null;
}

export function summarizeToolCallInput(toolCall: ToolCallInfo): string {
  try {
    const normalized =
      typeof toolCall.input === 'string' ? JSON.parse(toolCall.input) : toolCall.input;
    if (normalized && typeof normalized === 'object') {
      const objectValue = normalized as Record<string, unknown>;
      const preview =
        objectValue.path ??
        objectValue.file_path ??
        objectValue.command ??
        objectValue.query ??
        objectValue.prompt ??
        Object.values(objectValue)[0];
      if (typeof preview === 'string' && preview.trim()) {
        return preview.trim();
      }
    }
  } catch {
    // Fall back to the tool name when input parsing fails.
  }
  return toolCall.name;
}
