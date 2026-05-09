import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useConnection } from './useConnection';
import {
  getConnectionManager,
  destroyConnectionManager,
  onConnectionManagerStateChange,
  onConnectionManagerEvent,
} from './connection-manager';
import type { TransportConfig } from './unified-transport';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockManager = {
  connect: vi.fn(() => Promise.resolve()),
  disconnect: vi.fn(),
  sendPrompt: vi.fn(() => Promise.resolve()),
  interrupt: vi.fn(() => Promise.resolve()),
  respondToApproval: vi.fn(() => Promise.resolve()),
  probeHealth: vi.fn(() =>
    Promise.resolve({
      runnerReachable: false,
      runnerLatencyMs: null,
      controlPlaneReachable: true,
      controlPlaneLatencyMs: 50,
      recommendedStrategy: 'server_relay' as const,
    }),
  ),
  getLatestSequence: vi.fn(() => 0),
  state: {
    connectionState: 'idle' as string,
    strategy: null as string | null,
    metrics: null as unknown,
    health: null as unknown,
    latestSequence: 0,
  },
};

vi.mock('./connection-manager', () => ({
  getConnectionManager: vi.fn(() => mockManager),
  destroyConnectionManager: vi.fn(),
  onConnectionManagerStateChange: vi.fn(() => () => {}),
  onConnectionManagerEvent: vi.fn(() => () => {}),
}));

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useConnection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockManager.connect.mockResolvedValue(undefined);
    mockManager.sendPrompt.mockResolvedValue(undefined);
    mockManager.interrupt.mockResolvedValue(undefined);
    mockManager.respondToApproval.mockResolvedValue(undefined);
    mockManager.state = {
      connectionState: 'idle',
      strategy: null,
      metrics: null,
      health: null,
      latestSequence: 0,
    };
  });

  it('returns initial idle state', () => {
    const { result } = renderHook(() => useConnection());
    expect(result.current.connectionState).toBe('idle');
    expect(result.current.strategy).toBeNull();
    expect(result.current.metrics).toBeNull();
    expect(result.current.latestSequence).toBe(0);
  });

  it('delegates connect to ConnectionManager', async () => {
    const { result } = renderHook(() => useConnection());
    const config = makeConfig();

    await act(async () => {
      await result.current.connect(config, 10);
    });

    expect(getConnectionManager).toHaveBeenCalled();
    expect(mockManager.connect).toHaveBeenCalledWith(config, 10);
  });

  it('delegates disconnect to ConnectionManager', () => {
    const { result } = renderHook(() => useConnection());

    act(() => {
      result.current.disconnect();
    });

    expect(mockManager.disconnect).toHaveBeenCalled();
  });

  it('delegates sendPrompt to ConnectionManager', async () => {
    const { result } = renderHook(() => useConnection());

    await act(async () => {
      await result.current.sendPrompt('hello world');
    });

    expect(mockManager.sendPrompt).toHaveBeenCalledWith('hello world');
  });

  it('delegates interrupt to ConnectionManager', async () => {
    const { result } = renderHook(() => useConnection());

    await act(async () => {
      await result.current.interrupt();
    });

    expect(mockManager.interrupt).toHaveBeenCalled();
  });

  it('delegates respondToApproval to ConnectionManager', async () => {
    const { result } = renderHook(() => useConnection());

    await act(async () => {
      await result.current.respondToApproval('approval-1', 'approved', 'looks good');
    });

    expect(mockManager.respondToApproval).toHaveBeenCalledWith('approval-1', 'approved', 'looks good');
  });

  it('delegates probeHealth to ConnectionManager', async () => {
    const { result } = renderHook(() => useConnection());

    let report: unknown;
    await act(async () => {
      report = await result.current.probeHealth();
    });

    expect(mockManager.probeHealth).toHaveBeenCalled();
    expect(report).toEqual({
      runnerReachable: false,
      runnerLatencyMs: null,
      controlPlaneReachable: true,
      controlPlaneLatencyMs: 50,
      recommendedStrategy: 'server_relay',
    });
  });

  it('registers event listener when onEvent is provided', () => {
    const onEvent = vi.fn();
    renderHook(() => useConnection(onEvent));

    expect(onConnectionManagerEvent).toHaveBeenCalledWith(onEvent);
  });

  it('does not register event listener when onEvent is omitted', () => {
    renderHook(() => useConnection());

    expect(onConnectionManagerEvent).not.toHaveBeenCalled();
  });

  it('provides stable function references across re-renders', () => {
    const { result, rerender } = renderHook(() => useConnection());

    const firstConnect = result.current.connect;
    const firstDisconnect = result.current.disconnect;
    const firstSendPrompt = result.current.sendPrompt;

    rerender();

    expect(result.current.connect).toBe(firstConnect);
    expect(result.current.disconnect).toBe(firstDisconnect);
    expect(result.current.sendPrompt).toBe(firstSendPrompt);
  });
});
