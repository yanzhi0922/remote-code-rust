import { cn } from '../../lib/utils';

export interface Tab {
  id: string;
  label: string;
  count?: number;
}

export interface TagTabsProps {
  tabs: Tab[];
  activeTab: string;
  onChange: (tabId: string) => void;
}

export function TagTabs({ tabs, activeTab, onChange }: TagTabsProps) {
  return (
    <div data-testid="tag-tabs" className="flex gap-1">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          data-testid={`tag-tab-${tab.id}`}
          className={cn(
            'inline-flex items-center gap-1 rounded-full px-3 py-1 text-sm font-medium transition-colors',
            activeTab === tab.id
              ? 'bg-blue-100 text-blue-700'
              : 'text-slate-500 hover:bg-slate-100',
          )}
          onClick={() => onChange(tab.id)}
        >
          {tab.label}
          {tab.count !== undefined && (
            <span className={cn(
              'rounded-full px-1.5 py-0.5 text-xs',
              activeTab === tab.id ? 'bg-blue-200 text-blue-800' : 'bg-slate-200 text-slate-600',
            )}>
              {tab.count}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}
