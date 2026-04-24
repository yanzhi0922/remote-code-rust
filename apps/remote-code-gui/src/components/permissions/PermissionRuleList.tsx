import { useState, useMemo } from 'react';
import { Search, Plus, Trash2, X } from 'lucide-react';
import { PermissionRuleDescription, type PermissionBehavior } from './PermissionRuleDescription';

export interface PermissionRule {
  id: string;
  tool_name: string;
  rule_content: string;
  behavior: PermissionBehavior;
  source: string;
}

export type TabType = 'recent' | 'allow' | 'ask' | 'deny' | 'workspace';

export interface PermissionRuleListProps {
  rules: PermissionRule[];
  onDelete: (id: string) => void;
  onAddRule: () => void;
}

const TABS: { key: TabType; label: string }[] = [
  { key: 'recent', label: '最近拒绝' },
  { key: 'allow', label: 'Allow' },
  { key: 'ask', label: 'Ask' },
  { key: 'deny', label: 'Deny' },
  { key: 'workspace', label: 'Workspace' },
];

export function PermissionRuleList({ rules, onDelete, onAddRule }: PermissionRuleListProps) {
  const [activeTab, setActiveTab] = useState<TabType>('allow');
  const [searchQuery, setSearchQuery] = useState('');

  const filteredRules = useMemo(() => {
    let filtered = rules;

    // Filter by tab
    if (activeTab === 'recent') {
      filtered = filtered.filter((r) => r.behavior === 'deny');
    } else if (activeTab === 'workspace') {
      filtered = filtered.filter((r) => r.source === 'workspace');
    } else {
      filtered = filtered.filter((r) => r.behavior === activeTab);
    }

    // Filter by search
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (r) =>
          r.tool_name.toLowerCase().includes(q) ||
          r.rule_content.toLowerCase().includes(q) ||
          r.source.toLowerCase().includes(q),
      );
    }

    return filtered;
  }, [rules, activeTab, searchQuery]);

  return (
    <div className="flex flex-col" data-testid="rule-list">
      {/* Tabs */}
      <div className="flex border-b border-slate-200">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            type="button"
            onClick={() => setActiveTab(tab.key)}
            className={`px-4 py-2 text-sm font-medium transition-colors ${
              activeTab === tab.key
                ? 'border-b-2 border-blue-600 text-blue-600'
                : 'text-slate-500 hover:text-slate-700'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Search + Add */}
      <div className="flex items-center gap-2 border-b border-slate-100 px-4 py-2">
        <div className="relative flex-1">
          <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="搜索规则..."
            className="w-full rounded-lg border border-slate-200 py-1.5 pl-8 pr-3 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => setSearchQuery('')}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600"
              aria-label="清除搜索"
            >
              <X size={14} />
            </button>
          )}
        </div>
        <button
          type="button"
          onClick={onAddRule}
          className="flex items-center gap-1 rounded-2xl bg-blue-600 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-blue-700"
        >
          <Plus size={14} />
          添加
        </button>
      </div>

      {/* Rule items */}
      <div className="max-h-64 overflow-y-auto">
        {filteredRules.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-slate-400">
            {searchQuery ? '没有找到匹配的规则' : '暂无规则'}
          </div>
        ) : (
          filteredRules.map((rule) => (
            <div
              key={rule.id}
              className="flex items-start gap-3 border-b border-slate-50 px-4 py-3 transition-colors hover:bg-slate-50"
            >
              <div className="flex-1">
                <PermissionRuleDescription
                  ruleValue={{
                    tool_name: rule.tool_name,
                    rule_content: rule.rule_content,
                    behavior: rule.behavior,
                  }}
                />
                <div className="mt-1 text-xs text-slate-400">来源: {rule.source}</div>
              </div>
              <button
                type="button"
                onClick={() => onDelete(rule.id)}
                className="shrink-0 rounded-lg p-1 text-slate-400 transition-colors hover:bg-red-50 hover:text-red-500"
                aria-label={`删除规则 ${rule.id}`}
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
