import React, { useState } from 'react';
import { Shield, AlertTriangle } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface TrustWarning {
  type: string;
  label: string;
  description: string;
}

type Props = {
  warnings?: TrustWarning[];
  onAccept: () => void;
  onDecline: () => void;
  projectName?: string;
};

export function TrustDialog({
  warnings = [],
  onAccept,
  onDecline,
  projectName,
}: Props): React.ReactElement {
  const [accepted, setAccepted] = useState(false);

  const handleAccept = () => {
    setAccepted(true);
    onAccept();
  };

  if (accepted) {
    return (
      <div data-testid="trust-dialog-accepted" className="rounded-md bg-green-50 p-3 dark:bg-green-900/20">
        <span className="text-sm text-green-700 dark:text-green-400">
          ✓ Trust accepted. Session will continue.
        </span>
      </div>
    );
  }

  return (
    <div
      data-testid="trust-dialog"
      className="rounded-lg border border-yellow-200 bg-yellow-50 p-4 shadow-lg dark:border-yellow-700/50 dark:bg-yellow-900/20"
    >
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2">
          <Shield className="h-5 w-5 text-yellow-600 dark:text-yellow-400" />
          <h3 className="text-lg font-semibold text-yellow-800 dark:text-yellow-300">
            Trust This Project?
          </h3>
        </div>
      </div>

      {projectName && (
        <p className="mt-2 text-sm text-yellow-700 dark:text-yellow-400">
          Project: <span className="font-mono font-medium">{projectName}</span>
        </p>
      )}

      <p className="mt-2 text-sm text-yellow-700 dark:text-yellow-400">
        This project may contain configuration that could execute code or access sensitive data.
        Review the warnings below before proceeding.
      </p>

      {warnings.length > 0 && (
        <div className="mt-3 flex flex-col gap-2">
          {warnings.map((warning, i) => (
            <div
              key={i}
              data-testid={`trust-warning-${warning.type}`}
              className="flex items-start gap-2 rounded-md bg-yellow-100 p-2 dark:bg-yellow-900/30"
            >
              <AlertTriangle className="mt-0.5 h-4 w-4 flex-shrink-0 text-yellow-600 dark:text-yellow-400" />
              <div>
                <span className="text-sm font-medium text-yellow-800 dark:text-yellow-300">
                  {warning.label}
                </span>
                <p className="text-xs text-yellow-700 dark:text-yellow-400">
                  {warning.description}
                </p>
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="mt-4 flex gap-2">
        <button
          data-testid="trust-accept-btn"
          className={cn(
            'rounded-md bg-yellow-600 px-4 py-2 text-sm font-medium text-white',
            'hover:bg-yellow-700 transition-colors',
          )}
          onClick={handleAccept}
        >
          Trust & Continue
        </button>
        <button
          data-testid="trust-decline-btn"
          className={cn(
            'rounded-md border border-yellow-300 px-4 py-2 text-sm font-medium text-yellow-700',
            'hover:bg-yellow-100 transition-colors dark:border-yellow-600 dark:text-yellow-400 dark:hover:bg-yellow-900/30',
          )}
          onClick={onDecline}
        >
          Decline
        </button>
      </div>
    </div>
  );
}
