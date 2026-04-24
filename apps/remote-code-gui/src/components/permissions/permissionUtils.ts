import type { PermissionRequestInfo } from '../../lib/types';

/**
 * 获取权限请求的工具类别。
 */
export function getToolCategory(toolName: string): string {
  const lower = toolName.toLowerCase();
  if (lower.includes('bash') || lower.includes('shell')) return 'shell';
  if (lower.includes('file') || lower.includes('edit') || lower.includes('write')) return 'filesystem';
  if (lower.includes('notebook')) return 'notebook';
  if (lower.includes('web') || lower.includes('fetch')) return 'network';
  if (lower.includes('mcp')) return 'mcp';
  if (lower.includes('skill')) return 'skill';
  if (lower.includes('sandbox')) return 'sandbox';
  if (lower.includes('monitor') || lower.includes('computer')) return 'monitor';
  return 'other';
}

/**
 * 获取权限类别的图标名称。
 */
export function getCategoryIcon(category: string): string {
  const icons: Record<string, string> = {
    shell: 'Terminal',
    filesystem: 'FileText',
    notebook: 'BookOpen',
    network: 'Globe',
    mcp: 'Puzzle',
    skill: 'Sparkles',
    sandbox: 'Box',
    monitor: 'Monitor',
    other: 'Shield',
  };
  return icons[category] ?? 'Shield';
}

/**
 * 从权限请求中提取文件路径。
 */
export function extractFilePath(request: PermissionRequestInfo): string | null {
  const input = request.input as Record<string, unknown> | null;
  if (!input) return null;
  const candidates = ['path', 'file_path', 'filePath', 'filename'];
  for (const key of candidates) {
    const val = input[key];
    if (typeof val === 'string' && val.trim()) return val;
  }
  return null;
}

/**
 * 格式化权限规则为显示字符串。
 */
export function formatPermissionRule(rule: string): string {
  return rule.trim().replace(/\s+/g, ' ');
}

/**
 * 检查权限请求是否为危险操作。
 */
export function isDangerousPermission(request: PermissionRequestInfo): boolean {
  const dangerousTools = ['Bash', 'PowerShell', 'Write', 'Delete', 'SedEdit'];
  return dangerousTools.some((t) => request.tool_name.toLowerCase().includes(t.toLowerCase()));
}
