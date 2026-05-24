import type {
  RemoteApprovalDecision,
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteBootstrapClaimResponse,
  RemoteCommandResponse,
  RemoteControlPlaneHealth,
  RemoteListResponse,
  RemotePairingAcceptResponse,
  RemoteSessionRecord,
  RemoteStreamTicketResponse,
  RemoteTimelineEvent,
} from './types';
import {
  resolveRemoteAccessToken,
  resolveRemoteRefreshToken,
  persistRemoteAccessToken,
  clearRemoteAccessToken,
  clearRemoteRefreshToken,
} from '../lib/runtime';

interface RemoteErrorEnvelope {
  error?: {
    message?: string;
  };
}

export async function listSessions(baseUrl: string): Promise<RemoteListResponse<RemoteSessionRecord>> {
  return requestJson<RemoteListResponse<RemoteSessionRecord>>(baseUrl, '/v1/sessions');
}

export async function getControlPlaneHealth(baseUrl: string): Promise<RemoteControlPlaneHealth> {
  return requestJson<RemoteControlPlaneHealth>(baseUrl, '/healthz');
}

export async function bootstrapControlPlane(
  baseUrl: string,
  bootstrapSecret: string,
  deviceName: string,
): Promise<RemoteBootstrapClaimResponse> {
  return requestJson<RemoteBootstrapClaimResponse>(baseUrl, '/v1/bootstrap/claim', {
    method: 'POST',
    body: JSON.stringify({
      bootstrap_secret: bootstrapSecret,
      device_name: deviceName,
      device_kind: 'browser',
    }),
  });
}

export async function acceptPairingOffer(
  baseUrl: string,
  offerId: string,
  pairingSecret: string,
  deviceName?: string,
): Promise<RemotePairingAcceptResponse> {
  return requestJson<RemotePairingAcceptResponse>(baseUrl, '/v1/pairing/accept', {
    method: 'POST',
    body: JSON.stringify({
      offer_id: offerId,
      pairing_secret: pairingSecret,
      device_name: deviceName?.trim() ? deviceName : null,
      device_kind: 'browser',
    }),
  });
}

export async function listSessionEvents(
  baseUrl: string,
  sessionId: string,
  limit = 200,
): Promise<RemoteListResponse<RemoteTimelineEvent>> {
  return requestJson<RemoteListResponse<RemoteTimelineEvent>>(
    baseUrl,
    `/v1/sessions/${encodeURIComponent(sessionId)}/events?limit=${limit}`,
  );
}

export async function createStreamTicket(
  baseUrl: string,
  path: string,
): Promise<RemoteStreamTicketResponse> {
  return requestJson<RemoteStreamTicketResponse>(baseUrl, '/v1/stream-ticket', {
    method: 'POST',
    body: JSON.stringify({ path }),
  });
}

export async function listSessionApprovals(
  baseUrl: string,
  sessionId: string,
): Promise<RemoteListResponse<RemoteApprovalRecord>> {
  return requestJson<RemoteListResponse<RemoteApprovalRecord>>(
    baseUrl,
    `/v1/sessions/${encodeURIComponent(sessionId)}/approvals`,
  );
}

export async function listSessionArtifacts(
  baseUrl: string,
  sessionId: string,
): Promise<RemoteListResponse<RemoteArtifactRecord>> {
  return requestJson<RemoteListResponse<RemoteArtifactRecord>>(
    baseUrl,
    `/v1/sessions/${encodeURIComponent(sessionId)}/artifacts`,
  );
}

export async function sendPrompt(
  baseUrl: string,
  sessionId: string,
  content: string,
  _runnerBaseUrl?: string | null,
): Promise<RemoteCommandResponse> {
  return requestJson<RemoteCommandResponse>(
    baseUrl,
    `/v1/sessions/${encodeURIComponent(sessionId)}/commands`,
    {
      method: 'POST',
      body: JSON.stringify({
        kind: 'send_prompt',
        content,
      }),
    },
  );
}

export async function interruptSession(
  baseUrl: string,
  sessionId: string,
  _runnerBaseUrl?: string | null,
): Promise<RemoteCommandResponse> {
  return requestJson<RemoteCommandResponse>(
    baseUrl,
    `/v1/sessions/${encodeURIComponent(sessionId)}/commands`,
    {
      method: 'POST',
      body: JSON.stringify({
        kind: 'interrupt',
      }),
    },
  );
}

export async function respondToApproval(
  baseUrl: string,
  approvalId: string,
  decision: RemoteApprovalDecision,
  note?: string,
  _runnerBaseUrl?: string | null,
): Promise<RemoteApprovalRecord> {
  return requestJson<RemoteApprovalRecord>(
    baseUrl,
    `/v1/approvals/${encodeURIComponent(approvalId)}/decision`,
    {
      method: 'POST',
      body: JSON.stringify({
        decision,
        responder: 'remote-code-gui',
        note: note ?? null,
      }),
    },
  );
}

export function buildArtifactDownloadUrl(baseUrl: string, artifactId: string): string {
  return buildHttpUrl(baseUrl, `/v1/artifacts/${encodeURIComponent(artifactId)}/download`);
}

export function buildSessionEventsStreamUrl(
  baseUrl: string,
  sessionId: string,
  after: number,
  streamTicket?: string | null,
): string {
  const wsUrl = new URL(
    `/v1/sessions/${encodeURIComponent(sessionId)}/events/stream?after=${after}`,
    buildHttpUrl(baseUrl, '/'),
  );
  wsUrl.protocol = wsUrl.protocol === 'https:' ? 'wss:' : 'ws:';
  if (streamTicket) {
    wsUrl.searchParams.set('stream_ticket', streamTicket);
  }
  return wsUrl.toString();
}

