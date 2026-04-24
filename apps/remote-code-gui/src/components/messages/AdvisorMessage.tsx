import { Lightbulb } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface AdvisorMessageProps {
  content: string;
  sender?: string;
  timestamp?: string;
  className?: string;
}

export function AdvisorMessage({ content, sender, timestamp, className }: AdvisorMessageProps) {
  return (
    <div
      className={cn(
        'rounded-lg border border-purple-200 bg-purple-50 px-4 py-3 dark:border-purple-800 dark:bg-purple-950/30',
        className,
      )}
      data-testid="advisor-message"
    >
      <div className="flex items-start gap-2">
        <Lightbulb className="mt-0.5 h-4 w-4 shrink-0 text-purple-500" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-purple-700 dark:text-purple-400">
              {sender ?? 'Advisor'}
            </span>
            {timestamp && (
              <span className="text-xs text-purple-400">{timestamp}</span>
            )}
          </div>
          <p className="mt-1 text-sm whitespace-pre-wrap text-purple-900 dark:text-purple-200">
            {content}
          </p>
        </div>
      </div>
    </div>
  );
}
