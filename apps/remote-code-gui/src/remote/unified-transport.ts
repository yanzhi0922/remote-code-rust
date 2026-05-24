import type { RemoteTimelineEvent } from './types';
import { subscribeToRemoteSessionEvents } from './transport';
import { invoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import { hasTauriRuntime, resolveRemoteAccessToken } from '../lib/runtime';
import { enqueueCommand, drainCommands } from './offline-queue';
import type { TransportCommandPayload } from './offline-queue';
import { listSessionEvents, requestJson } from './api';

// ─────────────────────────────────────────────────────────────────────────────
// Strategy types — mirrors the Rust rc-remote-transport crate
// ─────────────────────────────────────────────────────────────────────────────

export type TransportStrategyType =
  | 'direct_websocket'
  | 'server_relay'
  | 'outbound_polling'
  | 'hybrid'
  | 'quic';

export interface TransportConfig {
  strategy: TransportStrategyType;
  baseUrl: string;
  runnerBaseUrl?: string | null;
  /** Direct runner links are an explicit advanced mode; production defaults to relay-only. */
  allowDirectRunner?: boolean;
  sessionId: string;
  authToken?: string | null;
  /** QUIC-specific */
  quicServerUrl?: string;
  quicCertFingerprint?: string;
  /** Polling-specific */
  pollIntervalMs?: number;
}

export type ConnectionState =
  | 'idle'
  | 'probing'
  | 'connecting'
  | 'open'
  | 'reconnecting'
  | 'error';

export interface HealthReport {
  runnerReachable: boolean;
  runnerLatencyMs: number | null;
  controlPlaneReachable: boolean;
  controlPlaneLatencyMs: number | null;
  recommendedStrategy: TransportStrategyType | null;
}

export interface TransportMetrics {
  eventsReceived: number;
  eventsDropped: number;
  commandsSent: number;
  commandsFailed: number;
  reconnectCount: number;
  strategySwitches: number;
  activeStrategy: TransportStrategyType;
  latencyMs: number | null;
}

export interface TransportCallbacks {
  onConnectionStateChange: (state: ConnectionState) => void;
  onEvent: (event: RemoteTimelineEvent) => void;
  onMetricsUpdate: (metrics: TransportMetrics) => void;
  onHealthReport: (report: HealthReport) => void;
  onError: (error: Error) => void;
}

interface TransportHandle {
  close(): void;
  readonly state: ConnectionState;
  readonly strategy: TransportStrategyType;
  readonly metrics: TransportMetrics;
}

// ─────────────────────────────────────────────────────────────────────────────
// Health probe
// ─────────────────────────────────────────────────────────────────────────────

export async function probeEndpointHealth(
  baseUrl: string,
  timeoutMs = 3000,
): Promise<{ reachable: boolean; latencyMs: number | null; authValid: boolean }> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), timeoutMs);
  const token = resolveRemoteAccessToken();

  try {
    const start = performance.now();
    const url = new URL('/healthz', `${baseUrl.replace(/\/$/, '')}/`);
    const response = await fetch(url.toString(), {
      method: 'GET',
      signal: controller.signal,
      headers: token ? { authorization: `Bearer ${token}` } : {},
    });
    const latencyMs = Math.round(performance.now() - start);

    if (response.ok) {
      return { reachable: true, latencyMs, authValid: true };
    }
    if (response.status === 401) {
      return { reachable: true, latencyMs, authValid: false };
    }
    return { reachable: false, latencyMs: null, authValid: false };
  } catch {
    return { reachable: false, latencyMs: null, authValid: false };
  } finally {
    window.clearTimeout(timer);
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unified transport — delegates to strategy-specific implementations
// ─────────────────────────────────────────────────────────────────────────────

export class UnifiedTransport implements TransportHandle {
  private _state: ConnectionState = 'idle';
  private _strategy: TransportStrategyType;
  private _metrics: TransportMetrics;
  private _config: TransportConfig;
  private _callbacks: TransportCallbacks;
  private _handle: { close(): void } | null = null;
  private _latestSequence = 0;
  private _cancelled = false;
  private _reconnectAttempt = 0;
  private _reconnectTimer: number | null = null;
  private _healthTimer: number | null = null;
  private _pollTimer: number | null = null;

  constructor(config: TransportConfig, callbacks: TransportCallbacks) {
    this._config = config;
    this._strategy = config.strategy;
    this._callbacks = callbacks;
    this._metrics = {
      eventsReceived: 0,
      eventsDropped: 0,
      commandsSent: 0,
      commandsFailed: 0,
      reconnectCount: 0,
      strategySwitches: 0,
      activeStrategy: config.strategy,
      latencyMs: null,
    };
  }

  get state(): ConnectionState {
    return this._state;
  }

  get strategy(): TransportStrategyType {
    return this._strategy;
  }

  get metrics(): TransportMetrics {
    return this._metrics;
  }

  async connect(afterSequence = 0): Promise<void> {
    this._cancelled = false;
    this._latestSequence = afterSequence;
    this.setState('connecting');

    try {
      switch (this._strategy) {
        case 'direct_websocket':
          if (!this.canUseDirectRunner()) {
            this._strategy = 'server_relay';
            await this.connectWebSocket(this._config.baseUrl, afterSequence);
            break;
          }
          await this.connectWebSocket(this.getStreamBaseUrl('direct'), afterSequence);
          break;
        case 'server_relay':
          await this.connectWebSocket(this._config.baseUrl, afterSequence);
          break;
        case 'outbound_polling':
          await this.startPolling(afterSequence);
          break;
        case 'hybrid':
          await this.connectHybrid(afterSequence);
          break;
        case 'quic':
          await this.connectQuic(afterSequence);
          break;
      }
    } catch (error) {
      this.setState('error');
      this._callbacks.onError(error instanceof Error ? error : new Error(String(error)));
    }
  }

  close(): void {
    this._cancelled = true;
    this._handle?.close();
    this._handle = null;

    if (this._reconnectTimer !== null) {
      window.clearTimeout(this._reconnectTimer);
      this._reconnectTimer = null;
    }
    if (this._healthTimer !== null) {
      window.clearInterval(this._healthTimer);
      this._healthTimer = null;
    }
    if (this._pollTimer !== null) {
      window.clearInterval(this._pollTimer);
      this._pollTimer = null;
    }

    this.setState('idle');
  }

  async sendCommand(command: TransportCommandPayload): Promise<void> {
    this._metrics.commandsSent++;
    this.notifyMetrics();

    // If offline, queue for later.
    if (this._state === 'reconnecting' || this._state === 'error' || this._state === 'idle') {
      await enqueueCommand(this._config.sessionId, command);
      return;
    }

    try {
      const runnerBaseUrl = this._config.runnerBaseUrl;
      const target = this._strategy === 'direct_websocket' && runnerBaseUrl
        ? runnerBaseUrl
        : this._config.baseUrl;

      await this.executeCommand(target, command);

      // Drain any queued commands after successful send.
      const queued = await drainCommands(this._config.sessionId);
      const failed: typeof queued = [];
      for (const cmd of queued) {
        try {
          await this.executeCommand(target, cmd.command);
        } catch {
          failed.push(cmd);
        }
      }
      // Re-enqueue commands that failed to send so they aren't lost.
      for (const cmd of failed) {
        await enqueueCommand(this._config.sessionId, cmd.command);
      }
    } catch (error) {
      this._metrics.commandsFailed++;
      this.notifyMetrics();
      await enqueueCommand(this._config.sessionId, command);
      throw error;
    }
  }

  async probeHealth(): Promise<HealthReport> {
    const runnerBaseUrl = this.directRunnerBaseUrl();
    const [runner, cp] = await Promise.all([
      runnerBaseUrl
        ? probeEndpointHealth(runnerBaseUrl, 2000)
        : Promise.resolve({ reachable: false, latencyMs: null as number | null, authValid: false }),
      probeEndpointHealth(this._config.baseUrl, 5000),
    ]);

    const report: HealthReport = {
      runnerReachable: runner.reachable,
      runnerLatencyMs: runner.latencyMs,
      controlPlaneReachable: cp.reachable,
      controlPlaneLatencyMs: cp.latencyMs,
      recommendedStrategy: null,
    };

    if (this.canUseDirectRunner() && runner.reachable && runner.authValid) {
      report.recommendedStrategy = 'direct_websocket';
    } else if (cp.reachable && cp.authValid) {
      report.recommendedStrategy = 'server_relay';
    }

    this._callbacks.onHealthReport(report);
    return report;
  }

  getLatestSequence(): number {
    return this._latestSequence;
  }

  // ── Private methods ────────────────────────────────────────────────────

  private getStreamBaseUrl(preference: 'direct' | 'relay'): string {
    const runnerBaseUrl = this.directRunnerBaseUrl();
    if (preference === 'direct' && runnerBaseUrl) {
      return runnerBaseUrl;
    }
    return this._config.baseUrl;
  }

  private canUseDirectRunner(): boolean {
    return this.directRunnerBaseUrl() !== null;
  }

  private directRunnerBaseUrl(): string | null {
    if (this._config.allowDirectRunner !== true) return null;
    return this._config.runnerBaseUrl?.trim() || null;
  }

  private setState(state: ConnectionState): void {
    this._state = state;
    this._callbacks.onConnectionStateChange(state);
  }

  private notifyMetrics(): void {
    this._callbacks.onMetricsUpdate({ ...this._metrics, activeStrategy: this._strategy });
  }

  private connectWebSocket(baseUrl: string, after: number): Promise<void> {
    return new Promise((resolve, reject) => {
      let resolved = false;
      let timeoutId = window.setTimeout(() => {
        if (!resolved) {
          resolved = true;
          reject(new Error('WebSocket connection timed out after 30 seconds'));
        }
      }, 30_000);

      this._handle = subscribeToRemoteSessionEvents({
        baseUrl,
        sessionId: this._config.sessionId,
        runnerBaseUrl: this._config.runnerBaseUrl,
        getAfterSequence: () => this._latestSequence,
        onConnectionStateChange: (state) => {
          if (this._cancelled) return;
          const mapped = mapConnectionState(state);
          this.setState(mapped);
          if (mapped === 'reconnecting') {
            this._metrics.reconnectCount++;
            this.notifyMetrics();
          }
          // Resolve the connect() promise only when the WebSocket actually opens.
          if (mapped === 'open' && !resolved) {
            resolved = true;
            window.clearTimeout(timeoutId);
            resolve();
          }
          // Reject if the connection enters an error or reconnecting state before opening.
          if ((mapped === 'error') && !resolved) {
            resolved = true;
            window.clearTimeout(timeoutId);
            reject(new Error('WebSocket connection entered error state'));
          }
        },
        onEvent: (event) => {
          if (this._cancelled) return;
          this._latestSequence = Math.max(this._latestSequence, event.sequence);
          this._metrics.eventsReceived++;
          this.notifyMetrics();
          this._callbacks.onEvent(event);
        },
      });

      // Set initial state to 'connecting'; the actual 'open' state is set via
      // onConnectionStateChange when the WebSocket handshake completes.
      this.setState('connecting');
    });
  }

  private async connectHybrid(after: number): Promise<void> {
    const report = await this.probeHealth();

    if (this.canUseDirectRunner() && report.runnerReachable && report.runnerLatencyMs !== null) {
      this._strategy = 'direct_websocket';
      await this.connectWebSocket(this.directRunnerBaseUrl()!, after);
    } else {
      this._strategy = 'server_relay';
      await this.connectWebSocket(this._config.baseUrl, after);
    }

    // Periodic health probe for auto-switching.
    try {
      if (this._healthTimer !== null) {
        window.clearInterval(this._healthTimer);
      }
    } catch { /* ignore exception between clear and set */ }
    this._healthTimer = window.setInterval(() => {
      void this.autoSwitchIfNeeded().catch((err) => {
        console.warn('[unified-transport] autoSwitchIfNeeded failed:', err);
      });
    }, 30_000);
  }

  private async autoSwitchIfNeeded(): Promise<void> {
    if (this._cancelled) return;

    const report = await this.probeHealth();
    const currentIsDirect = this._strategy === 'direct_websocket';
    const shouldDirect = report.recommendedStrategy === 'direct_websocket';

    if (currentIsDirect !== shouldDirect) {
      this._metrics.strategySwitches++;
      this.notifyMetrics();
      this._handle?.close();

      if (shouldDirect && this.canUseDirectRunner()) {
        this._strategy = 'direct_websocket';
        this.connectWebSocket(this.directRunnerBaseUrl()!, this._latestSequence);
      } else {
        this._strategy = 'server_relay';
        this.connectWebSocket(this._config.baseUrl, this._latestSequence);
      }
    }
  }

  private async startPolling(after: number): Promise<void> {
    this.setState('open');
    this._latestSequence = after;

    const pollInterval = this._config.pollIntervalMs ?? 3000;

    const poll = async () => {
      if (this._cancelled) return;
      try {
        const response = await listSessionEvents(
          this._config.baseUrl,
          this._config.sessionId,
          100,
        );

        for (const event of response.items) {
          if (event.sequence > this._latestSequence) {
            this._latestSequence = Math.max(this._latestSequence, event.sequence);
            this._metrics.eventsReceived++;
            this._callbacks.onEvent(event);
          }
        }
        this.notifyMetrics();
        this.setState('open');
      } catch (error) {
        this._metrics.eventsDropped++;
        this.notifyMetrics();
        this.setState('reconnecting');
      }
    };

    await poll();
    this._pollTimer = window.setInterval(() => {
      void poll();
    }, pollInterval);
  }

  private async connectQuic(_after: number): Promise<void> {
    if (!hasTauriRuntime()) {
      this._strategy = 'server_relay';
      await this.connectWebSocket(this._config.baseUrl, _after);
      return;
    }

    try {
      await invoke('quic_connect', {
        url: this._config.quicServerUrl ?? this._config.baseUrl,
        token: this._config.authToken ?? '',
        sessionId: this._config.sessionId,
        serverCertFingerprint: this._config.quicCertFingerprint ?? null,
      });
      this.setState('open');

      const unlisten = await tauriListen<RemoteTimelineEvent>('quic-event', (event) => {
        if (this._cancelled) return;
        this._latestSequence = Math.max(this._latestSequence, event.payload.sequence);
        this._metrics.eventsReceived++;
        this.notifyMetrics();
        this._callbacks.onEvent(event.payload);
      });

      this._handle = {
        close: () => {
          unlisten();
          invoke('quic_disconnect').catch((err) => { console.warn('[unified-transport] quic_disconnect failed:', err); });
        },
      };
    } catch (error) {
      this._strategy = 'server_relay';
      await this.connectWebSocket(this._config.baseUrl, _after);
    }
  }

  private async executeCommand(
    target: string,
    command: TransportCommandPayload,
  ): Promise<void> {
    if (command.kind === 'send_prompt') {
      await requestJson(target, `/v1/sessions/${encodeURIComponent(this._config.sessionId)}/commands`, {
        method: 'POST',
        body: JSON.stringify({ kind: 'send_prompt', content: command.content }),
      });
    } else if (command.kind === 'interrupt') {
      await requestJson(target, `/v1/sessions/${encodeURIComponent(this._config.sessionId)}/commands`, {
        method: 'POST',
        body: JSON.stringify({ kind: 'interrupt' }),
      });
    } else if (command.kind === 'respond_to_approval') {
      await requestJson(target, `/v1/approvals/${encodeURIComponent(command.approvalId)}/decision`, {
        method: 'POST',
        body: JSON.stringify({
          decision: command.decision,
          responder: 'remote-code-gui',
          note: command.note ?? null,
        }),
      });
    }
  }
}

function mapConnectionState(state: string): ConnectionState {
  switch (state) {
    case 'open':
      return 'open';
    case 'connecting':
      return 'connecting';
    case 'reconnecting':
      return 'reconnecting';
    case 'error':
      return 'error';
    default:
      return 'idle';
  }
}
