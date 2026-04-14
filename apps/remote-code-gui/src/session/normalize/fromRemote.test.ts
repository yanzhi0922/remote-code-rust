import { describe, expect, it } from 'vitest';
import {
  appendRemoteTimelineEvent,
  buildRemoteSessionBundle,
  hydrateRemoteTimeline,
  normalizeRemoteSessionSummary,
} from './fromRemote';
import type {
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteSessionRecord,
  RemoteTimelineEvent,
} from '../../remote/types';

function baseSession(): RemoteSessionRecord {
  return {
    session_id: 'session-1',
    workspace_id: 'workspace-main',
    owner_runner_id: 'runner-1',
    owner_runner_available: true,
    owner_runner_state: 'busy',
    owner_runner_last_seen_at: '2026-04-14T10:00:00Z',
    state: 'running',
    metadata: { title: 'Alpha Session', provider_name: 'glm', model: 'glm-5.1' },
    created_at: '2026-04-14T09:00:00Z',
    updated_at: '2026-04-14T10:00:00Z',
  };
}

function baseEvents(): RemoteTimelineEvent[] {
  return [
    {
      sequence: 1,
      recorded_at: '2026-04-14T09:00:01Z',
      runner_id: 'runner-1',
      session_id: 'session-1',
      detail: {
        kind: 'message_delta',
        role: 'assistant',
        delta: 'Hel',
        message_id: 'msg-1',
      },
    },
    {
      sequence: 2,
      recorded_at: '2026-04-14T09:00:02Z',
      runner_id: 'runner-1',
      session_id: 'session-1',
      detail: {
        kind: 'message_delta',
        role: 'assistant',
        delta: 'lo',
        message_id: 'msg-1',
      },
    },
    {
      sequence: 3,
      recorded_at: '2026-04-14T09:00:03Z',
      runner_id: 'runner-1',
      session_id: 'session-1',
      detail: {
        kind: 'tool_progress',
        tool_call_id: 'tool-1',
        tool_name: 'bash_command',
        delta: 'git ',
        elapsed_time_seconds: 1,
      },
    },
    {
      sequence: 4,
      recorded_at: '2026-04-14T09:00:04Z',
      runner_id: 'runner-1',
      session_id: 'session-1',
      detail: {
        kind: 'tool_progress',
        tool_call_id: 'tool-1',
        tool_name: 'bash_command',
        delta: 'status',
        elapsed_time_seconds: 2,
      },
    },
  ];
}

describe('fromRemote', () => {
  it('normalizes remote session summaries with title fallback support', () => {
    const vm = normalizeRemoteSessionSummary(baseSession());
    expect(vm.title).toBe('Alpha Session');
    expect(vm.workspaceLabel).toBe('workspace-main');
    expect(vm.providerName).toBe('glm');
    expect(vm.model).toBe('glm-5.1');
  });

  it('hydrates message deltas, tool progress, and filters redundant daemon/session states', () => {
    const hydrated = hydrateRemoteTimeline([
      ...baseEvents(),
      {
        sequence: 5,
        recorded_at: '2026-04-14T09:00:05Z',
        runner_id: 'runner-1',
        session_id: 'session-1',
        detail: {
          kind: 'session_state_changed',
          previous_state: 'running',
          state: 'running',
        },
      },
      {
        sequence: 6,
        recorded_at: '2026-04-14T09:00:06Z',
        runner_id: 'runner-1',
        session_id: 'session-1',
        detail: {
          kind: 'daemon_presence_changed',
          state: 'online',
        },
      },
      {
        sequence: 7,
        recorded_at: '2026-04-14T09:00:07Z',
        runner_id: 'runner-1',
        session_id: 'session-1',
        detail: {
          kind: 'daemon_presence_changed',
          state: 'online',
        },
      },
      {
        sequence: 8,
        recorded_at: '2026-04-14T09:00:08Z',
        runner_id: 'runner-1',
        session_id: 'session-1',
        detail: {
          kind: 'message_committed',
          role: 'assistant',
          text: 'Hello',
          message_id: 'msg-1',
        },
      },
    ]);

    expect(hydrated).toHaveLength(3);
    expect(hydrated[0].detail.kind).toBe('tool_progress');
    expect((hydrated[0].detail.kind === 'tool_progress' && hydrated[0].detail.delta) || null).toBe(
      'git status',
    );
    expect(hydrated[1].detail.kind).toBe('daemon_presence_changed');
    expect(hydrated[2].detail.kind).toBe('message_committed');
  });

  it('deduplicates repeated event sequences', () => {
    const event = baseEvents()[0];
    const appended = appendRemoteTimelineEvent([event], event);
    expect(appended).toHaveLength(1);
  });

  it('builds a remote session bundle with hydrated timeline and panel data', () => {
    const approvals: RemoteApprovalRecord[] = [
      {
        approval_id: 'approval-1',
        session_id: 'session-1',
        runner_id: 'runner-1',
        state: 'pending',
        title: 'Approve shell command',
        description: 'Run git status',
        metadata: { blocked_path: 'C:\\repo' },
        created_at: '2026-04-14T09:10:00Z',
        updated_at: '2026-04-14T09:10:00Z',
        responded_at: null,
        responder: null,
        note: null,
      },
    ];
    const artifacts: RemoteArtifactRecord[] = [
      {
        artifact_id: 'artifact-1',
        session_id: 'session-1',
        runner_id: 'runner-1',
        name: 'Transcript',
        file_name: 'session.md',
        media_type: 'text/markdown',
        size_bytes: 1024,
        metadata: {},
        created_at: '2026-04-14T09:11:00Z',
      },
    ];

    const bundle = buildRemoteSessionBundle({
      session: baseSession(),
      events: [
        ...baseEvents(),
        {
          sequence: 8,
          recorded_at: '2026-04-14T09:00:08Z',
          runner_id: 'runner-1',
          session_id: 'session-1',
          detail: {
            kind: 'message_committed',
            role: 'assistant',
            text: 'Hello',
            message_id: 'msg-1',
          },
        },
      ],
      approvals,
      artifacts,
      connection: {
        state: 'open',
        canSendPrompt: true,
        canInterrupt: true,
        notice: null,
      },
      latestCursor: 8,
    });

    expect(bundle.session?.title).toBe('Alpha Session');
    expect(bundle.timeline[bundle.timeline.length - 1]?.kind).toBe('message');
    expect(bundle.approvals[0].blockedPath).toBe('C:\\repo');
    expect(bundle.artifacts[0].fileName).toBe('session.md');
    expect(bundle.latestCursor).toBe(8);
  });
});
