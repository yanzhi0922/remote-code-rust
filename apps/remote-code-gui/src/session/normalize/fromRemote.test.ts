import { describe, expect, it } from 'vitest';
import {
  appendRemoteTimelineEvent,
  hydrateRemoteTimeline,
  resolveRemoteSessionTitle,
} from './fromRemote';
import type { RemoteTimelineEvent } from '../../remote/types';

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

  it('deduplicates non-merge repeated event sequences', () => {
    const event: RemoteTimelineEvent = {
      sequence: 99,
      recorded_at: '2026-04-14T09:00:09Z',
      runner_id: 'runner-1',
      session_id: 'session-1',
      detail: {
        kind: 'session_state_changed',
        previous_state: 'pending',
        state: 'running',
      },
    };

    const appended = appendRemoteTimelineEvent([event], event);

    expect(appended).toHaveLength(1);
    expect(appended[0]).toBe(event);
  });

  it('resolves title from metadata when present', () => {
    expect(
      resolveRemoteSessionTitle({
        metadata: { title: '  My Session  ' },
        workspace_id: 'ws-1',
      }),
    ).toBe('My Session');
  });

  it('falls back to workspace_id when title is empty', () => {
    expect(
      resolveRemoteSessionTitle({
        metadata: {},
        workspace_id: 'ws-fallback',
      }),
    ).toBe('ws-fallback');
  });
});
