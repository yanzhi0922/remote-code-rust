/**
 * TaskStatusUtils — 任务状态工具函数。
 *
 * 提供状态颜色、标签、图标名和持续时间格式化。
 */

export function getTaskStatusColor(status: string): string {
  switch (status) {
    case 'running':
      return 'text-green-500';
    case 'completed':
      return 'text-green-600';
    case 'failed':
      return 'text-red-500';
    case 'pending':
      return 'text-slate-400';
    default:
      return 'text-slate-400';
  }
}

export function getTaskStatusLabel(status: string): string {
  switch (status) {
    case 'running':
      return 'Running';
    case 'completed':
      return 'Completed';
    case 'failed':
      return 'Failed';
    case 'pending':
      return 'Pending';
    default:
      return status;
  }
}

export function getTaskStatusIcon(status: string): string {
  switch (status) {
    case 'running':
      return 'Loader2';
    case 'completed':
      return 'CheckCircle2';
    case 'failed':
      return 'XCircle';
    case 'pending':
      return 'Clock';
    default:
      return 'Clock';
  }
}

export function formatTaskDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}
