import type {
  ArtifactTimelineItemVm,
  ApprovalTimelineItemVm,
  DaemonTimelineItemVm,
  MessageTimelineItemVm,
  RunnerTimelineItemVm,
  RuntimeErrorTimelineItemVm,
  SessionBundleVm,
  SessionConnectionVm,
  SessionEventTimelineItemVm,
  SessionSummaryVm,
  TimelineItemVm,
  ToolTimelineItemVm,
} from '../contracts';
import { buildSessionBundleVm } from '../view-model';
import type {
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteSessionRecord,
  RemoteTimelineEvent,
  RemoteTimelineEventDetail,
} from '../../remote/types';

export function normalizeRemoteSessionSummary(session: RemoteSessionRecord): SessionSummaryVm {
  return {
    id: session.session_id,
    source: 'remote',
    title: resolveRemoteSessionTitle(session),
    workspaceLabel: session.workspace_id,
    providerName: session.metadata.provider_name ?? null,
    model: session.metadata.model ?? null,
    runnerId: session.owner_runner_id,
    runnerAvailable: session.owner_runner_available ?? null,
    runnerState: session.owner_runner_state ?? null,
    runnerLastSeenAt: session.owner_runner_last_seen_at ?? null,
    state: session.state,
    metadata: session.metadata,
    createdAt: session.created_at,
    updatedAt: session.updated_at,
  };
}

export function normalizeRemoteApproval(record: RemoteApprovalRecord) {
  return {
    id: record.approval_id,
    source: 'remote' as const,
    sessionId: record.session_id,
    runnerId: record.runner_id,
    state: record.state,
    title: record.title,
    description: record.description,
    metadata: record.metadata,
    blockedPath: record.metadata.blocked_path ?? null,
    createdAt: record.created_at,
    updatedAt: record.updated_at,
    respondedAt: record.responded_at,
    responder: record.responder,
    note: record.note,
  };
}

export function normalizeRemoteArtifact(record: RemoteArtifactRecord) {
  return {
    id: record.artifact_id,
    source: 'remote' as const,
    sessionId: record.session_id,
    runnerId: record.runner_id,
    name: record.name,
    fileName: record.file_name,
    mediaType: record.media_type,
    sizeBytes: record.size_bytes,
    metadata: record.metadata,
    createdAt: record.created_at,
  };
}

export function hydrateRemoteTimeline(events: RemoteTimelineEvent[]): RemoteTimelineEvent[] {
  return [...events]
    .sort((left, right) => left.sequence - right.sequence)
    .reduce<RemoteTimelineEvent[]>((current, event) => appendRemoteTimelineEvent(current, event), []);
}

export function appendRemoteTimelineEvent(
  current: RemoteTimelineEvent[],
  nextEvent: RemoteTimelineEvent,
): RemoteTimelineEvent[] {
  if (current.some((event) => event.sequence === nextEvent.sequence)) {
    return current;
  }

  if (nextEvent.detail.kind === 'message_committed') {
    const committedDetail = nextEvent.detail;
    const filtered = current.filter((event) => {
      if (event.detail.kind !== 'message_delta') {
        return true;
      }
      return !sameMessageStream(event.detail, committedDetail);
    });
    return [...filtered, nextEvent].sort((left, right) => left.sequence - right.sequence);
  }

  if (nextEvent.detail.kind === 'message_delta') {
    const deltaDetail = nextEvent.detail;
    const last = current[current.length - 1];
    if (last?.detail.kind === 'message_delta' && sameMessageStream(last.detail, deltaDetail)) {
      const merged: RemoteTimelineEvent = {
        ...nextEvent,
        detail: {
          ...deltaDetail,
          delta: `${last.detail.delta}${deltaDetail.delta}`,
        },
      };
      return [...current.slice(0, -1), merged];
    }
  }

  if (nextEvent.detail.kind === 'tool_progress') {
    const progressDetail = nextEvent.detail;
    const last = current[current.length - 1];
    if (last?.detail.kind === 'tool_progress' && sameToolProgress(last.detail, progressDetail)) {
      const mergedDelta = `${last.detail.delta ?? ''}${progressDetail.delta ?? ''}`.trim();
      const merged: RemoteTimelineEvent = {
        ...nextEvent,
        detail: {
          ...progressDetail,
          delta: mergedDelta || undefined,
          elapsed_time_seconds:
            progressDetail.elapsed_time_seconds ?? last.detail.elapsed_time_seconds,
        },
      };
      return [...current.slice(0, -1), merged];
    }
  }

  if (
    nextEvent.detail.kind === 'session_state_changed' &&
    nextEvent.detail.previous_state === nextEvent.detail.state
  ) {
    return current;
  }

  if (nextEvent.detail.kind === 'daemon_presence_changed') {
    const last = current[current.length - 1];
    if (
      last?.detail.kind === 'daemon_presence_changed' &&
      last.detail.state === nextEvent.detail.state
    ) {
      return current;
    }
  }

  return [...current, nextEvent].sort((left, right) => left.sequence - right.sequence);
}

