import { Brain } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface HighlightedThinkingTextProps {
  text: string;
  highlights?: string[];
  className?: string;
}

export function HighlightedThinkingText({
  text,
  highlights = [],
  className,
}: HighlightedThinkingTextProps) {
  function renderContent(): React.ReactNode {
    if (highlights.length === 0) return text;

    let result: React.ReactNode[] = [text];
    for (const hl of highlights) {
      const next: React.ReactNode[] = [];
      for (const part of result) {
        if (typeof part !== 'string') {
          next.push(part);
          continue;
        }
        const segments = part.split(hl);
        segments.forEach((seg, i) => {
          if (seg) next.push(seg);
          if (i < segments.length - 1) {
            next.push(
              <mark key={`${hl}-${i}`} className="rounded bg-yellow-200 px-0.5 dark:bg-yellow-800">
                {hl}
              </mark>,
            );
          }
        });
      }
      result = next;
    }
    return result;
  }

  return (
    <div
      className={cn(
        'rounded-lg border border-indigo-200 bg-indigo-50 px-4 py-3 dark:border-indigo-800 dark:bg-indigo-950/30',
        className,
      )}
      data-testid="highlighted-thinking-text"
    >
      <div className="flex items-start gap-2">
        <Brain className="mt-0.5 h-4 w-4 shrink-0 text-indigo-500" />
        <p className="text-sm whitespace-pre-wrap text-indigo-900 dark:text-indigo-200">
          {renderContent()}
        </p>
      </div>
    </div>
  );
}
