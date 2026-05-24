import type {
  RemoteTimelineEvent,
  RemoteTimelineEventDetail,
} from '../../remote/types';

export function hydrateRemoteTimeline(events: RemoteTimelineEvent[]): RemoteTimelineEvent[] {
  const seen = new Set<number>();
  return [...events]
    .sort((left, right) => left.sequence - right.sequence)
    .reduce<RemoteTimelineEvent[]>((current, event) => appendRemoteTimelineEvent(current, event, seen), []);
}

export function appendRemoteTimelineEvent(
  current: RemoteTimelineEvent[],
  nextEvent: RemoteTimelineEvent,
  seen: Set<number> = new Set(),
): RemoteTimelineEvent[] {
  if (seen.has(nextEvent.sequence) || current.some((event) => event.sequence === nextEvent.sequence)) {
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
    seen.add(nextEvent.sequence);
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
      seen.add(nextEvent.sequence);
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
      seen.add(nextEvent.sequence);
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

  seen.add(nextEvent.sequence);
  return [...current, nextEvent].sort((left, right) => left.sequence - right.sequence);
}

export function resolveRemoteSessionTitle(session: { metadata: Record<string, string>; workspace_id: string }): string {
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
