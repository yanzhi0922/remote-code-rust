import { Plus, Shield } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../../lib/utils';

export interface AddPermissionRulesProps {
  onAdd: (rule: string) => void;
  className?: string;
}

export function AddPermissionRules({ onAdd, className }: AddPermissionRulesProps) {
  const [rule, setRule] = useState('');

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (rule.trim()) {
      onAdd(rule.trim());
      setRule('');
    }
  }

  return (
    <form
      className={cn('flex items-center gap-2', className)}
      data-testid="add-permission-rules"
      onSubmit={handleSubmit}
    >
      <Shield className="h-4 w-4 shrink-0 text-slate-400" />
      <input
        type="text"
        value={rule}
        onChange={(e) => setRule(e.target.value)}
        placeholder="添加权限规则..."
        className="min-w-0 flex-1 rounded-md border border-slate-300 bg-white px-3 py-1.5 text-sm dark:border-slate-600 dark:bg-slate-800"
        data-testid="permission-rule-input"
      />
      <button
        type="submit"
        className="rounded-md bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700 disabled:opacity-50"
        disabled={!rule.trim()}
        title="添加规则"
        data-testid="add-rule-btn"
      >
        <Plus className="h-4 w-4" />
      </button>
    </form>
  );
}