let _refreshPromise: Promise<boolean> | null = null;

async function tryRefreshAccessToken(baseUrl: string): Promise<boolean> {
  if (_refreshPromise) return _refreshPromise;

  const p = (async () => {
    const refreshToken = resolveRemoteRefreshToken();
    if (!refreshToken) return false;

    try {
      const response = await fetch(buildHttpUrl(baseUrl, '/v1/auth/refresh'), {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ refresh_token: refreshToken }),
        signal: AbortSignal.timeout(5000),
      });

      if (!response.ok) {
        if (response.status === 401) {
          clearRemoteAccessToken();
          clearRemoteRefreshToken();
        }
        return false;
      }

      const data = await response.json() as { access_token: string };
      persistRemoteAccessToken(data.access_token);
      return true;
    } catch {
      return false;
    }
  })();
  _refreshPromise = p;
  p.finally(() => { if (_refreshPromise === p) _refreshPromise = null; });

  return p;
}

export async function requestJson<T>(
  baseUrl: string,
  path: string,
  init?: RequestInit,
): Promise<T> {
  const method = (init?.method ?? 'GET').toUpperCase();
  const requestUrl = buildHttpUrl(baseUrl, path);
  const canRetry = method === 'GET';

  for (let attempt = 0; ; attempt += 1) {
    try {
      const response = await fetchWithTimeout(requestUrl, {
        ...init,
        cache: method === 'GET' ? 'no-store' : init?.cache,
        headers: buildRequestHeaders(init),
      });

      if (response.status === 401 && attempt === 0) {
        // Read and discard the body before retrying.
        try { await response.clone().text(); } catch { /* ignore */ }
        const refreshed = await tryRefreshAccessToken(baseUrl);
        if (refreshed) continue;
      }

      if (!response.ok) {
        // Read only the first 1KB for error messages.
        let errorText = '';
        try {
          const reader = response.body?.getReader();
          if (reader) {
            const { value } = await reader.read();
            if (value) errorText = new TextDecoder().decode(value).substring(0, 1024);
          } else {
            errorText = (await response.text()).substring(0, 1024);
          }
        } catch {
          errorText = '';
        }
        throw new Error(extractRemoteError(errorText, response.status));
      }

      const responseText = await response.text();

      if (!responseText) {
        if (method !== 'GET') {
          console.warn(`[api] requestJson: empty response body for ${method} ${path}, returning empty object`);
        }
        return {} as T;
      }

      return JSON.parse(responseText) as T;
    } catch (error) {
      if (!canRetry || attempt >= 1 || !isRetryableRemoteError(error)) {
        throw error;
      }
      await delay(350 * (attempt + 1));
    }
  }
}

function buildHttpUrl(baseUrl: string, path: string): string {
  return new URL(path, `${baseUrl.replace(/\/$/, '')}/`).toString();
}

function buildAuthHeaders(): HeadersInit {
  const token = resolveRemoteAccessToken();
  if (!token) {
    return {};
  }
  return {
    authorization: `Bearer ${token}`,
  };
}

function buildRequestHeaders(init?: RequestInit): Headers {
  const headers = new Headers(init?.headers ?? {});
  headers.set('accept', 'application/json');

  for (const [key, value] of Object.entries(buildAuthHeaders())) {
    if (!headers.has(key)) {
      headers.set(key, value);
    }
  }

  if (shouldAttachJsonContentType(init?.body) && !headers.has('content-type')) {
    headers.set('content-type', 'application/json');
  }

  return headers;
}

function shouldAttachJsonContentType(body: BodyInit | null | undefined): boolean {
  if (body == null) {
    return false;
  }
  if (typeof FormData !== 'undefined' && body instanceof FormData) {
    return false;
  }
  if (typeof URLSearchParams !== 'undefined' && body instanceof URLSearchParams) {
    return false;
  }
  if (typeof Blob !== 'undefined' && body instanceof Blob) {
    return false;
  }
  return true;
}

async function fetchWithTimeout(input: string, init: RequestInit): Promise<Response> {
  const controller = new AbortController();
  const timeoutId = window.setTimeout(() => {
    controller.abort(new DOMException('Remote request timed out.', 'AbortError'));
  }, 15_000);
  const cleanupSignal = pipeAbortSignal(init.signal, controller);

  try {
    return await fetch(input, {
      ...init,
      signal: controller.signal,
    });
  } finally {
    cleanupSignal();
    window.clearTimeout(timeoutId);
  }
}

function pipeAbortSignal(
  signal: AbortSignal | null | undefined,
  controller: AbortController,
): () => void {
  if (!signal) {
    return () => {};
  }

  if (signal.aborted) {
    controller.abort(signal.reason);
    return () => {};
  }

  const handleAbort = () => {
    controller.abort(signal.reason);
  };
  signal.addEventListener('abort', handleAbort, { once: true });
  return () => {
    signal.removeEventListener('abort', handleAbort);
  };
}

function isRetryableRemoteError(error: unknown): boolean {
  if (error instanceof DOMException && error.name === 'AbortError') {
    return true;
  }
  if (error instanceof TypeError) {
    return true;
  }
  if (error instanceof Error) {
    return /NetworkError|Failed to fetch/i.test(error.message);
  }
  return false;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function extractRemoteError(payload: string, status: number): string {
  if (!payload) {
    return `Remote request failed with HTTP ${status}.`;
  }

  try {
    const parsed = JSON.parse(payload) as RemoteErrorEnvelope;
    const message = parsed.error?.message?.trim();
    if (message) {
      return `Remote request failed with HTTP ${status}: ${message}`;
    }
  } catch {
    // Fall through to plain-text payloads.
  }

  return `Remote request failed with HTTP ${status}: ${payload.substring(0, 200)}`;
}
