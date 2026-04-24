import { ChevronLeft, ChevronRight } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface QuestionNavigationBarProps {
  current: number;
  total: number;
  onPrev: () => void;
  onNext: () => void;
  className?: string;
}

export function QuestionNavigationBar({ current, total, onPrev, onNext, className }: QuestionNavigationBarProps) {
  return (
    <div className={cn('flex items-center gap-2', className)} data-testid="question-navigation-bar">
      <button className="rounded p-1 hover:bg-slate-100" onClick={onPrev} disabled={current <= 0} title="上一个">
        <ChevronLeft className="h-4 w-4" />
      </button>
      <span className="text-sm text-slate-600">
        {current + 1} / {total}
      </span>
      <button className="rounded p-1 hover:bg-slate-100" onClick={onNext} disabled={current >= total - 1} title="下一个">
        <ChevronRight className="h-4 w-4" />
      </button>
    </div>
  );
}
