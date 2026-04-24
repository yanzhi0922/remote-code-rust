import { type ReactNode, useState } from 'react';
import { Globe, X, Loader2, CheckCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface EnvironmentResource {
  environment_id: string;
  name: string;
  description?: string;
}

interface Props {
  onDone: (message?: string) => void;
  environments?: EnvironmentResource[];
  selectedEnvironmentId?: string | null;
  loading?: boolean;
}

export function RemoteEnvironmentDialog({
  onDone,
  environments = [],
  selectedEnvironmentId,
  loading = false,
}: Props): ReactNode {
  const [selected, setSelected] = useState<string | null>(selectedEnvironmentId ?? null);
  const [updating, setUpdating] = useState(false);

  const handleSelect = (envId: string) => {
    setSelected(envId);
  };

  const handleConfirm = () => {
    if (selected) {
      setUpdating(true);
      onDone(`Switched to environment: ${selected}`);
    }
  };

  const handleCancel = () => {
    onDone();
  };

  return (
    <div
      data-testid="remote-environment-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Globe className="h-5 w-5 text-blue-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Select Remote Environment
            </h3>
          </div>
          <button
            data-testid="remote-environment-close"
            onClick={handleCancel}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-2 text-xs text-gray-500 dark:text-gray-500">
          Configure environments at: https://claude.ai/code
        </p>

        {loading ? (
          <div className="mt-4 flex items-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin text-blue-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">Loading environments…</p>
          </div>
        ) : (
          <div className="mt-4 space-y-2">
            {environments.length === 0 ? (
              <p className="text-sm text-gray-500 dark:text-gray-500">
                No environments configured.
              </p>
            ) : (
              environments.map((env) => (
                <button
                  key={env.environment_id}
                  data-testid={`remote-env-${env.environment_id}`}
                  onClick={() => handleSelect(env.environment_id)}
                  className={cn(
                    'w-full rounded px-4 py-2 text-left text-sm',
                    selected === env.environment_id
                      ? 'border-2 border-blue-500 bg-blue-50 dark:bg-blue-950'
                      : 'border border-gray-200 bg-gray-50 hover:bg-gray-100 dark:border-gray-600 dark:bg-gray-700 dark:hover:bg-gray-600',
                  )}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-medium text-gray-900 dark:text-gray-100">
                      {env.name}
                    </span>
                    {selected === env.environment_id && (
                      <CheckCircle className="h-4 w-4 text-blue-500" />
                    )}
                  </div>
                  {env.description && (
                    <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">{env.description}</p>
                  )}
                </button>
              ))
            )}
          </div>
        )}

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="remote-environment-cancel"
            onClick={handleCancel}
            className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
          >
            Cancel
          </button>
          <button
            data-testid="remote-environment-confirm"
            onClick={handleConfirm}
            disabled={!selected || updating}
            className={cn(
              'rounded px-4 py-2 text-sm text-white',
              selected && !updating
                ? 'bg-blue-600 hover:bg-blue-700'
                : 'bg-gray-400 cursor-not-allowed',
            )}
          >
            {updating ? 'Updating…' : 'Select'}
          </button>
        </div>
      </div>
    </div>
  );
}
