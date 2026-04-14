import {
  buildSessionEventsStreamUrl,
  listSessionApprovals,
  listSessionArtifacts,
  listSessionEvents,
} from './api';
import type { RemoteConnectionState } from './i18n';
import type {
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteTimelineEvent,
} from './types';
import { hydrateRemoteTimeline } from '../session/normalize/fromRemote';

export interface RemoteSessionBundleData {
  events: RemoteTimelineEvent[];
  approvals: RemoteApprovalRecord[];
  artifacts: RemoteArtifactRecord[];
  latestSequence: number;
}

export async function loadRemoteSessionBundle(
  baseUrl: string,
  sessionId: string,
): Promise<RemoteSessionBundleData> {
  const [eventsResponse, approvalsResponse, artifactsResponse] = await Promise.all([
    listSessionEvents(baseUrl, sessionId),
    listSessionApprovals(baseUrl, sessionId),
    listSessionArtifacts(baseUrl, sessionId),
  ]);

  const events = hydrateRemoteTimeline(eventsResponse.items);
  return {
    events,
    approvals: sortApprovals(approvalsResponse.items),
    artifacts: sortArtifacts(artifactsResponse.items),
    latestSequence: eventsResponse.latest_sequence ?? events[events.length - 1]?.sequence ?? 0,
  };
}

export interface RemoteSessionStreamHandle {
  close(): void;
}

export function subscribeToRemoteSessionEvents(input: {
  baseUrl: string;
  sessionId: string;
  getAfterSequence: () => number;
  onConnectionStateChange: (state: RemoteConnectionState) => void;
  onEvent: (event: RemoteTimelineEvent) => void;
}): RemoteSessionStreamHandle {
  let cancelled = false;
  let reconnectTimer: number | null = null;
  let socket: WebSocket | null = null;

  const openSocket = (after: number) => {
    if (cancelled) {
      return;
    }

    input.onConnectionStateChange(after > 0 ? 'reconnecting' : 'connecting');
    socket = new WebSocket(buildSessionEventsStreamUrl(input.baseUrl, input.sessionId, after));

    socket.onopen = () => {
      if (!cancelled) {
        input.onConnectionStateChange('open');
      }
    };

    socket.onmessage = (message) => {
      if (typeof message.data !== 'string') {
        return;
      }

      try {
        input.onEvent(JSON.parse(message.data) as RemoteTimelineEvent);
      } catch {
        input.onConnectionStateChange('error');
      }
    };

    socket.onerror = () => {
      if (!cancelled) {
        input.onConnectionStateChange('error');
      }
    };

    socket.onclose = () => {
      if (cancelled) {
        return;
      }
      reconnectTimer = window.setTimeout(() => {
        openSocket(input.getAfterSequence());
      }, 1_000);
    };
  };

  openSocket(input.getAfterSequence());

  return {
    close() {
      cancelled = true;
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
      }
      socket?.close();
    },
  };
}

function sortApprovals(items: RemoteApprovalRecord[]): RemoteApprovalRecord[] {
  return [...items].sort(
    (left, right) => new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime(),
  );
}

function sortArtifacts(items: RemoteArtifactRecord[]): RemoteArtifactRecord[] {
  return [...items].sort(
    (left, right) => new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
  );
}
