import { AlertTriangle } from 'lucide-react';
import type { StructuredDiffFile } from '../diff/StructuredDiff';

export interface FallbackProps {
  diffs: StructuredDiffFile[];
  error?: string;
}

export function Fallback({ diffs, error }: FallbackProps) {
  return (
    <div data-testid="structured-diff-fallback" className="rounded border border-amber-200 bg-amber-50 p-4">
      <div className="mb-2 flex items-center gap-2 text-amber-700">
        <AlertTriangle className="h-4 w-4" />
        <span className="text-sm font-medium">差异显示降级模式</span>
      </div>
      {error && (
        <p className="mb-2 text-xs text-amber-600">{error}</p>
      )}
      <div className="space-y-2">
        {diffs.map((diff, i) => (
          <div key={i} data-testid={`fallback-diff-${i}`} className="rounded bg-white p-2">
            <p className="text-xs font-medium text-slate-700">{diff.file_path}</p>
            <p className="text-xs text-slate-500">
              {diff.hunks.length} 个差异块
            </p>
          </div>
        ))}
      </div>
    </div>
  );
}
