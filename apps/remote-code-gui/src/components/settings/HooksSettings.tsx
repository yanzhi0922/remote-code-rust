import { Plus, Trash2 } from 'lucide-react';
import { ToggleSwitch } from './ToggleSwitch';

export interface HookConfig {
  event: string;
  command: string;
  enabled: boolean;
}

export interface HooksSettingsProps {
  hooks: HookConfig[];
  onUpdate: (hooks: HookConfig[]) => void;
}

const HOOK_EVENTS = [
  { value: 'PreToolUse', label: 'PreToolUse' },
  { value: 'PostToolUse', label: 'PostToolUse' },
  { value: 'Notification', label: 'Notification' },
  { value: 'Stop', label: 'Stop' },
];

export function HooksSettings({ hooks, onUpdate }: HooksSettingsProps) {
  function addHook() {
    onUpdate([...hooks, { event: 'PreToolUse', command: '', enabled: true }]);
  }

  function removeHook(index: number) {
    const next = hooks.filter((_, i) => i !== index);
    onUpdate(next);
  }

  function updateHook(index: number, patch: Partial<HookConfig>) {
    const next = hooks.map((hook, i) => (i === index ? { ...hook, ...patch } : hook));
    onUpdate(next);
  }

  return (
    <div className="space-y-6" data-testid="hooks-settings">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-slate-800">Hooks 配置</h3>
        <button
          type="button"
          onClick={addHook}
          className="flex items-center gap-1 rounded-xl bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-700"
          data-testid="add-hook-btn"
        >
          <Plus size={14} />
          添加 Hook
        </button>
      </div>

      {hooks.length === 0 ? (
        <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-slate-300 py-8 text-slate-400">
          <p className="text-sm">暂无 Hooks 配置</p>
        </div>
      ) : (
        <div className="space-y-3">
          {hooks.map((hook, index) => (
            <div
              key={index}
              className="flex items-start gap-3 rounded-xl border border-slate-200 bg-white p-3"
              data-testid={`hook-row-${index}`}
            >
              <select
                value={hook.event}
                onChange={(e) => updateHook(index, { event: e.target.value })}
                className="shrink-0 rounded-lg border border-slate-300 bg-white px-2 py-1.5 text-sm text-slate-800 focus:border-blue-500 focus:outline-none"
                aria-label="Hook 事件类型"
              >
                {HOOK_EVENTS.map((evt) => (
                  <option key={evt.value} value={evt.value}>
                    {evt.label}
                  </option>
                ))}
              </select>

              <input
                type="text"
                value={hook.command}
                onChange={(e) => updateHook(index, { command: e.target.value })}
                placeholder="输入命令"
                className="flex-1 rounded-lg border border-slate-300 bg-white px-2 py-1.5 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none"
                aria-label="Hook 命令"
              />

              <ToggleSwitch
                checked={hook.enabled}
                onChange={(enabled) => updateHook(index, { enabled })}
                label="启用"
              />

              <button
                type="button"
                onClick={() => removeHook(index)}
                className="shrink-0 rounded-lg p-1.5 text-slate-400 hover:bg-red-50 hover:text-red-500"
                aria-label="删除 Hook"
                data-testid={`remove-hook-${index}`}
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
