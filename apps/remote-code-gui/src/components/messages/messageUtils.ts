import type { ConversationEntry, ConversationRole } from '../../lib/types';

/**
 * 格式化时间戳为本地时间字符串。
 */
export function formatTimestamp(timestamp: string | Date): string {
  const date = typeof timestamp === 'string' ? new Date(timestamp) : timestamp;
  return date.toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
  });
}

/**
 * 格式化持续时间（毫秒）为人类可读字符串。
 */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.floor((ms % 60_000) / 1000);
  return `${minutes}m ${seconds}s`;
}

/**
 * 截断文本到指定长度，添加省略号。
 */
export function truncateText(text: string, maxLength: number = 200): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + '…';
}

/**
 * 获取对话条目的显示角色标签。
 */
export function getRoleLabel(role: ConversationRole): string {
  const labels: Record<ConversationRole, string> = {
    system: '系统',
    user: '用户',
    assistant: '助手',
    tool: '工具',
  };
  return labels[role];
}

/**
 * 检测消息是否为错误类型。
 */
export function isErrorMessage(entry: ConversationEntry): boolean {
  return entry.is_error || entry.text.toLowerCase().includes('error');
}

/**
 * 从对话条目中提取工具名称列表。
 */
export function extractToolNames(entry: ConversationEntry): string[] {
  return entry.tool_calls.map((tc) => tc.name);
}

/**
 * 格式化 token 数量为可读字符串。
 */
export function formatTokenCount(count: number): string {
  if (count < 1000) return String(count);
  if (count < 1_000_000) return `${(count / 1000).toFixed(1)}K`;
  return `${(count / 1_000_000).toFixed(2)}M`;
}
