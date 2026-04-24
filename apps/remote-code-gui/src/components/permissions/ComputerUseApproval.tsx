import { Monitor } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function stringField(record: Record<string, unknown> | null, ...keys: string[]): string | null {
  if (!record) return null;
  for (const key of keys) {
    const val = record[key];
    if (typeof val === 'string' && val.trim()) return val;
  }
  return null;
}

const ACTION_LABELS: Record<string, string> = {
  screenshot: '截屏',
  click: '点击',
  type: '输入',
  scroll: '滚动',
  drag: '拖拽',
  key_press: '按键',
  move: '移动鼠标',
};

function getActionLabel(action: string): string {
  return ACTION_LABELS[action.toLowerCase()] ?? action;
}

function getActionColor(action: string): string {
  switch (action.toLowerCase()) {
    case 'screenshot':
      return 'bg-blue-50 text-blue-700';
    case 'click':
    case 'type':
    case 'key_press':
      return 'bg-amber-50 text-amber-700';
    case 'scroll':
    case 'drag':
    case 'move':
      return 'bg-slate-50 text-slate-700';
    default:
      return 'bg-slate-50 text-slate-700';
  }
}

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface ComputerUseApprovalProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function ComputerUseApproval({
  request,
  onAllow,
  onReject,
  className,
}: ComputerUseApprovalProps) {
  const record = asRecord(request.input);
  const action = stringField(record, 'action', 'operation', 'type') ?? '';

  return (
    <div
      className={cn('rounded-2xl border border-purple-200 bg-white p-4', className)}
      data-testid="computer-use-approval"
    >
      <PermissionRequestTitle
        title={request.title || 'Computer Use'}
        subtitle={request.description}
      />

      <div className="my-3 flex items-center gap-2">
        <Monitor size={16} className="shrink-0 text-purple-400" />
        <span className="text-sm text-slate-500">操作类型:</span>
        {action ? (
          <span
            className={cn(
              'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium',
              getActionColor(action),
            )}
          >
            {getActionLabel(action)}
          </span>
        ) : (
          <span className="text-xs text-slate-400">未知操作</span>
        )}
      </div>

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={onAllow}
          className="rounded-2xl bg-purple-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-purple-700"
        >
          允许执行
        </button>
        <button
          type="button"
          onClick={() => onReject()}
          className="rounded-2xl border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-50"
        >
          拒绝
        </button>
      </div>
    </div>
  );
}
