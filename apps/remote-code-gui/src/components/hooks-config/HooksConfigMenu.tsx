/**
 * HooksConfigMenu — Hooks 配置菜单列表组件。
 *
 * 显示所有 hooks 配置项，支持启用/禁用、编辑和新增。
 */

import { Zap, ToggleLeft, ToggleRight, Pencil, Plus } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface HookConfig {
  id: string;
  event: string;
  matcher: string;
  command: string;
  enabled: boolean;
}

export interface HooksConfigMenuProps {
  hooks: HookConfig[];
  onToggle: (id: string) => void;
  onEdit: (id: string) => void;
  onAdd: () => void;
  className?: string;
}

export function HooksConfigMenu({
  hooks,
  onToggle,
  onEdit,
  onAdd,
  className,
}: HooksConfigMenuProps) {
  return (
    <div
      data-testid="hooks-config-menu"
      className={cn('rounded-xl border border-slate-200 bg-white', className)}
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3">
        <div className="flex items-center gap-2">
          <Zap className="h-4 w-4 text-amber-500" />
          <h3 className="text-sm font-semibold text-slate-900">Hooks 配置</h3>
        </div>
        <button
          onClick={onAdd}
          className="flex items-center gap-1 rounded-lg bg-blue-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-blue-700"
          data-testid="hooks-add-btn"
        >
          <Plus className="h-3 w-3" />
          添加
        </button>
      </div>

      {/* Hook list */}
      <div className="divide-y divide-slate-100">
        {hooks.length === 0 ? (
          <p className="px-4 py-8 text-center text-sm text-slate-400">
            暂无 Hook 配置
          </p>
        ) : (
          hooks.map((hook) => (
            <div
              key={hook.id}
              data-testid={`hook-item-${hook.id}`}
              className="flex items-center gap-3 px-4 py-3 transition-colors hover:bg-slate-50"
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="rounded bg-slate-100 px-1.5 py-0.5 text-xs font-medium text-slate-600">
                    {hook.event}
                  </span>
                  <span className="truncate text-sm text-slate-900">
                    {hook.matcher}
                  </span>
                </div>
                <p className="mt-0.5 truncate text-xs text-slate-400 font-mono">
                  {hook.command}
                </p>
              </div>

              <div className="flex items-center gap-1">
                <button
                  onClick={() => onToggle(hook.id)}
                  className="rounded p-1 text-slate-400 hover:text-slate-600"
                  data-testid={`hook-toggle-${hook.id}`}
                  aria-label={hook.enabled ? '禁用' : '启用'}
                >
                  {hook.enabled ? (
                    <ToggleRight className="h-5 w-5 text-green-500" />
                  ) : (
                    <ToggleLeft className="h-5 w-5" />
                  )}
                </button>
                <button
                  onClick={() => onEdit(hook.id)}
                  className="rounded p-1 text-slate-400 hover:text-slate-600"
                  data-testid={`hook-edit-${hook.id}`}
                  aria-label="编辑"
                >
                  <Pencil className="h-4 w-4" />
                </button>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
