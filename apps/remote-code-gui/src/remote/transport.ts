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

interface RemoteSessionBundleData {
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

interface RemoteSessionStreamHandle {
  close(): void;
}

export function subscribeToRemoteSessionEvents(input: {
  baseUrl: string;
  sessionId: string;
  /** When provided, stream events directly from the runner instead of the control plane. */
  runnerBaseUrl?: string | null;
  getAfterSequence: () => number;
  onConnectionStateChange: (state: RemoteConnectionState) => void;
  onEvent: (event: RemoteTimelineEvent) => void;
}): RemoteSessionStreamHandle {
  let cancelled = false;
  let reconnectTimer: number | null = null;
  let socket: WebSocket | null = null;
  let reconnectAttempt = 0;

  const streamBaseUrl = input.runnerBaseUrl ?? input.baseUrl;

  const openSocket = (after: number) => {
    if (cancelled) {
      return;
    }

    if (reconnectTimer !== null) {
      window.clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }

    input.onConnectionStateChange(after > 0 ? 'reconnecting' : 'connecting');
    socket = new WebSocket(buildSessionEventsStreamUrl(streamBaseUrl, input.sessionId, after));

    socket.onopen = () => {
      if (!cancelled) {
        reconnectAttempt = 0;
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
      socket = null;
      scheduleReconnect();
    };
  };

  const scheduleReconnect = () => {
    if (cancelled || reconnectTimer !== null) {
      return;
    }

    const isOffline =
      typeof navigator !== 'undefined' &&
      'onLine' in navigator &&
      navigator.onLine === false;
    input.onConnectionStateChange('reconnecting');

    if (isOffline) {
      return;
    }

    const delayMs = Math.min(1_000 * 2 ** reconnectAttempt, 15_000);
    reconnectTimer = window.setTimeout(() => {
      reconnectTimer = null;
      reconnectAttempt += 1;
      openSocket(input.getAfterSequence());
    }, delayMs);
  };

  const handleOnline = () => {
    if (cancelled || socket) {
      return;
    }
    if (reconnectTimer !== null) {
      window.clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    reconnectAttempt = 0;
    openSocket(input.getAfterSequence());
  };

  const handleOffline = () => {
    if (!cancelled) {
      input.onConnectionStateChange('reconnecting');
    }
    if (reconnectTimer !== null) {
      window.clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  };

  window.addEventListener('online', handleOnline);
  window.addEventListener('offline', handleOffline);

  openSocket(input.getAfterSequence());

  return {
    close() {
      cancelled = true;
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
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