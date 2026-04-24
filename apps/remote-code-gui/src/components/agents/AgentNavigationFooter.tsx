import { ChevronLeft, ChevronRight, Plus } from 'lucide-react';

export interface AgentNavigationFooterProps {
  currentIndex: number;
  totalCount: number;
  onPrev?: () => void;
  onNext?: () => void;
  onAdd?: () => void;
}

export function AgentNavigationFooter({
  currentIndex,
  totalCount,
  onPrev,
  onNext,
  onAdd,
}: AgentNavigationFooterProps) {
  return (
    <div data-testid="agent-nav-footer" className="flex items-center justify-between border-t border-slate-200 px-4 py-2">
      <button
        type="button"
        data-testid="agent-nav-prev"
        className="inline-flex items-center gap-1 rounded px-2 py-1 text-sm text-slate-600 hover:bg-slate-100 disabled:opacity-30"
        onClick={onPrev}
        disabled={currentIndex <= 0}
        title="上一个"
      >
        <ChevronLeft className="h-4 w-4" />
        上一个
      </button>
      <span data-testid="agent-nav-position" className="text-xs text-slate-400">
        {totalCount > 0 ? `${currentIndex + 1} / ${totalCount}` : '0 / 0'}
      </span>
      <div className="flex gap-1">
        <button
          type="button"
          data-testid="agent-nav-next"
          className="inline-flex items-center gap-1 rounded px-2 py-1 text-sm text-slate-600 hover:bg-slate-100 disabled:opacity-30"
          onClick={onNext}
          disabled={currentIndex >= totalCount - 1}
          title="下一个"
        >
          下一个
          <ChevronRight className="h-4 w-4" />
        </button>
        {onAdd && (
          <button
            type="button"
            data-testid="agent-nav-add"
            className="inline-flex items-center gap-1 rounded bg-blue-600 px-2 py-1 text-sm text-white hover:bg-blue-700"
            onClick={onAdd}
            title="新建"
          >
            <Plus className="h-4 w-4" />
          </button>
        )}
      </div>
    </div>
  );
}
