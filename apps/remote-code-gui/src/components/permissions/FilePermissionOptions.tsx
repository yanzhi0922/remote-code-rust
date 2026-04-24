import { useState } from 'react';
import { ShieldCheck, ShieldX, ShieldAlert } from 'lucide-react';
import type { PermissionBehavior } from './PermissionRuleDescription';

type PermissionScope = 'session' | 'project' | 'user';

export interface FilePermissionOptionsProps {
  filePath: string;
  currentBehavior: PermissionBehavior;
  onSelect: (behavior: PermissionBehavior, scope: PermissionScope) => void;
}

const BEHAVIOR_OPTIONS: {
  value: PermissionBehavior;
  label: string;
  icon: typeof ShieldCheck;
  color: string;
}[] = [
  { value: 'allow', label: '允许', icon: ShieldCheck, color: 'text-emerald-600 border-emerald-300 bg-emerald-50 hover:bg-emerald-100' },
  { value: 'deny', label: '拒绝', icon: ShieldX, color: 'text-red-600 border-red-300 bg-red-50 hover:bg-red-100' },
  { value: 'ask', label: '每次询问', icon: ShieldAlert, color: 'text-amber-600 border-amber-300 bg-amber-50 hover:bg-amber-100' },
];

const SCOPE_OPTIONS: { value: PermissionScope; label: string; description: string }[] = [
  { value: 'session', label: '本次会话', description: '仅在当前会话中生效' },
  { value: 'project', label: '项目级', description: '在此项目目录中永久生效' },
  { value: 'user', label: '用户级', description: '在所有项目中永久生效' },
];

export function FilePermissionOptions({
  filePath,
  currentBehavior,
  onSelect,
}: FilePermissionOptionsProps) {
  const [selectedBehavior, setSelectedBehavior] = useState<PermissionBehavior>(currentBehavior);
  const [selectedScope, setSelectedScope] = useState<PermissionScope>('session');

  function handleBehaviorSelect(behavior: PermissionBehavior) {
    setSelectedBehavior(behavior);
    onSelect(behavior, selectedScope);
  }

  function handleScopeChange(scope: PermissionScope) {
    setSelectedScope(scope);
    onSelect(selectedBehavior, scope);
  }

  const previewRule = `${selectedBehavior} ${filePath}${selectedBehavior === 'allow' ? ' (all operations)' : ''}`;

  return (
    <div className="rounded-xl border border-slate-200 bg-white p-4" data-testid="file-permission-options">
      {/* File path */}
      <div className="mb-3 font-mono text-sm text-slate-700 bg-slate-50 rounded-lg px-3 py-2">
        {filePath}
      </div>

      {/* Behavior buttons */}
      <div className="mb-3 flex gap-2">
        {BEHAVIOR_OPTIONS.map((opt) => {
          const Icon = opt.icon;
          const isActive = selectedBehavior === opt.value;
          return (
            <button
              key={opt.value}
              type="button"
              onClick={() => handleBehaviorSelect(opt.value)}
              className={`flex items-center gap-1.5 rounded-xl border px-3 py-2 text-sm font-medium transition-colors ${
                isActive ? opt.color : 'border-slate-200 text-slate-500 hover:bg-slate-50'
              }`}
            >
              <Icon size={14} />
              {opt.label}
            </button>
          );
        })}
      </div>

      {/* Scope selection */}
      <div className="mb-3">
        <div className="mb-1.5 text-xs font-medium text-slate-500">作用域</div>
        <div className="flex flex-col gap-1">
          {SCOPE_OPTIONS.map((opt) => (
            <label
              key={opt.value}
              className="flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-slate-50"
            >
              <input
                type="radio"
                name="permission-scope"
                value={opt.value}
                checked={selectedScope === opt.value}
                onChange={() => handleScopeChange(opt.value)}
                className="accent-blue-600"
              />
              <div>
                <span className="text-sm font-medium text-slate-700">{opt.label}</span>
                <span className="ml-2 text-xs text-slate-400">{opt.description}</span>
              </div>
            </label>
          ))}
        </div>
      </div>

      {/* Preview */}
      <div className="rounded-lg bg-slate-50 px-3 py-2 font-mono text-xs text-slate-500">
        预览规则: {previewRule}
      </div>
    </div>
  );
}
