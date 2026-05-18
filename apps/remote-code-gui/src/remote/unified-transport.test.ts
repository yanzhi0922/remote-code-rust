import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { UnifiedTransport, probeEndpointHealth } from './unified-transport';
import type { TransportConfig, TransportCallbacks, TransportStrategyType } from './unified-transport';
import type { RemoteTimelineEvent } from './types';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockTransport = vi.hoisted(() => ({
  close: vi.fn(),
}));

const mockApi = vi.hoisted(() => ({
  requestJson: vi.fn(),
  listSessionEvents: vi.fn(),
}));

const mockOfflineQueue = vi.hoisted(() => ({
  enqueueCommand: vi.fn(),
  drainCommands: vi.fn(() => Promise.resolve([])),
}));

const mockRuntime = vi.hoisted(() => ({
  resolveRemoteAccessToken: vi.fn(() => 'test-token'),
  hasTauriRuntime: vi.fn(() => false),
}));

vi.mock('./transport', () => ({
  subscribeToRemoteSessionEvents: vi.fn(() => ({
    close: mockTransport.close,
  })),
}));

vi.mock('./api', () => mockApi);
vi.mock('./offline-queue', () => mockOfflineQueue);
vi.mock('../lib/runtime', () => mockRuntime);

function makeConfig(overrides?: Partial<TransportConfig>): TransportConfig {
  return {
    strategy: 'server_relay',
    baseUrl: 'https://cp.test',
    runnerBaseUrl: null,
    sessionId: 'session-1',
    authToken: 'token-1',
    ...overrides,
  };
}

