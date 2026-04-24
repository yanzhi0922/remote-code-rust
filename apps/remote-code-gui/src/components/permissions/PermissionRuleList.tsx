import { useState, useMemo, useCallback } from 'react';
import {
  Search,
  Plus,
  Trash2,
  X,
  ChevronDown,
  ChevronUp,
  Shield,
  AlertTriangle,
  Lock,
  ArrowUpDown,
} from 'lucide-react';
import { PermissionRuleDescription, type PermissionBehavior } from './PermissionRuleDescription';
import { cn } from '../../lib/utils';

export interface PermissionRule {
  id: string;
  tool_name: string;
  rule_content: string;
  behavior: PermissionBehavior;
  source: string;
  isManaged?: boolean;
  overriddenBy?: string | null;
  createdAt?: string;
}

export type TabType = 'recent' | 'allow' | 'ask' | 'deny' | 'workspace';
export type SortMode = 'default' | 'source' | 'tool' | 'behavior';

export interface PermissionRuleListProps {
  rules: PermissionRule[];
  onDelete: (id: string) => void;
  onAddRule: () => void;
}

const TABS: { key: TabType; label: string; emptyMessage: string }[] = [
  { key: 'recent', label: '最近拒绝', emptyMessage: '暂无最近拒绝的规则' },
  { key: 'allow', label: 'Allow', emptyMessage: '暂无允许规则' },
  { key: 'ask', label: 'Ask', emptyMessage: '暂无询问规则' },
  { key: 'deny', label: 'Deny', emptyMessage: '暂无拒绝规则' },
  { key: 'workspace', label: 'Workspace', emptyMessage: '暂无工作区规则' },
];

const SOURCE_LABELS: Record<string, string> = {
  project: '项目',
  user: '用户',
  managed: '托管',
  workspace: '工作区',
  policySettings: '策略设置',
};

function getSourceLabel(source: string): string {
  return SOURCE_LABELS[source] ?? source;
}

function getSourceBadgeColor(source: string): string {
  switch (source) {
    case 'project':
      return 'bg-blue-50 text-blue-600';
    case 'user':
      return 'bg-purple-50 text-purple-600';
    case 'managed':
    case 'policySettings':
      return 'bg-slate-100 text-slate-500';
    case 'workspace':
      return 'bg-emerald-50 text-emerald-600';
    default:
      return 'bg-slate-50 text-slate-500';
  }
}

