import { describe, it, expect, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import { useMcpOAuth } from './useMcpOAuth';
import type { McpServerInfo } from '../../lib/types';

const mockServer: McpServerInfo = {
  name: 'oauth-server',
  enabled: true,
  transport: 'http',
  config_path: '/path/to/config',
  command: null,
  url: 'http://localhost:8080',
  args: [],
  cwd: null,
  env_keys: [],
  metadata_keys: [],
  startup_timeout_secs: null,
  request_timeout_secs: null,
  live: null,
};

describe('useMcpOAuth', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns initial state', () => {
    const { result } = renderHook(() => useMcpOAuth());

    expect(result.current.oauthStatus).toEqual({});
    expect(result.current.isLoggingIn('test')).toBe(false);
  });

  it('getStatus returns default for unknown server', () => {
    const { result } = renderHook(() => useMcpOAuth());

    const status = result.current.getStatus('unknown-server');

    expect(status).toEqual({
      isAuthenticated: false,
      needsLogin: false,
      isChecking: false,
    });
  });

  it('checkOAuthStatus updates state', async () => {
    const { result } = renderHook(() => useMcpOAuth());

    await act(async () => {
      await result.current.checkOAuthStatus(mockServer);
    });

    const status = result.current.getStatus('oauth-server');
    expect(status.isChecking).toBe(false);
    expect(status.error).toBeDefined();
  });

  it('login returns not-implemented result', async () => {
    const { result } = renderHook(() => useMcpOAuth());

    let loginResult: unknown;
    await act(async () => {
      loginResult = await result.current.login(mockServer);
    });

    expect(loginResult).toEqual({
      success: false,
      error: 'OAuth login not yet implemented in backend',
    });
  });

  it('tracks logging in state', async () => {
    const { result } = renderHook(() => useMcpOAuth());

    act(() => {
      result.current.login(mockServer);
    });

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    expect(result.current.isLoggingIn('oauth-server')).toBe(false);
  });

  it('skips non-http/sse servers', async () => {
    const stdioServer: McpServerInfo = {
      ...mockServer,
      transport: 'stdio',
      command: 'test',
    };

    const { result } = renderHook(() => useMcpOAuth());

    await act(async () => {
      await result.current.checkOAuthStatus(stdioServer);
    });

    // Should not have any status for stdio server
    expect(result.current.oauthStatus).toEqual({});
  });
});
