import type { RemoteApprovalDecision, RemoteTimelineEvent } from './types';
import type {
  TransportConfig,
  TransportStrategyType,
  ConnectionState,
  HealthReport,
  TransportMetrics,
  TransportCallbacks,
} from './unified-transport';
import { UnifiedTransport, probeEndpointHealth } from './unified-transport';
import { drainCommands, enqueueCommand } from './offline-queue';
import { isOnline, onNetworkChange } from '../lib/mobile/network';

export type { TransportConfig, TransportStrategyType, ConnectionState, HealthReport, TransportMetrics };

export interface ConnectionManagerState {
  connectionState: ConnectionState;
  strategy: TransportStrategyType | null;
  metrics: TransportMetrics | null;
  health: HealthReport | null;
  latestSequence: number;
}

type StateListener = (state: ConnectionManagerState) => void;
type EventListener = (event: RemoteTimelineEvent) => void;

const listeners = new Set<StateListener>();
const eventListeners = new Set<EventListener>();
let manager: ConnectionManager | null = null;

export function getConnectionManager(): ConnectionManager {
  if (!manager) {
    manager = new ConnectionManager();
  }
  return manager;
}

export function destroyConnectionManager(): void {
  if (manager) {
    manager.disconnect();
    manager = null;
  }
  listeners.clear();
  eventListeners.clear();
}

export function onConnectionManagerStateChange(listener: StateListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function onConnectionManagerEvent(listener: EventListener): () => void {
  eventListeners.add(listener);
  return () => eventListeners.delete(listener);
}

class ConnectionManager {
  private transport: UnifiedTransport | null = null;
  private _state: ConnectionManagerState = {
    connectionState: 'idle',
    strategy: null,
    metrics: null,
    health: null,
    latestSequence: 0,
  };
  private _config: TransportConfig | null = null;
  private _unsubscribeNetwork: (() => void) | null = null;

  get state(): ConnectionManagerState {
    return this._state;
  }

  async connect(config: TransportConfig, afterSequence = 0): Promise<void> {
    this.disconnect();

    this._config = config;
    this._state = {
      ...this._state,
      connectionState: 'connecting',
      strategy: config.strategy,
      latestSequence: afterSequence,
    };
    this.notify();

    const callbacks: TransportCallbacks = {
      onConnectionStateChange: (state) => {
        this._state = { ...this._state, connectionState: state };
        this.notify();
      },
      onEvent: (event) => {
        this._state = {
          ...this._state,
          latestSequence: Math.max(this._state.latestSequence, event.sequence),
        };
        eventListeners.forEach((fn) => fn(event));
      },
      onMetricsUpdate: (metrics) => {
        this._state = { ...this._state, metrics };
        this.notify();
      },
      onHealthReport: (health) => {
        this._state = { ...this._state, health };
        this.notify();
      },
      onError: (error) => {
        console.error('[ConnectionManager] transport error:', error);
      },
    };

    this.transport = new UnifiedTransport(config, callbacks);
    await this.transport.connect(afterSequence);

    // Drain queued commands from offline period (fire-and-forget so connect() resolves immediately).
    if (this._config) {
      const sessionId = this._config.sessionId;
      const transport = this.transport;
      Promise.resolve().then(async () => {
        try {
          const queued = await drainCommands(sessionId);
          for (const cmd of queued) {
            if (this.transport !== transport) break;
            try {
              await transport.sendCommand(cmd.command);
            } catch {
              // Re-enqueue failed command for next connect attempt.
              try { await enqueueCommand(sessionId, cmd.command); } catch { /* best effort */ }
              break;
            }
          }
        } catch {
          // Drain failed; commands remain queued for next attempt.
        }
      });
    }

    // Subscribe to network changes for auto-reconnect.
    this._unsubscribeNetwork = onNetworkChange((connected) => {
      if (connected && this.transport && this._state.connectionState === 'reconnecting') {
        void this.reconnect();
      }
    });
  }

  disconnect(): void {
    if (this._unsubscribeNetwork) {
      this._unsubscribeNetwork();
      this._unsubscribeNetwork = null;
    }
    if (this.transport) {
      this.transport.close();
      this.transport = null;
    }
    this._state = {
      ...this._state,
      connectionState: 'idle',
      strategy: null,
    };
    this.notify();
  }

  async sendPrompt(content: string): Promise<void> {
    if (!this.transport || !this._config) throw new Error('not connected');
    await this.transport.sendCommand({ kind: 'send_prompt', content });
  }

  async interrupt(): Promise<void> {
    if (!this.transport || !this._config) throw new Error('not connected');
    await this.transport.sendCommand({ kind: 'interrupt' });
  }

  async respondToApproval(
    approvalId: string,
    decision: RemoteApprovalDecision,
    note?: string,
  ): Promise<void> {
    if (!this.transport || !this._config) throw new Error('not connected');
    await this.transport.sendCommand({
      kind: 'respond_to_approval',
      approvalId,
      decision,
      note,
    });
  }

  async probeHealth(): Promise<HealthReport> {
    if (!this.transport) {
      return {
        runnerReachable: false,
        runnerLatencyMs: null,
        controlPlaneReachable: false,
        controlPlaneLatencyMs: null,
        recommendedStrategy: null,
      };
    }
    return this.transport.probeHealth();
  }

  getLatestSequence(): number {
    return this.transport?.getLatestSequence() ?? this._state.latestSequence;
  }

  private async reconnect(): Promise<void> {
    if (!this.transport || !this._config) return;
    try {
      await this.transport.connect(this._state.latestSequence);
    } catch (error) {
      console.error('[ConnectionManager] reconnect failed:', error);
    }
  }

  private notify(): void {
    listeners.forEach((fn) => fn({ ...this._state }));
  }
}