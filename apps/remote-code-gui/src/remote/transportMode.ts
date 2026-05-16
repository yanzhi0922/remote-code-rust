import type { TransportStrategyType } from './connection-manager';
import type { RemoteSessionRecord } from './types';

type RemoteTransportMode = 'relay_only' | 'hybrid' | 'direct_only';

export function resolveRemoteTransportMode(): RemoteTransportMode {
  const raw = import.meta.env.VITE_REMOTE_CODE_TRANSPORT_MODE?.trim().toLowerCase();
  if (raw === 'hybrid' || raw === 'direct_only') {
    return raw;
  }
  return 'relay_only';
}

export function resolveRemoteRunnerBaseUrl(session: RemoteSessionRecord | null): string | null {
  const mode = resolveRemoteTransportMode();
  if (mode === 'relay_only') {
    return null;
  }
  return session?.owner_runner_public_base_url?.trim() || null;
}

export function resolveRemoteTransportStrategy(
  session: RemoteSessionRecord | null,
): TransportStrategyType {
  const mode = resolveRemoteTransportMode();
  const runnerBaseUrl = resolveRemoteRunnerBaseUrl(session);
  if (mode === 'direct_only' && runnerBaseUrl) {
    return 'direct_websocket';
  }
  if (mode === 'hybrid' && runnerBaseUrl) {
    return 'hybrid';
  }
  return 'server_relay';
}
