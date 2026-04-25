/**
 * MCP OAuth 管理 Hook — 处理 MCP 服务器的 OAuth 认证状态检查和登录流程
 * MCP OAuth management hook — handles OAuth auth status checks and login flow
 *
 * Adapted from AionUi useMcpOAuth pattern, using our Tauri API layer.
 */

import { useState, useCallback } from 'react';
import type { McpServerInfo } from '../../lib/types';

export interface McpOAuthStatus {
  isAuthenticated: boolean;
  needsLogin: boolean;
  isChecking: boolean;
  error?: string;
}

/**
 * MCP OAuth 管理 Hook
 *
 * @example
 * ```tsx
 * const { oauthStatus, checkOAuthStatus, login } = useMcpOAuth();
 * ```
 */
export function useMcpOAuth() {
  const [oauthStatus, setOAuthStatus] = useState<Record<string, McpOAuthStatus>>({});
  const [loggingIn, setLoggingIn] = useState<Record<string, boolean>>({});

  /** 检查 OAuth 状态 */
  const checkOAuthStatus = useCallback(async (server: McpServerInfo) => {
    // 只检查 HTTP/SSE 类型的服务器
    // 只检查 HTTP/SSE 类型的服务器
    if (server.transport !== 'http' && server.transport !== 'sse') return;

    setOAuthStatus((prev) => ({
      ...prev,
      [server.name]: {
        isAuthenticated: false,
        needsLogin: false,
        isChecking: true,
      },
    }));

    try {
      // 通过 Tauri 后端检查 OAuth 状态
      // 这里是预留接口，实际需要后端支持
      setOAuthStatus((prev) => ({
        ...prev,
        [server.name]: {
          isAuthenticated: false,
          needsLogin: false,
          isChecking: false,
          error: 'OAuth check not yet implemented in backend',
        },
      }));
    } catch (error) {
      setOAuthStatus((prev) => ({
        ...prev,
        [server.name]: {
          isAuthenticated: false,
          needsLogin: false,
          isChecking: false,
          error: error instanceof Error ? error.message : 'Unknown error',
        },
      }));
    }
  }, []);

  /** 执行 OAuth 登录 */
  const login = useCallback(
    async (server: McpServerInfo): Promise<{ success: boolean; error?: string }> => {
      setLoggingIn((prev) => ({ ...prev, [server.name]: true }));

      try {
        // 通过 Tauri 后端执行 OAuth 登录
        // 这里是预留接口，实际需要后端支持
        setOAuthStatus((prev) => ({
          ...prev,
          [server.name]: {
            isAuthenticated: false,
            needsLogin: true,
            isChecking: false,
            error: 'OAuth login not yet implemented in backend',
          },
        }));

        return { success: false, error: 'OAuth login not yet implemented in backend' };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : 'Login failed',
        };
      } finally {
        setLoggingIn((prev) => {
          const next = { ...prev };
          delete next[server.name];
          return next;
        });
      }
    },
    [],
  );

  /** 获取指定服务器的 OAuth 状态 */
  const getStatus = useCallback(
    (serverName: string): McpOAuthStatus =>
      oauthStatus[serverName] || {
        isAuthenticated: false,
        needsLogin: false,
        isChecking: false,
      },
    [oauthStatus],
  );

  /** 检查服务器是否正在登录 */
  const isLoggingIn = useCallback(
    (serverName: string) => !!loggingIn[serverName],
    [loggingIn],
  );

  return {
    oauthStatus,
    checkOAuthStatus,
    login,
    getStatus,
    isLoggingIn,
  };
}
