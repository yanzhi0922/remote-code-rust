import { Send } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface SubmitQuestionsViewProps {
  answers: Record<string, string>;
  onSubmit: () => void;
  className?: string;
}

export function SubmitQuestionsView({ answers, onSubmit, className }: SubmitQuestionsViewProps) {
  const count = Object.keys(answers).length;

  return (
    <div className={cn('rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-700', className)} data-testid="submit-questions-view">
      <p className="text-sm text-slate-600">
        已回答 <span className="font-semibold">{count}</span> 个问题
      </p>
      <button
        className="mt-3 flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:opacity-50"
        onClick={onSubmit}
        disabled={count === 0}
      >
        <Send className="h-4 w-4" />
        提交答案
      </button>
    </div>
  );
}
