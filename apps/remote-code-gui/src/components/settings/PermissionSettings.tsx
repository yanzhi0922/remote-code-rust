import { clsx } from 'clsx';
import { Shield, ShieldAlert, ShieldCheck, ClipboardList } from 'lucide-react';
import type { FullSettings } from '../../lib/types';
import { BypassPermissions } from '../permissions/BypassPermissions';

export interface PermissionSettingsProps {
  settings: FullSettings;
  onUpdate: (updates: Record<string, unknown>) => void;
}

const PERMISSION_MODES = [
  {
    value: 'default',
    label: '默认',
    desc: '读取自动执行，写入和命令需确认',
    icon: Shield,
    color: 'text-blue-500',
  },
  {
    value: 'acceptEdits',
    label: '自动编辑',
    desc: '文件编辑自动执行，命令仍需确认',
    icon: ShieldCheck,
    color: 'text-emerald-500',
  },
  {
    value: 'dontAsk',
    label: '不询问',
    desc: '仅自动放行低风险读取工具',
    icon: ShieldAlert,
    color: 'text-amber-500',
  },
  {
    value: 'bypassPermissions',
    label: '全自动',
    desc: '跳过全部权限确认',
    icon: ShieldAlert,
    color: 'text-red-500',
  },
  {
    value: 'plan',
    label: '规划',
    desc: '只规划，不执行工具',
    icon: ClipboardList,
    color: 'text-purple-500',
  },
];

export function PermissionSettings({ settings, onUpdate }: PermissionSettingsProps) {
  const currentMode = settings.permission_mode;

  function handleBypassToggle() {
    const nextMode = currentMode === 'bypassPermissions' ? 'default' : 'bypassPermissions';
    onUpdate({ permission_mode: nextMode });
  }

  return (
    <div className="space-y-6" data-testid="permission-settings">
      <h3 className="text-lg font-semibold text-slate-800">权限设置</h3>

      <div className="space-y-2">
        {PERMISSION_MODES.map((mode) => {
          const Icon = mode.icon;
          return (
            <button
              key={mode.value}
              type="button"
              onClick={() => onUpdate({ permission_mode: mode.value })}
              className={clsx(
                'flex w-full items-start gap-3 rounded-xl border p-3 text-left transition-colors',
                currentMode === mode.value
                  ? 'border-blue-500 bg-blue-50'
                  : 'border-slate-200 bg-white hover:bg-slate-50',
              )}
              data-testid={`permission-mode-${mode.value}`}
            >
              <Icon size={20} className={clsx('mt-0.5 shrink-0', mode.color)} />
              <div>
                <div className="text-sm font-medium text-slate-800">{mode.label}</div>
                <div className="text-xs text-slate-500">{mode.desc}</div>
              </div>
            </button>
          );
        })}
      </div>

      <BypassPermissions
        enabled={currentMode === 'bypassPermissions'}
        onToggle={handleBypassToggle}
        killswitchActive={false}
      />
    </div>
  );
}
