/**
 * MCP 连接测试管理 Hook — 处理 MCP 服务器的连接测试和状态更新
 * MCP connection test management hook — handles MCP server connection testing and status updates
 *
 * Adapted from AionUi useMcpConnection pattern, using our Tauri API layer.
 */

import { useState, useCallback } from 'react';
import type { McpServerInfo, ConfigScope } from '../../lib/types';
import * as tauri from '../../lib/tauri';

/** 截断过长的错误消息 */
function truncateErrorMessage(message: string, maxLength = 150): string {
  if (message.length <= maxLength) return message;
  return message.substring(0, maxLength) + '...';
}

export interface McpConnectionResult {
  success: boolean;
  error?: string;
  tools?: Array<{ name: string; description: string | null }>;
}

export interface UseMcpConnectionOptions {
  onConnectionTested?: (serverName: string, result: McpConnectionResult) => void;
}

/**
 * MCP 连接测试 Hook
 *
 * @example
 * ```tsx
 * const { testingServers, testConnection } = useMcpConnection({
 *   onConnectionTested: (name, result) => console.log(name, result),
 * });
 * ```
 */
export function useMcpConnection(options: UseMcpConnectionOptions = {}) {
  const { onConnectionTested } = options;
  const [testingServers, setTestingServers] = useState<Record<string, boolean>>({});

  const testConnection = useCallback(
    async (server: McpServerInfo, scope: ConfigScope = 'project', projectPath: string | null = null) => {
      setTestingServers((prev) => ({ ...prev, [server.name]: true }));

      try {
        const serverList = await tauri.listMcpServers(scope, projectPath, true, true);
        const found = serverList.servers.find((s) => s.name === server.name);

        const result: McpConnectionResult = {
          success: !!found && found.live !== null && found.live.status === 'connected',
          tools: found?.live?.tools?.map((t) => ({ name: t.name, description: t.description })),
          error: !found
            ? 'Server not found'
            : !found.live
              ? 'Server not connected'
              : found.live.status !== 'connected'
                ? `Status: ${found.live.status}`
                : undefined,
        };

        onConnectionTested?.(server.name, result);
        return result;
      } catch (error) {
        const result: McpConnectionResult = {
          success: false,
          error: truncateErrorMessage(error instanceof Error ? error.message : 'Unknown error'),
        };
        onConnectionTested?.(server.name, result);
        return result;
      } finally {
        setTestingServers((prev) => {
          const next = { ...prev };
          delete next[server.name];
          return next;
        });
      }
    },
    [onConnectionTested],
  );

  const isTesting = useCallback(
    (serverName: string) => !!testingServers[serverName],
    [testingServers],
  );

  return {
    testingServers,
    testConnection,
    isTesting,
  };
}
