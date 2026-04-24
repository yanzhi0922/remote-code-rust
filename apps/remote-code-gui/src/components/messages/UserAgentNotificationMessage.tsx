import { Bell } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface UserAgentNotificationMessageProps {
  content: string;
  notificationType?: string;
  className?: string;
}

export function UserAgentNotificationMessage({
  content,
  notificationType,
  className,
}: UserAgentNotificationMessageProps) {
  return (
    <div
      className={cn(
        'flex items-start gap-2 rounded-lg border border-blue-200 bg-blue-50 px-4 py-3 dark:border-blue-800 dark:bg-blue-950/30',
        className,
      )}
      data-testid="user-agent-notification-message"
    >
      <Bell className="mt-0.5 h-4 w-4 shrink-0 text-blue-500" />
      <div className="min-w-0 flex-1">
        {notificationType && (
          <span className="mb-1 inline-block rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700 dark:bg-blue-900 dark:text-blue-300">
            {notificationType}
          </span>
        )}
        <p className="text-sm whitespace-pre-wrap text-blue-900 dark:text-blue-200">
          {content}
        </p>
      </div>
    </div>
  );
}
