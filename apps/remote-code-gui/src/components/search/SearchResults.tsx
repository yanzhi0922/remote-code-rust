import { File, MessageSquare, Command, Settings } from 'lucide-react';

export interface SearchResult {
  type: 'message' | 'file' | 'command' | 'setting';
  title: string;
  subtitle?: string;
  icon?: string;
}

export interface SearchResultsProps {
  results: SearchResult[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onHover: (index: number) => void;
}

const TYPE_LABELS: Record<SearchResult['type'], string> = {
  message: '消息',
  file: '文件',
  command: '命令',
  setting: '设置',
};

function ResultIcon({ type }: { type: SearchResult['type'] }) {
  switch (type) {
    case 'message':
      return <MessageSquare className="h-4 w-4 text-blue-500" />;
    case 'file':
      return <File className="h-4 w-4 text-slate-500" />;
    case 'command':
      return <Command className="h-4 w-4 text-purple-500" />;
    case 'setting':
      return <Settings className="h-4 w-4 text-orange-500" />;
  }
}

export function SearchResults({ results, selectedIndex, onSelect, onHover }: SearchResultsProps) {
  if (results.length === 0) {
    return (
      <div data-testid="search-results-empty" className="px-4 py-8 text-center text-sm text-slate-400">
        未找到匹配结果
      </div>
    );
  }

  // Group results by type
  const grouped = results.reduce<Record<string, { result: SearchResult; originalIndex: number }[]>>(
    (acc, result, index) => {
      const label = TYPE_LABELS[result.type];
      if (!acc[label]) acc[label] = [];
      acc[label].push({ result, originalIndex: index });
      return acc;
    },
    {},
  );

  return (
    <div data-testid="search-results" className="max-h-80 overflow-y-auto">
      {Object.entries(grouped).map(([groupLabel, items]) => (
        <div key={groupLabel}>
          <div className="sticky top-0 bg-slate-50 px-3 py-1 text-xs font-medium text-slate-500">
            {groupLabel}
          </div>
          {items.map(({ result, originalIndex }) => (
            <button
              key={originalIndex}
              type="button"
              data-testid={`search-result-${originalIndex}`}
              className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors ${
                originalIndex === selectedIndex
                  ? 'bg-blue-50 text-blue-900'
                  : 'text-slate-700 hover:bg-slate-50'
              }`}
              onClick={() => onSelect(originalIndex)}
              onMouseEnter={() => onHover(originalIndex)}
            >
              <ResultIcon type={result.type} />
              <div className="min-w-0 flex-1">
                <div className="truncate font-medium">{result.title}</div>
                {result.subtitle && (
                  <div className="truncate text-xs text-slate-400">{result.subtitle}</div>
                )}
              </div>
            </button>
          ))}
        </div>
      ))}
      <div className="border-t border-slate-100 px-3 py-1.5 text-xs text-slate-400">
        {results.length} 个结果
      </div>
    </div>
  );
}
