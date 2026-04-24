import { HelpCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface QuestionViewProps {
  question: string;
  index: number;
  total: number;
  className?: string;
}

export function QuestionView({ question, index, total, className }: QuestionViewProps) {
  return (
    <div className={cn('rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-700', className)} data-testid="question-view">
      <div className="flex items-center gap-2 text-xs text-slate-400">
        <HelpCircle className="h-3.5 w-3.5" />
        <span>问题 {index + 1}/{total}</span>
      </div>
      <p className="mt-2 text-sm text-slate-700">{question}</p>
    </div>
  );
}
