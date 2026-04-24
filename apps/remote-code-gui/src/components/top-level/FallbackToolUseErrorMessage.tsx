import { AlertCircle } from 'lucide-react';

export interface FallbackToolUseErrorMessageProps {
  error: string;
  verbose?: boolean;
}

const MAX_LINES = 10;

export function FallbackToolUseErrorMessage({ error, verbose = false }: FallbackToolUseErrorMessageProps) {
  const displayError = error.startsWith('Error:') || error.startsWith('Cancelled:')
    ? error
    : `Error: ${error}`;

  const lines = displayError.split('\n');
  const truncated = !verbose && lines.length > MAX_LINES;
  const visibleLines = truncated ? lines.slice(0, MAX_LINES) : lines;

  return (
    <div data-testid="fallback-tool-use-error" className="rounded border border-red-200 bg-red-50 p-3">
      <div className="mb-1 flex items-center gap-1.5 text-red-600">
        <AlertCircle className="h-4 w-4 shrink-0" />
        <span className="text-sm font-medium">工具执行错误</span>
      </div>
      <pre data-testid="fallback-tool-use-error-text" className="overflow-x-auto text-xs text-red-700">
        {visibleLines.join('\n')}
      </pre>
      {truncated && (
        <p className="mt-1 text-xs text-red-500">
          还有 {lines.length - MAX_LINES} 行未显示。使用详细模式查看完整输出。
        </p>
      )}
    </div>
  );
}
