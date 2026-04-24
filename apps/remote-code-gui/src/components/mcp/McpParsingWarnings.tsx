import { AlertTriangle, ChevronDown, ChevronUp } from 'lucide-react';
import { useState } from 'react';

interface McpParsingWarningsProps {
  warnings: string[];
}

export function McpParsingWarnings({ warnings }: McpParsingWarningsProps) {
  const [expanded, setExpanded] = useState(true);

  if (warnings.length === 0) return null;

  return (
    <div className="rounded-2xl border border-amber-200 bg-amber-50" data-testid="mcp-parsing-warnings">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center justify-between px-4 py-3 text-left"
        data-testid="mcp-parsing-warnings-toggle"
      >
        <div className="flex items-center gap-2">
          <AlertTriangle size={16} className="text-amber-600" />
          <span className="text-sm font-medium text-amber-800">
            配置警告 ({warnings.length})
          </span>
        </div>
        {expanded ? (
          <ChevronUp size={16} className="text-amber-600" />
        ) : (
          <ChevronDown size={16} className="text-amber-600" />
        )}
      </button>

      {expanded && (
        <div className="border-t border-amber-200 px-4 py-2">
          {warnings.map((warning, idx) => (
            <div key={idx} className="py-1.5 text-sm text-amber-700">
              {warning}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
