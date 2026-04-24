import { type ReactNode } from 'react';
import { Cloud, X, AlertCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface AwsAuthStatus {
  isAuthenticating: boolean;
  error: string | null;
  output: string[];
}

interface Props {
  status: AwsAuthStatus;
  onClose?: () => void;
}

export function AwsAuthStatusDialog({ status, onClose }: Props): ReactNode {
  if (!status.isAuthenticating && !status.error) {
    return null;
  }

  return (
    <div
      data-testid="aws-auth-status-dialog"
      className="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-800 dark:bg-amber-950"
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Cloud className="h-5 w-5 text-amber-500" />
          <h4 className="font-semibold text-amber-800 dark:text-amber-200">
            Cloud Authentication
          </h4>
        </div>
        {onClose && (
          <button
            data-testid="aws-auth-close"
            onClick={onClose}
            aria-label="Close"
            className="rounded p-1 text-amber-400 hover:text-amber-600 dark:hover:text-amber-300"
          >
            <X className="h-4 w-4" />
          </button>
        )}
      </div>

      {status.output.length > 0 && (
        <div className="mt-2 space-y-1">
          {status.output.slice(-5).map((line, index) => (
            <p key={index} className="text-sm text-gray-500 dark:text-gray-400">
              {line}
            </p>
          ))}
        </div>
      )}

      {status.error && (
        <div className="mt-2 flex items-center gap-1">
          <AlertCircle className="h-4 w-4 text-red-500" />
          <p className="text-sm text-red-600 dark:text-red-400">{status.error}</p>
        </div>
      )}

      {status.isAuthenticating && (
        <div className="mt-2">
          <div className={cn(
            'h-1.5 w-full overflow-hidden rounded-full',
            'bg-amber-200 dark:bg-amber-900',
          )}>
            <div className="h-full w-1/3 animate-pulse rounded-full bg-amber-500" />
          </div>
        </div>
      )}
    </div>
  );
}
