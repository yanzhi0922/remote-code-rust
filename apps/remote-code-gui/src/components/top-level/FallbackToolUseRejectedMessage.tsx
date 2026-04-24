import { StopCircle } from 'lucide-react';

export function FallbackToolUseRejectedMessage() {
  return (
    <div data-testid="fallback-tool-use-rejected" className="flex items-center gap-1.5 text-slate-500">
      <StopCircle className="h-4 w-4" />
      <span className="text-sm">操作已被用户中断</span>
    </div>
  );
}