export function normalizeRemoteTimelineEvent(event: RemoteTimelineEvent): TimelineItemVm {
  const base = {
    id: `remote-${event.sequence}`,
    source: 'remote' as const,
    sessionId: event.session_id,
    order: event.sequence,
    timestamp: event.recorded_at,
  };

  switch (event.detail.kind) {
    case 'message_delta':
    case 'message_committed':
      return {
        ...base,
        kind: 'message',
        role: event.detail.role,
        text: event.detail.kind === 'message_committed' ? event.detail.text : event.detail.delta,
        status: event.detail.kind === 'message_committed' ? 'committed' : 'streaming',
        thinkingBlocks: [],
        toolCalls: [],
        isError: false,
      } satisfies MessageTimelineItemVm;
    case 'tool_started':
      return {
        ...base,
        kind: 'tool',
        stage: 'started',
        toolCallId: event.detail.tool_call_id,
        toolName: event.detail.tool_name,
        summary: event.detail.tool_name,
        elapsedTimeSeconds: null,
        isError: false,
      } satisfies ToolTimelineItemVm;
    case 'tool_progress':
      return {
        ...base,
        kind: 'tool',
        stage: 'progress',
        toolCallId: event.detail.tool_call_id ?? null,
        toolName: event.detail.tool_name ?? null,
        summary:
          event.detail.delta?.trim() ||
          event.detail.tool_name ||
          event.detail.tool_call_id ||
          'tool',
        elapsedTimeSeconds: event.detail.elapsed_time_seconds ?? null,
        isError: false,
      } satisfies ToolTimelineItemVm;
    case 'tool_finished':
      return {
        ...base,
        kind: 'tool',
        stage: 'finished',
        toolCallId: event.detail.tool_call_id,
        toolName: event.detail.tool_name,
        summary: event.detail.summary?.trim() || event.detail.tool_name,
        elapsedTimeSeconds: null,
        isError: event.detail.is_error,
      } satisfies ToolTimelineItemVm;
    case 'approval_requested':
    case 'approval_resolved':
      return {
        ...base,
        kind: 'approval',
        stage: event.detail.kind === 'approval_requested' ? 'requested' : 'resolved',
        approvalId: event.detail.approval_id,
        title: 'title' in event.detail ? event.detail.title : null,
        state: event.detail.state,
        responder: 'responder' in event.detail ? event.detail.responder : null,
      } satisfies ApprovalTimelineItemVm;
    case 'artifact_created':
      return {
        ...base,
        kind: 'artifact',
        stage: 'created',
        artifactId: event.detail.artifact_id,
        artifactIds: [event.detail.artifact_id],
        name: event.detail.name,
        fileName: event.detail.file_name,
        mediaType: event.detail.media_type,
        sizeBytes: event.detail.size_bytes,
      } satisfies ArtifactTimelineItemVm;
    case 'artifact_manifest':
      return {
        ...base,
        kind: 'artifact',
        stage: 'manifest',
        artifactId: null,
        artifactIds: event.detail.artifact_ids,
        name: null,
        fileName: null,
        mediaType: null,
        sizeBytes: null,
      } satisfies ArtifactTimelineItemVm;
    case 'runtime_error':
      return {
        ...base,
        kind: 'runtime_error',
        message: event.detail.message,
      } satisfies RuntimeErrorTimelineItemVm;
    case 'daemon_presence_changed':
      return {
        ...base,
        kind: 'daemon',
        state: event.detail.state,
      } satisfies DaemonTimelineItemVm;
    case 'session_created':
      return {
        ...base,
        kind: 'session',
        event: 'created',
        state: event.detail.state,
        previousState: null,
        workspaceId: event.detail.workspace_id,
        ownerRunnerId: event.detail.owner_runner_id,
      } satisfies SessionEventTimelineItemVm;
    case 'session_state_changed':
      return {
        ...base,
        kind: 'session',
        event: 'state_changed',
        state: event.detail.state,
        previousState: event.detail.previous_state,
        workspaceId: null,
        ownerRunnerId: null,
      } satisfies SessionEventTimelineItemVm;
    case 'runner_registered':
      return {
        ...base,
        kind: 'runner',
        event: 'registered',
        runnerId: event.runner_id,
        state: event.detail.state,
        workspaceIds: event.detail.workspace_ids,
        leaseTtlSecs: event.detail.lease_ttl_secs,
        activeSessions: null,
        queuedSessions: null,
      } satisfies RunnerTimelineItemVm;
    case 'runner_heartbeat':
      return {
        ...base,
        kind: 'runner',
        event: 'heartbeat',
        runnerId: event.runner_id,
        state: event.detail.state,
        workspaceIds: [],
        leaseTtlSecs: null,
        activeSessions: event.detail.active_sessions,
        queuedSessions: event.detail.queued_sessions,
      } satisfies RunnerTimelineItemVm;
  }
}

