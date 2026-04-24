import { Hash } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface UserChannelMessageProps {
  channel: string;
  content: string;
  sender?: string;
  className?: string;
}

export function UserChannelMessage({
  channel,
  content,
  sender,
  className,
}: UserChannelMessageProps) {
  return (
    <div
      className={cn(
        'rounded-lg border border-teal-200 bg-teal-50 px-4 py-3 dark:border-teal-800 dark:bg-teal-950/30',
        className,
      )}
      data-testid="user-channel-message"
    >
      <div className="flex items-center gap-2 text-xs text-teal-600 dark:text-teal-400">
        <Hash className="h-3.5 w-3.5" />
        <span className="font-medium">{channel}</span>
        {sender && <span>· {sender}</span>}
      </div>
      <p className="mt-1 text-sm whitespace-pre-wrap text-teal-900 dark:text-teal-200">
        {content}
      </p>
    </div>
  );
}
