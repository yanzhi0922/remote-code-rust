import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import { useMcpConnection } from './useMcpConnection';
import type { McpServerInfo } from '../../lib/types';

// Mock tauri module
vi.mock('../../lib/tauri', () => ({
  listMcpServers: vi.fn(),
}));

import * as tauri from '../../lib/tauri';

const mockServer: McpServerInfo = {
  name: 'test-server',
  enabled: true,
  transport: 'stdio',
  config_path: '/path/to/config',
  command: 'test',
  url: null,
  args: [],
  cwd: null,
  env_keys: [],
  metadata_keys: [],
  startup_timeout_secs: null,
  request_timeout_secs: null,
  live: {
    status: 'connected',
    protocol_version: '1.0',
    peer_name: 'test',
    peer_version: '1.0',
    tool_count: 1,
    tools: [{ name: 'tool1', description: 'Test tool' }],
    error: null,
  },
};

describe('useMcpConnection', () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('returns initial state', () => {
    const { result } = renderHook(() => useMcpConnection());

    expect(result.current.testingServers).toEqual({});
    expect(result.current.isTesting('test')).toBe(false);
  });

  it('tests connection successfully', async () => {
    vi.mocked(tauri.listMcpServers).mockResolvedValue({
      scope: 'project',
      config_path: '/path',
      warnings: [],
      servers: [mockServer],
    });

    const onConnectionTested = vi.fn();
    const { result } = renderHook(() =>
      useMcpConnection({ onConnectionTested }),
    );

    let testResult: unknown;
    await act(async () => {
      testResult = await result.current.testConnection(mockServer);
    });

    expect(testResult).toEqual({
      success: true,
      tools: [{ name: 'tool1', description: 'Test tool' }],
    });
    expect(onConnectionTested).toHaveBeenCalledWith('test-server', expect.objectContaining({ success: true }));
  });

  it('handles server not found', async () => {
    vi.mocked(tauri.listMcpServers).mockResolvedValue({
      scope: 'project',
      config_path: '/path',
      warnings: [],
      servers: [],
    });

    const { result } = renderHook(() => useMcpConnection());

    let testResult: unknown;
    await act(async () => {
      testResult = await result.current.testConnection(mockServer);
    });

    expect(testResult).toEqual({
      success: false,
      error: 'Server not found',
    });
  });

  it('handles connection error', async () => {
    vi.mocked(tauri.listMcpServers).mockRejectedValue(new Error('Network error'));

    const { result } = renderHook(() => useMcpConnection());

    let testResult: unknown;
    await act(async () => {
      testResult = await result.current.testConnection(mockServer);
    });

    expect(testResult).toEqual({
      success: false,
      error: 'Network error',
    });
  });

  it('tracks testing state', async () => {
    vi.mocked(tauri.listMcpServers).mockImplementation(
      () => new Promise((resolve) => setTimeout(() => resolve({
        scope: 'project',
        config_path: '/path',
        warnings: [],
        servers: [],
      }), 100)),
    );

    const { result } = renderHook(() => useMcpConnection());

    act(() => {
      result.current.testConnection(mockServer);
    });

    expect(result.current.isTesting('test-server')).toBe(true);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 150));
    });

    expect(result.current.isTesting('test-server')).toBe(false);
  });

  it('truncates long error messages', async () => {
    const longError = 'A'.repeat(300);
    vi.mocked(tauri.listMcpServers).mockRejectedValue(new Error(longError));

    const { result } = renderHook(() => useMcpConnection());

    let testResult: unknown;
    await act(async () => {
      testResult = await result.current.testConnection(mockServer);
    });

    expect((testResult as { error: string }).error.length).toBeLessThan(200);
  });

  it('handles server with null live info', async () => {
    const serverNoLive = { ...mockServer, live: null };
    vi.mocked(tauri.listMcpServers).mockResolvedValue({
      scope: 'project',
      config_path: '/path',
      warnings: [],
      servers: [serverNoLive],
    });

    const { result } = renderHook(() => useMcpConnection());

    let testResult: unknown;
    await act(async () => {
      testResult = await result.current.testConnection(serverNoLive);
    });

    expect(testResult).toEqual({
      success: false,
      error: 'Server not connected',
    });
  });
});
