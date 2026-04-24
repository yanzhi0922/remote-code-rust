import { useState } from 'react';
import { AlertTriangle, X } from 'lucide-react';

type RiskTier = 'conservative' | 'moderate' | 'aggressive';

interface TierInfo {
  key: RiskTier;
  label: string;
  description: string;
  previewCommands: string[];
}

const TIERS: TierInfo[] = [
  {
    key: 'conservative',
    label: '保守',
    description: '仅允许安全的只读命令',
    previewCommands: ['ls', 'cat', 'grep', 'find', 'git status', 'git log', 'git diff'],
  },
  {
    key: 'moderate',
    label: '适中',
    description: '允许常见的开发命令',
    previewCommands: [
      'ls',
      'cat',
      'grep',
      'npm test',
      'npm run build',
      'git add',
      'git commit',
      'cargo build',
    ],
  },
  {
    key: 'aggressive',
    label: '激进',
    description: '允许几乎所有命令（不含极端危险命令）',
    previewCommands: [
      'npm install',
      'npm run *',
      'cargo *',
      'git *',
      'python *',
      'make *',
      'docker *',
    ],
  },
];

export interface AutoModeOptInProps {
  visible: boolean;
  onConfirm: (rules: string[]) => void;
  onCancel: () => void;
}

export function AutoModeOptIn({ visible, onConfirm, onCancel }: AutoModeOptInProps) {
  const [selectedTier, setSelectedTier] = useState<RiskTier>('conservative');
  const [confirmText, setConfirmText] = useState('');

  if (!visible) return null;

  const currentTier = TIERS.find((t) => t.key === selectedTier)!;
  const isConfirmed = confirmText === 'AUTO MODE';

  function handleConfirm() {
    if (!isConfirmed) return;
    onConfirm(currentTier.previewCommands);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" data-testid="auto-mode-dialog">
      <div className="w-full max-w-lg rounded-2xl border border-slate-200 bg-white shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-200 px-6 py-4">
          <div className="flex items-center gap-2">
            <AlertTriangle size={20} className="text-amber-500" />
            <h2 className="text-lg font-semibold text-slate-800">启用自动模式</h2>
          </div>
          <button
            type="button"
            onClick={onCancel}
            className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            aria-label="关闭"
          >
            <X size={18} />
          </button>
        </div>

        {/* Warning */}
        <div className="mx-6 mt-4 rounded-xl bg-amber-50 p-3 text-sm text-amber-800">
          <strong>警告：</strong>自动模式将跳过权限确认，AI 将直接执行命令。
          这可能导致不可逆的数据更改。请确保你理解相关风险。
        </div>

        {/* Tier selection */}
        <div className="px-6 py-4">
          <div className="mb-3 text-sm font-medium text-slate-700">选择风险等级</div>
          <div className="flex flex-col gap-2">
            {TIERS.map((tier) => (
              <button
                key={tier.key}
                type="button"
                onClick={() => setSelectedTier(tier.key)}
                className={`rounded-xl border p-3 text-left transition-colors ${
                  selectedTier === tier.key
                    ? 'border-blue-500 bg-blue-50'
                    : 'border-slate-200 hover:border-slate-300'
                }`}
              >
                <div className="font-medium text-slate-800">{tier.label}</div>
                <div className="text-xs text-slate-500">{tier.description}</div>
              </button>
            ))}
          </div>

          {/* Preview */}
          <div className="mt-3">
            <div className="mb-1 text-xs font-medium text-slate-500">将允许的命令：</div>
            <div className="flex flex-wrap gap-1">
              {currentTier.previewCommands.map((cmd) => (
                <span
                  key={cmd}
                  className="rounded-full bg-slate-100 px-2 py-0.5 font-mono text-xs text-slate-600"
                >
                  {cmd}
                </span>
              ))}
            </div>
          </div>
        </div>

        {/* Confirm input */}
        <div className="border-t border-slate-200 px-6 py-4">
          <label className="mb-1 block text-sm text-slate-600" htmlFor="auto-mode-confirm">
            请输入 <strong>AUTO MODE</strong> 以确认启用
          </label>
          <input
            id="auto-mode-confirm"
            type="text"
            value={confirmText}
            onChange={(e) => setConfirmText(e.target.value)}
            placeholder="AUTO MODE"
            className="mb-3 w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
          <div className="flex gap-2">
            <button
              type="button"
              onClick={handleConfirm}
              disabled={!isConfirmed}
              className="rounded-2xl bg-red-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              确认启用
            </button>
            <button
              type="button"
              onClick={onCancel}
              className="rounded-2xl border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-50"
            >
              取消
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
