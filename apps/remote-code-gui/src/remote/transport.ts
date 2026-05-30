import {
  buildSessionEventsStreamUrl,
  createStreamTicket,
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

function isRemoteTimelineEvent(data: unknown): data is RemoteTimelineEvent {
  if (typeof data !== 'object' || data === null) return false;
  const obj = data as Record<string, unknown>;
  return typeof obj.sequence === 'number'
    && typeof obj.detail === 'object'
    && obj.detail !== null
    && typeof (obj.detail as Record<string, unknown>).kind === 'string';
}

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
  const MAX_RECONNECT_ATTEMPTS = 20;

  const streamBaseUrl = input.runnerBaseUrl ?? input.baseUrl;

  const openSocket = (after: number) => {
    void openSocketWithTicket(after);
  };

  const openSocketWithTicket = async (after: number) => {
    if (cancelled) {
      return;
    }

    if (reconnectTimer !== null) {
      window.clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }

    input.onConnectionStateChange(after > 0 ? 'reconnecting' : 'connecting');

    let streamTicket: string | null = null;
    try {
      if (streamBaseUrl === input.baseUrl) {
        const streamPath = `/v1/sessions/${encodeURIComponent(input.sessionId)}/events/stream`;
        const response = await createStreamTicket(input.baseUrl, streamPath);
        streamTicket = response.stream_ticket;
      }
    } catch {
      if (!cancelled) {
        input.onConnectionStateChange('error');
        scheduleReconnect();
      }
      return;
    }

    if (cancelled) {
      return;
    }

    socket = new WebSocket(
      buildSessionEventsStreamUrl(streamBaseUrl, input.sessionId, after, streamTicket),
    );

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
        const parsed: unknown = JSON.parse(message.data);
        if (!isRemoteTimelineEvent(parsed)) {
          console.warn('[transport] discarding invalid RemoteTimelineEvent:', parsed);
          return;
        }
        input.onEvent(parsed);
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

    if (reconnectAttempt >= MAX_RECONNECT_ATTEMPTS) {
      input.onConnectionStateChange('error');
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

  let onlineReconnectTimer: ReturnType<typeof setTimeout> | null = null;

  const handleOnline = () => {
    if (cancelled || socket) {
      return;
    }
    if (reconnectTimer !== null) {
      window.clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    if (onlineReconnectTimer !== null) {
      window.clearTimeout(onlineReconnectTimer);
    }
    // Debounce to coalesce rapid connectivity flaps
    onlineReconnectTimer = window.setTimeout(() => {
      onlineReconnectTimer = null;
      if (!cancelled && !socket) {
        reconnectAttempt = 0;
        openSocket(input.getAfterSequence());
      }
    }, 1000);
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
      if (onlineReconnectTimer !== null) {
        window.clearTimeout(onlineReconnectTimer);
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
