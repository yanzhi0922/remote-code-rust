import { Webhook } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface UserGitHubWebhookMessageProps {
  event: string;
  action?: string;
  repository?: string;
  sender?: string;
  payload?: string;
  className?: string;
}

export function UserGitHubWebhookMessage({
  event,
  action,
  repository,
  sender,
  payload,
  className,
}: UserGitHubWebhookMessageProps) {
  return (
    <div
      className={cn(
        'rounded-lg border border-gray-200 bg-gray-50 px-4 py-3 dark:border-gray-700 dark:bg-gray-800/50',
        className,
      )}
      data-testid="user-github-webhook-message"
    >
      <div className="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400">
        <Webhook className="h-3.5 w-3.5" />
        <span className="font-medium">{event}</span>
        {action && <span>· {action}</span>}
      </div>
      {(repository || sender) && (
        <div className="mt-1 flex items-center gap-2 text-xs text-gray-500">
          {repository && <span className="font-mono">{repository}</span>}
          {sender && <span>by {sender}</span>}
        </div>
      )}
      {payload && (
        <pre className="mt-2 max-h-32 overflow-auto rounded bg-gray-100 p-2 text-xs dark:bg-gray-900">
          {payload}
        </pre>
      )}
    </div>
  );
}
