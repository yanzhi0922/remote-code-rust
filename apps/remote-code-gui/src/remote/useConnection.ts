import { useCallback, useEffect, useMemo, useRef, useSyncExternalStore } from 'react';
import type { RemoteTimelineEvent } from './types';
import {
  getConnectionManager,
  onConnectionManagerStateChange,
  onConnectionManagerEvent,
  destroyConnectionManager,
  type ConnectionManagerState,
  type TransportConfig,
  type TransportStrategyType,
  type ConnectionState,
  type HealthReport,
  type TransportMetrics,
} from './connection-manager';

export interface UseConnectionReturn {
  connectionState: ConnectionState;
  strategy: TransportStrategyType | null;
  metrics: TransportMetrics | null;
  health: HealthReport | null;
  connect: (config: TransportConfig, afterSequence?: number) => Promise<void>;
  disconnect: () => void;
  sendPrompt: (content: string) => Promise<void>;
  interrupt: () => Promise<void>;
  respondToApproval: (
    approvalId: string,
    decision: import('./types').RemoteApprovalDecision,
    note?: string,
  ) => Promise<void>;
  probeHealth: () => Promise<HealthReport>;
  latestSequence: number;
}

function subscribeToState(callback: () => void): () => void {
  return onConnectionManagerStateChange(() => callback());
}

function getSnapshot(): ConnectionManagerState {
  return getConnectionManager().state;
}

function getServerSnapshot(): ConnectionManagerState {
  return {
    connectionState: 'idle',
    strategy: null,
    metrics: null,
    health: null,
    latestSequence: 0,
  };
}

export function useConnection(
  onEvent?: (event: RemoteTimelineEvent) => void,
): UseConnectionReturn {
  const state = useSyncExternalStore(subscribeToState, getSnapshot, getServerSnapshot);

  // Keep the latest onEvent in a ref so we subscribe only once and avoid
  // listener churn on every render when the caller's callback identity changes.
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  useEffect(() => {
    const handler = (event: RemoteTimelineEvent) => onEventRef.current?.(event);
    const unsubscribe = onConnectionManagerEvent(handler);
    return () => {
      unsubscribe();
      destroyConnectionManager();
    };
  }, []);

  const connect = useCallback(async (config: TransportConfig, afterSequence = 0) => {
    const mgr = getConnectionManager();
    await mgr.connect(config, afterSequence);
  }, []);

  const disconnect = useCallback(() => {
    getConnectionManager().disconnect();
  }, []);

  const sendPrompt = useCallback(async (content: string) => {
    await getConnectionManager().sendPrompt(content);
  }, []);

  const interrupt = useCallback(async () => {
    await getConnectionManager().interrupt();
  }, []);

  const respondToApproval = useCallback(async (
    approvalId: string,
    decision: import('./types').RemoteApprovalDecision,
    note?: string,
  ) => {
    await getConnectionManager().respondToApproval(approvalId, decision, note);
  }, []);

  const probeHealth = useCallback(async (): Promise<HealthReport> => {
    return getConnectionManager().probeHealth();
  }, []);

  return useMemo(
    () => ({
      connectionState: state.connectionState,
      strategy: state.strategy,
      metrics: state.metrics,
      health: state.health,
      latestSequence: state.latestSequence,
      connect,
      disconnect,
      sendPrompt,
      interrupt,
      respondToApproval,
      probeHealth,
    }),
    [state, connect, disconnect, sendPrompt, interrupt, respondToApproval, probeHealth],
  );
}