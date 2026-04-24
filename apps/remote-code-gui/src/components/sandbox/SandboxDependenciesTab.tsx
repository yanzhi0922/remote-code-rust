import React from 'react';
import { CheckCircle, XCircle, AlertTriangle } from 'lucide-react';

export interface DependencyCheckResult {
  errors: string[];
  warnings: string[];
}

type Props = {
  depCheck: DependencyCheckResult;
  platform?: 'macos' | 'linux' | 'windows';
};

export function SandboxDependenciesTab({
  depCheck,
  platform = 'linux',
}: Props): React.ReactElement {
  const isMac = platform === 'macos';

  const rgMissing = depCheck.errors.some((e) => e.includes('ripgrep'));
  const bwrapMissing = depCheck.errors.some((e) => e.includes('bwrap'));
  const socatMissing = depCheck.errors.some((e) => e.includes('socat'));
  const seccompMissing = depCheck.warnings.length > 0;

  const otherErrors = depCheck.errors.filter(
    (e) => !e.includes('ripgrep') && !e.includes('bwrap') && !e.includes('socat'),
  );

  const rgInstallHint = isMac ? 'brew install ripgrep' : 'apt install ripgrep';

  return (
    <div data-testid="sandbox-dependencies-tab" className="flex flex-col gap-2 py-2">
      {isMac && (
        <div className="flex items-center gap-2">
          <span className="text-sm">seatbelt:</span>
          <span className="text-sm text-green-600 dark:text-green-400">built-in (macOS)</span>
        </div>
      )}

      <div className="flex flex-col">
        <div className="flex items-center gap-2">
          <span className="text-sm">ripgrep (rg):</span>
          {rgMissing ? (
            <>
              <XCircle className="h-4 w-4 text-red-500" />
              <span className="text-sm text-red-600 dark:text-red-400">not found</span>
            </>
          ) : (
            <>
              <CheckCircle className="h-4 w-4 text-green-500" />
              <span className="text-sm text-green-600 dark:text-green-400">found</span>
            </>
          )}
        </div>
        {rgMissing && (
          <span className="ml-6 text-sm text-gray-500 dark:text-gray-400">· {rgInstallHint}</span>
        )}
      </div>

      {!isMac && (
        <>
          <div className="flex flex-col">
            <div className="flex items-center gap-2">
              <span className="text-sm">bubblewrap (bwrap):</span>
              {bwrapMissing ? (
                <>
                  <XCircle className="h-4 w-4 text-red-500" />
                  <span className="text-sm text-red-600 dark:text-red-400">not installed</span>
                </>
              ) : (
                <>
                  <CheckCircle className="h-4 w-4 text-green-500" />
                  <span className="text-sm text-green-600 dark:text-green-400">installed</span>
                </>
              )}
            </div>
            {bwrapMissing && (
              <span className="ml-6 text-sm text-gray-500 dark:text-gray-400">
                · apt install bubblewrap
              </span>
            )}
          </div>

          <div className="flex flex-col">
            <div className="flex items-center gap-2">
              <span className="text-sm">socat:</span>
              {socatMissing ? (
                <>
                  <XCircle className="h-4 w-4 text-red-500" />
                  <span className="text-sm text-red-600 dark:text-red-400">not installed</span>
                </>
              ) : (
                <>
                  <CheckCircle className="h-4 w-4 text-green-500" />
                  <span className="text-sm text-green-600 dark:text-green-400">installed</span>
                </>
              )}
            </div>
            {socatMissing && (
              <span className="ml-6 text-sm text-gray-500 dark:text-gray-400">
                · apt install socat
              </span>
            )}
          </div>

          <div className="flex flex-col">
            <div className="flex items-center gap-2">
              <span className="text-sm">seccomp filter:</span>
              {seccompMissing ? (
                <>
                  <AlertTriangle className="h-4 w-4 text-yellow-500" />
                  <span className="text-sm text-yellow-600 dark:text-yellow-400">not installed</span>
                </>
              ) : (
                <>
                  <CheckCircle className="h-4 w-4 text-green-500" />
                  <span className="text-sm text-green-600 dark:text-green-400">installed</span>
                </>
              )}
            </div>
          </div>
        </>
      )}

      {otherErrors.map((err) => (
        <span key={err} className="text-sm text-red-600 dark:text-red-400">
          {err}
        </span>
      ))}
    </div>
  );
}
