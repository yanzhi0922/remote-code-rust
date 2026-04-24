import { Eye } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface PreviewQuestionViewProps {
  question: string;
  answer?: string;
  className?: string;
}

export function PreviewQuestionView({ question, answer, className }: PreviewQuestionViewProps) {
  return (
    <div className={cn('rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-700', className)} data-testid="preview-question-view">
      <div className="flex items-start gap-2">
        <Eye className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
        <div>
          <p className="text-sm font-medium text-slate-700">{question}</p>
          {answer && <p className="mt-1 text-sm text-slate-500">{answer}</p>}
        </div>
      </div>
    </div>
  );
}
