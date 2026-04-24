import { useState } from 'react';
import type { PermissionBehavior } from './PermissionRuleDescription';

export interface PermissionRuleInputProps {
  onSubmit: (rule: {
    tool_name: string;
    rule_content: string;
    behavior: PermissionBehavior;
  }) => void;
  onCancel: () => void;
}

export function PermissionRuleInput({ onSubmit, onCancel }: PermissionRuleInputProps) {
  const [toolName, setToolName] = useState('');
  const [ruleContent, setRuleContent] = useState('');
  const [behavior, setBehavior] = useState<PermissionBehavior>('allow');
  const [errors, setErrors] = useState<{ toolName?: string; ruleContent?: string }>({});

  function handleSubmit() {
    const nextErrors: typeof errors = {};
    if (!toolName.trim()) {
      nextErrors.toolName = '工具名不能为空';
    }
    if (!ruleContent.trim()) {
      nextErrors.ruleContent = '规则内容不能为空';
    }
    if (Object.keys(nextErrors).length > 0) {
      setErrors(nextErrors);
      return;
    }
    onSubmit({
      tool_name: toolName.trim(),
      rule_content: ruleContent.trim(),
      behavior,
    });
    setToolName('');
    setRuleContent('');
    setBehavior('allow');
    setErrors({});
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  }

  return (
    <div className="rounded-xl border border-slate-200 bg-white p-4" data-testid="rule-input">
      <div className="mb-3 text-sm font-semibold text-slate-700">添加权限规则</div>

      <div className="mb-3 flex flex-col gap-2">
        <div>
          <label className="mb-1 block text-xs font-medium text-slate-500" htmlFor="rule-tool-name">
            工具名
          </label>
          <input
            id="rule-tool-name"
            type="text"
            value={toolName}
            onChange={(e) => {
              setToolName(e.target.value);
              if (errors.toolName) setErrors((prev) => ({ ...prev, toolName: undefined }));
            }}
            onKeyDown={handleKeyDown}
            placeholder="e.g. Bash"
            className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
          {errors.toolName && (
            <span className="mt-1 text-xs text-red-500">{errors.toolName}</span>
          )}
        </div>

        <div>
          <label className="mb-1 block text-xs font-medium text-slate-500" htmlFor="rule-content">
            规则内容
          </label>
          <input
            id="rule-content"
            type="text"
            value={ruleContent}
            onChange={(e) => {
              setRuleContent(e.target.value);
              if (errors.ruleContent) setErrors((prev) => ({ ...prev, ruleContent: undefined }));
            }}
            onKeyDown={handleKeyDown}
            placeholder="e.g. npm test"
            className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
          {errors.ruleContent && (
            <span className="mt-1 text-xs text-red-500">{errors.ruleContent}</span>
          )}
        </div>

        <div>
          <label className="mb-1 block text-xs font-medium text-slate-500" htmlFor="rule-behavior">
            行为
          </label>
          <select
            id="rule-behavior"
            value={behavior}
            onChange={(e) => setBehavior(e.target.value as PermissionBehavior)}
            className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          >
            <option value="allow">Allow</option>
            <option value="deny">Deny</option>
            <option value="ask">Ask</option>
          </select>
        </div>
      </div>

      <div className="flex gap-2">
        <button
          type="button"
          onClick={handleSubmit}
          className="rounded-2xl bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700"
        >
          添加
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
  );
}