function makeCallbacks(): TransportCallbacks & {
  states: string[];
  events: RemoteTimelineEvent[];
  errors: Error[];
} {
  const states: string[] = [];
  const events: RemoteTimelineEvent[] = [];
  const errors: Error[] = [];
  return {
    states,
    events,
    errors,
    onConnectionStateChange: (state) => states.push(state),
    onEvent: (event) => events.push(event),
    onMetricsUpdate: vi.fn(),
    onHealthReport: vi.fn(),
    onError: (error) => errors.push(error),
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('probeEndpointHealth', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('reports reachable when healthz returns 200', async () => {
    fetchMock.mockResolvedValue(new Response('ok', { status: 200 }));
    const result = await probeEndpointHealth('https://runner.test');
    expect(result.reachable).toBe(true);
    expect(result.authValid).toBe(true);
    expect(result.latencyMs).toBeGreaterThanOrEqual(0);
  });

  it('reports reachable but auth invalid on 401', async () => {
    fetchMock.mockResolvedValue(new Response('unauthorized', { status: 401 }));
    const result = await probeEndpointHealth('https://runner.test');
    expect(result.reachable).toBe(true);
    expect(result.authValid).toBe(false);
  });

  it('reports unreachable on network error', async () => {
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    const result = await probeEndpointHealth('https://runner.test');
    expect(result.reachable).toBe(false);
    expect(result.authValid).toBe(false);
    expect(result.latencyMs).toBeNull();
  });
});

describe('UnifiedTransport', () => {
  afterEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('connects using server_relay strategy', async () => {
    const cb = makeCallbacks();
    const transport = new UnifiedTransport(makeConfig(), cb);
    await transport.connect(0);

    expect(cb.states).toContain('connecting');
    expect(cb.states).toContain('open');
    expect(transport.state).toBe('open');
    expect(transport.strategy).toBe('server_relay');

    transport.close();
    expect(transport.state).toBe('idle');
  });

  it('connects using direct_websocket strategy with runnerBaseUrl', async () => {
    const cb = makeCallbacks();
    const config = makeConfig({
      strategy: 'direct_websocket',
      runnerBaseUrl: 'https://runner.test',
      allowDirectRunner: true,
    });
    const transport = new UnifiedTransport(config, cb);
    await transport.connect(0);

    expect(transport.state).toBe('open');
    expect(transport.strategy).toBe('direct_websocket');
    transport.close();
  });

  it('falls back to relay when direct_websocket is not explicitly allowed', async () => {
    const cb = makeCallbacks();
    const config = makeConfig({
      strategy: 'direct_websocket',
      runnerBaseUrl: 'https://runner.test',
    });
    const transport = new UnifiedTransport(config, cb);
    await transport.connect(0);

    expect(transport.state).toBe('open');
    expect(transport.strategy).toBe('server_relay');
    transport.close();
  });

  it('queues commands when disconnected', async () => {
    const cb = makeCallbacks();
    const transport = new UnifiedTransport(makeConfig(), cb);

    await transport.sendCommand({ kind: 'interrupt' });

    expect(mockOfflineQueue.enqueueCommand).toHaveBeenCalledWith(
      'session-1',
      { kind: 'interrupt' },
    );
    transport.close();
  });

  it('sets state to error on connect failure', async () => {
    const { subscribeToRemoteSessionEvents } = await import('./transport');
    (subscribeToRemoteSessionEvents as ReturnType<typeof vi.fn>).mockImplementationOnce(() => {
      throw new Error('WS failed');
    });

    const cb = makeCallbacks();
    const transport = new UnifiedTransport(makeConfig(), cb);
    await transport.connect(0);

    expect(transport.state).toBe('error');
    expect(cb.errors).toHaveLength(1);
    expect(cb.errors[0].message).toBe('WS failed');
  });

  it('close clears all timers', async () => {
    vi.useFakeTimers();
    const cb = makeCallbacks();
    const config = makeConfig({ strategy: 'outbound_polling', pollIntervalMs: 1000 });
    mockApi.listSessionEvents.mockResolvedValue({ items: [] });

    const transport = new UnifiedTransport(config, cb);
    await transport.connect(0);

    expect(transport.state).toBe('open');

    transport.close();
    expect(transport.state).toBe('idle');

    // Advance timers — no further polling should happen
    await vi.advanceTimersByTimeAsync(5000);
    // listSessionEvents called once during connect, not again after close
    expect(mockApi.listSessionEvents.mock.calls.length).toBeLessThanOrEqual(1);
  });

  it('outbound_polling strategy polls for events', async () => {
    vi.useFakeTimers();
    const cb = makeCallbacks();
    const config = makeConfig({ strategy: 'outbound_polling', pollIntervalMs: 1000 });

    const event1: RemoteTimelineEvent = {
      sequence: 5,
      recorded_at: '2026-04-14T09:00:00Z',
      runner_id: 'runner-1',
      session_id: 'session-1',
      detail: { kind: 'runtime_error', message: 'test' },
    };
    const event2: RemoteTimelineEvent = {
      sequence: 6,
      recorded_at: '2026-04-14T09:00:01Z',
      runner_id: 'runner-1',
      session_id: 'session-1',
      detail: { kind: 'runtime_error', message: 'test2' },
    };

    mockApi.listSessionEvents
      .mockResolvedValueOnce({ items: [event1] })
      .mockResolvedValueOnce({ items: [event2] });

    const transport = new UnifiedTransport(config, cb);
    await transport.connect(4);

    // First poll during connect
    expect(cb.events).toHaveLength(1);
    expect(cb.events[0].sequence).toBe(5);
    expect(transport.getLatestSequence()).toBe(5);

    // Advance to trigger second poll
    await vi.advanceTimersByTimeAsync(1000);
    expect(cb.events).toHaveLength(2);
    expect(cb.events[1].sequence).toBe(6);
    expect(transport.getLatestSequence()).toBe(6);

    transport.close();
  });

  it('hybrid strategy probes health then connects to best endpoint', async () => {
    const cb = makeCallbacks();
    const config = makeConfig({
      strategy: 'hybrid',
      runnerBaseUrl: 'https://runner.test',
      allowDirectRunner: true,
    });

    // Mock fetch for health probe — runner reachable
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    fetchMock.mockResolvedValue(new Response('ok', { status: 200 }));

    const transport = new UnifiedTransport(config, cb);
    await transport.connect(0);

    // Should have probed runner health and chosen direct_websocket
    expect(transport.strategy).toBe('direct_websocket');
    expect(transport.state).toBe('open');

    transport.close();
    vi.unstubAllGlobals();
  });

  it('hybrid falls back to relay when runner unreachable', async () => {
    const cb = makeCallbacks();
    const config = makeConfig({
      strategy: 'hybrid',
      runnerBaseUrl: 'https://runner.test',
      allowDirectRunner: true,
    });

    // Runner unreachable, control plane reachable
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    fetchMock
      .mockRejectedValueOnce(new TypeError('fail')) // runner probe
      .mockResolvedValueOnce(new Response('ok', { status: 200 })); // cp probe

    const transport = new UnifiedTransport(config, cb);
    await transport.connect(0);

    expect(transport.strategy).toBe('server_relay');
    expect(transport.state).toBe('open');

    transport.close();
    vi.unstubAllGlobals();
  });

  it('metrics track events received', async () => {
    const { subscribeToRemoteSessionEvents } = await import('./transport');
    let onEventCb: ((event: RemoteTimelineEvent) => void) | null = null;
    (subscribeToRemoteSessionEvents as ReturnType<typeof vi.fn>).mockImplementationOnce(
      (input: { onEvent: (e: RemoteTimelineEvent) => void }) => {
        onEventCb = input.onEvent;
        return { close: mockTransport.close };
      },
    );

    const cb = makeCallbacks();
    const transport = new UnifiedTransport(makeConfig(), cb);
    await transport.connect(0);

    // Simulate events arriving
    onEventCb!({
      sequence: 1,
      recorded_at: '2026-04-14T09:00:00Z',
      runner_id: 'r1',
      session_id: 'session-1',
      detail: { kind: 'runtime_error', message: 'test' },
    });
    onEventCb!({
      sequence: 2,
      recorded_at: '2026-04-14T09:00:01Z',
      runner_id: 'r1',
      session_id: 'session-1',
      detail: { kind: 'runtime_error', message: 'test2' },
    });

    expect(transport.metrics.eventsReceived).toBe(2);
    expect(transport.getLatestSequence()).toBe(2);

    transport.close();
  });

  it('quic strategy falls back to relay when Tauri unavailable', async () => {
    const cb = makeCallbacks();
    mockRuntime.hasTauriRuntime.mockReturnValue(false);
    const config = makeConfig({ strategy: 'quic' });

    const transport = new UnifiedTransport(config, cb);
    await transport.connect(0);

    // Should fall back to server_relay
    expect(transport.strategy).toBe('server_relay');
    expect(transport.state).toBe('open');

    transport.close();
  });

  it('sendCommand increments commandsSent metric', async () => {
    const cb = makeCallbacks();
    const transport = new UnifiedTransport(makeConfig(), cb);
    await transport.connect(0);

    mockApi.requestJson.mockResolvedValue({});
    mockOfflineQueue.drainCommands.mockResolvedValue([]);

    await transport.sendCommand({ kind: 'interrupt' });

    expect(transport.metrics.commandsSent).toBe(1);
    transport.close();
  });
});