export function PermissionRuleList({ rules, onDelete, onAddRule }: PermissionRuleListProps) {
  const [activeTab, setActiveTab] = useState<TabType>('allow');
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedRuleId, setExpandedRuleId] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [sortMode, setSortMode] = useState<SortMode>('default');

  // Filter rules by tab
  const tabFilteredRules = useMemo(() => {
    let filtered = rules;
    if (activeTab === 'recent') {
      filtered = filtered.filter((r) => r.behavior === 'deny');
    } else if (activeTab === 'workspace') {
      filtered = filtered.filter((r) => r.source === 'workspace');
    } else {
      filtered = filtered.filter((r) => r.behavior === activeTab);
    }
    return filtered;
  }, [rules, activeTab]);

  // Search filter
  const filteredRules = useMemo(() => {
    let filtered = tabFilteredRules;
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
  }, [tabFilteredRules, searchQuery]);

  // Sort
  const sortedRules = useMemo(() => {
    if (sortMode === 'default') return filteredRules;
    const sorted = [...filteredRules];
    switch (sortMode) {
      case 'source':
        sorted.sort((a, b) => a.source.localeCompare(b.source));
        break;
      case 'tool':
        sorted.sort((a, b) => a.tool_name.localeCompare(b.tool_name));
        break;
      case 'behavior':
        sorted.sort((a, b) => a.behavior.localeCompare(b.behavior));
        break;
    }
    return sorted;
  }, [filteredRules, sortMode]);

  // Rule counts per tab
  const ruleCounts = useMemo(() => {
    const counts: Record<TabType, number> = {
      recent: rules.filter((r) => r.behavior === 'deny').length,
      allow: rules.filter((r) => r.behavior === 'allow').length,
      ask: rules.filter((r) => r.behavior === 'ask').length,
      deny: rules.filter((r) => r.behavior === 'deny').length,
      workspace: rules.filter((r) => r.source === 'workspace').length,
    };
    return counts;
  }, [rules]);

  const currentEmptyMessage = TABS.find((t) => t.key === activeTab)?.emptyMessage ?? '暂无规则';

  const handleDeleteClick = useCallback((id: string) => {
    setDeleteConfirmId(id);
  }, []);

  const handleConfirmDelete = useCallback(
    (id: string) => {
      onDelete(id);
      setDeleteConfirmId(null);
      setExpandedRuleId(null);
    },
    [onDelete],
  );

  const handleCancelDelete = useCallback(() => {
    setDeleteConfirmId(null);
  }, []);

  const toggleExpand = useCallback((id: string) => {
    setExpandedRuleId((prev) => (prev === id ? null : id));
  }, []);

  const cycleSortMode = useCallback(() => {
    setSortMode((prev) => {
      const modes: SortMode[] = ['default', 'source', 'tool', 'behavior'];
      const idx = modes.indexOf(prev);
      return modes[(idx + 1) % modes.length];
    });
  }, []);

  const sortLabel: Record<SortMode, string> = {
    default: '默认排序',
    source: '按来源',
    tool: '按工具',
    behavior: '按行为',
  };

  return (
    <div className="flex flex-col" data-testid="rule-list">
      {/* Tabs */}
      <div className="flex border-b border-slate-200 overflow-x-auto">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            type="button"
            onClick={() => {
              setActiveTab(tab.key);
              setExpandedRuleId(null);
              setDeleteConfirmId(null);
            }}
            className={cn(
              'relative flex items-center gap-1.5 whitespace-nowrap px-4 py-2 text-sm font-medium transition-colors',
              activeTab === tab.key
                ? 'border-b-2 border-blue-600 text-blue-600'
                : 'text-slate-500 hover:text-slate-700',
            )}
            data-testid={`tab-${tab.key}`}
          >
            {tab.label}
            {ruleCounts[tab.key] > 0 && (
              <span
                className={cn(
                  'inline-flex h-5 min-w-[20px] items-center justify-center rounded-full px-1.5 text-[10px] font-semibold',
                  activeTab === tab.key ? 'bg-blue-100 text-blue-700' : 'bg-slate-100 text-slate-500',
                )}
              >
                {ruleCounts[tab.key]}
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Search + Sort + Add */}
      <div className="flex items-center gap-2 border-b border-slate-100 px-4 py-2">
        <div className="relative flex-1">
          <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="搜索规则..."
            className="w-full rounded-lg border border-slate-200 py-1.5 pl-8 pr-3 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            data-testid="rule-search-input"
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
          onClick={cycleSortMode}
          className="flex items-center gap-1 rounded-lg border border-slate-200 px-2 py-1.5 text-xs text-slate-500 hover:bg-slate-50"
          data-testid="rule-sort-button"
          title={sortLabel[sortMode]}
        >
          <ArrowUpDown size={12} />
          {sortLabel[sortMode]}
        </button>
        <button
          type="button"
          onClick={onAddRule}
          className="flex items-center gap-1 rounded-2xl bg-blue-600 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-blue-700"
          data-testid="rule-add-button"
        >
          <Plus size={14} />
          添加
        </button>
      </div>

      {/* Rule items */}
      <div className="max-h-64 overflow-y-auto" data-testid="rule-items-container">
        {sortedRules.length === 0 ? (
          <div className="px-4 py-8 text-center" data-testid="rule-empty-state">
            <Shield className="mx-auto mb-2 h-8 w-8 text-slate-300" />
            <p className="text-sm text-slate-400">
              {searchQuery ? '没有找到匹配的规则' : currentEmptyMessage}
            </p>
          </div>
        ) : (
          sortedRules.map((rule) => {
            const isExpanded = expandedRuleId === rule.id;
            const isConfirmingDelete = deleteConfirmId === rule.id;
            const isManaged = rule.isManaged || rule.source === 'policySettings';
            const isOverridden = !!rule.overriddenBy;

            return (
              <div
                key={rule.id}
                className={cn(
                  'border-b border-slate-50 transition-colors',
                  isExpanded ? 'bg-slate-50' : 'hover:bg-slate-50/50',
                )}
                data-testid={`rule-item-${rule.id}`}
              >
                {/* Rule row */}
                <div className="flex items-start gap-3 px-4 py-3">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                      <PermissionRuleDescription
                        ruleValue={{
                          tool_name: rule.tool_name,
                          rule_content: rule.rule_content,
                          behavior: rule.behavior,
                        }}
                      />
                      {/* Source badge */}
                      <span
                        className={cn(
                          'inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium',
                          getSourceBadgeColor(rule.source),
                        )}
                      >
                        {getSourceLabel(rule.source)}
                      </span>
                      {/* Managed badge */}
                      {isManaged && (
                        <span
                          className="inline-flex items-center gap-0.5 rounded-full bg-slate-100 px-2 py-0.5 text-[10px] font-medium text-slate-500"
                          data-testid={`rule-managed-${rule.id}`}
                        >
                          <Lock className="h-2.5 w-2.5" />
                          托管
                        </span>
                      )}
                      {/* Override warning */}
                      {isOverridden && (
                        <span
                          className="inline-flex items-center gap-0.5 rounded-full bg-amber-50 px-2 py-0.5 text-[10px] font-medium text-amber-600"
                          data-testid={`rule-overridden-${rule.id}`}
                        >
                          <AlertTriangle className="h-2.5 w-2.5" />
                          被 {rule.overriddenBy} 覆盖
                        </span>
                      )}
                    </div>
                    {/* Expand/collapse toggle */}
                    <button
                      type="button"
                      className="mt-1 flex items-center gap-1 text-xs text-slate-400 hover:text-slate-600"
                      onClick={() => toggleExpand(rule.id)}
                      data-testid={`rule-toggle-${rule.id}`}
                    >
                      {isExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
                      {isExpanded ? '收起详情' : '展开详情'}
                    </button>
                  </div>
                  {/* Delete button */}
                  {!isManaged && (
                    <button
                      type="button"
                      onClick={() => handleDeleteClick(rule.id)}
                      className="shrink-0 rounded-lg p-1 text-slate-400 transition-colors hover:bg-red-50 hover:text-red-500"
                      aria-label={`删除规则 ${rule.id}`}
                      data-testid={`rule-delete-${rule.id}`}
                    >
                      <Trash2 size={14} />
                    </button>
                  )}
                </div>

                {/* Expanded details */}
                {isExpanded && (
                  <div
                    className="border-t border-slate-100 bg-white px-4 py-3"
                    data-testid={`rule-detail-${rule.id}`}
                  >
                    <div className="space-y-2 text-xs">
                      <div className="flex items-center gap-2">
                        <span className="text-slate-400">工具:</span>
                        <span className="font-mono text-slate-700">{rule.tool_name}</span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="text-slate-400">规则:</span>
                        <span className="font-mono text-slate-700 break-all">
                          {rule.rule_content || '(全部)'}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="text-slate-400">来源:</span>
                        <span className="text-slate-700">{getSourceLabel(rule.source)}</span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="text-slate-400">行为:</span>
                        <span className="text-slate-700">{rule.behavior}</span>
                      </div>
                      {rule.createdAt && (
                        <div className="flex items-center gap-2">
                          <span className="text-slate-400">创建时间:</span>
                          <span className="text-slate-700">{rule.createdAt}</span>
                        </div>
                      )}
                      {isOverridden && (
                        <div className="flex items-center gap-2 text-amber-600">
                          <AlertTriangle className="h-3 w-3" />
                          <span>此规则被 {rule.overriddenBy} 中的规则覆盖</span>
                        </div>
                      )}
                      {isManaged && (
                        <div className="flex items-center gap-2 text-slate-500">
                          <Lock className="h-3 w-3" />
                          <span>此规则由托管设置配置，无法修改。请联系系统管理员。</span>
                        </div>
                      )}
                    </div>
                  </div>
                )}

                {/* Delete confirmation */}
                {isConfirmingDelete && (
                  <div
                    className="border-t border-red-100 bg-red-50 px-4 py-3"
                    data-testid={`rule-delete-confirm-${rule.id}`}
                  >
                    <p className="text-sm font-medium text-red-700">确认删除此规则？</p>
                    <p className="mt-1 text-xs text-red-500">
                      删除后，此权限规则将不再生效。此操作不可撤销。
                    </p>
                    <div className="mt-2 flex items-center gap-2">
                      <button
                        type="button"
                        className="rounded-lg bg-red-600 px-3 py-1 text-xs font-medium text-white hover:bg-red-700"
                        onClick={() => handleConfirmDelete(rule.id)}
                        data-testid={`rule-delete-confirm-yes-${rule.id}`}
                      >
                        确认删除
                      </button>
                      <button
                        type="button"
                        className="rounded-lg border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-600 hover:bg-slate-50"
                        onClick={handleCancelDelete}
                        data-testid={`rule-delete-confirm-no-${rule.id}`}
                      >
                        取消
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>

      {/* Footer summary */}
      <div className="border-t border-slate-100 px-4 py-2 text-xs text-slate-400">
        共 {rules.length} 条规则 · 显示 {sortedRules.length} 条
      </div>
    </div>
  );
}