export function normalizeRemoteTimeline(events: RemoteTimelineEvent[]): TimelineItemVm[] {
  return hydrateRemoteTimeline(events).map(normalizeRemoteTimelineEvent);
}

export function buildRemoteSessionBundle(input: {
  session: RemoteSessionRecord | null;
  events: RemoteTimelineEvent[];
  approvals: RemoteApprovalRecord[];
  artifacts: RemoteArtifactRecord[];
  connection: SessionConnectionVm;
  composerValue?: string;
  composerDisabled?: boolean;
  composerBusy?: boolean;
  composerPlaceholder?: string | null;
  latestCursor?: number | null;
}): SessionBundleVm {
  return buildSessionBundleVm({
    session: input.session ? normalizeRemoteSessionSummary(input.session) : null,
    timeline: normalizeRemoteTimeline(input.events),
    approvals: input.approvals.map(normalizeRemoteApproval),
    artifacts: input.artifacts.map(normalizeRemoteArtifact),
    connection: input.connection,
    composer: {
      value: input.composerValue ?? '',
      disabled: input.composerDisabled ?? false,
      busy: input.composerBusy ?? false,
      placeholder: input.composerPlaceholder ?? null,
    },
    latestCursor: input.latestCursor ?? null,
  });
}

export function resolveRemoteSessionTitle(session: RemoteSessionRecord): string {
  const title = session.metadata.title?.trim();
  return title || session.workspace_id;
}

function sameMessageStream(
  left: Extract<RemoteTimelineEventDetail, { kind: 'message_delta' | 'message_committed' }>,
  right: Extract<RemoteTimelineEventDetail, { kind: 'message_delta' | 'message_committed' }>,
): boolean {
  if (left.message_id && right.message_id) {
    return left.message_id === right.message_id;
  }
  return left.role === right.role;
}

function sameToolProgress(
  left: Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }>,
  right: Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }>,
): boolean {
  const leftKey = left.tool_call_id ?? left.tool_name ?? '';
  const rightKey = right.tool_call_id ?? right.tool_name ?? '';
  return leftKey.length > 0 && leftKey === rightKey;
}
