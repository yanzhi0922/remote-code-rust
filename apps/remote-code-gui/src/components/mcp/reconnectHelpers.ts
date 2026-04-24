/**
 * MCP 重连辅助函数
 */

export interface ReconnectAttempt {
  serverName: string;
  attempt: number;
  maxAttempts: number;
  lastError: string | null;
  timestamp: number;
}

export type ReconnectStatus = 'idle' | 'pending' | 'success' | 'failed';

/**
 * 计算指数退避延迟（毫秒）
 */
export function getBackoffDelay(attempt: number, baseDelay = 1000): number {
  return Math.min(baseDelay * Math.pow(2, attempt), 30000);
}

/**
 * 判断是否可以重试
 */
export function canRetry(attempt: number, maxAttempts: number): boolean {
  return attempt < maxAttempts;
}

/**
 * 创建初始重连状态
 */
export function createInitialReconnectState(serverName: string, maxAttempts = 3): ReconnectAttempt {
  return {
    serverName,
    attempt: 0,
    maxAttempts,
    lastError: null,
    timestamp: Date.now(),
  };
}

/**
 * 推进重连尝试
 */
export function advanceReconnect(state: ReconnectAttempt, error?: string): ReconnectAttempt {
  return {
    ...state,
    attempt: state.attempt + 1,
    lastError: error ?? state.lastError,
    timestamp: Date.now(),
  };
}

/**
 * 从重连状态获取状态标签
 */
export function getReconnectStatusLabel(status: ReconnectStatus): string {
  switch (status) {
    case 'idle':
      return '空闲';
    case 'pending':
      return '重连中';
    case 'success':
      return '已连接';
    case 'failed':
      return '重连失败';
  }
}

/**
 * 格式化重连进度文本
 */
export function formatReconnectProgress(state: ReconnectAttempt): string {
  return `${state.serverName} (${state.attempt}/${state.maxAttempts})`;
}

/**
 * 判断错误是否为可重试的连接错误
 */
export function isRetryableError(error: string): boolean {
  const retryablePatterns = [
    /ECONNREFUSED/i,
    /ECONNRESET/i,
    /ETIMEDOUT/i,
    /timeout/i,
    /network/i,
    /socket hang up/i,
    /fetch failed/i,
  ];
  return retryablePatterns.some((p) => p.test(error));
}
