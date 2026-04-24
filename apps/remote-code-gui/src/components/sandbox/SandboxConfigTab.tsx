import React from 'react';
import { Shield, AlertTriangle } from 'lucide-react';

export interface SandboxConfig {
  enabled: boolean;
  autoAllowBashIfSandboxed: boolean;
  excludedCommands: string[];
  fsReadDenyPaths: string[];
  fsWriteAllowPaths: string[];
  networkAllowedHosts: string[];
  networkDeniedHosts: string[];
}

type Props = {
  config: SandboxConfig;
  warnings?: string[];
};

export function SandboxConfigTab({ config, warnings = [] }: Props): React.ReactElement {
  if (!config.enabled) {
    return (
      <div data-testid="sandbox-config-tab" className="flex flex-col py-2">
        <span className="text-gray-500 dark:text-gray-400">Sandbox is not enabled</span>
        {warnings.length > 0 && (
          <div className="mt-2 flex flex-col">
            {warnings.map((w, i) => (
              <span key={i} className="text-sm text-gray-500 dark:text-gray-400">{w}</span>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div data-testid="sandbox-config-tab" className="flex flex-col gap-3 py-2">
      <div className="flex items-center gap-2">
        <Shield className="h-4 w-4 text-green-500" />
        <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
          Sandbox Enabled
        </span>
        {config.autoAllowBashIfSandboxed && (
          <span className="rounded bg-green-100 px-1.5 py-0.5 text-xs text-green-700 dark:bg-green-900/30 dark:text-green-400">
            Auto-allow
          </span>
        )}
      </div>

      <div className="flex flex-col">
        <span className="text-sm font-medium text-orange-600 dark:text-orange-400">
          Excluded Commands:
        </span>
        <span className="text-sm text-gray-500 dark:text-gray-400">
          {config.excludedCommands.length > 0
            ? config.excludedCommands.join(', ')
            : 'None'}
        </span>
      </div>

      {config.fsReadDenyPaths.length > 0 && (
        <div className="flex flex-col">
          <span className="text-sm font-medium text-orange-600 dark:text-orange-400">
            Filesystem Read Restrictions:
          </span>
          <span className="text-sm text-gray-500 dark:text-gray-400">
            Denied: {config.fsReadDenyPaths.join(', ')}
          </span>
        </div>
      )}

      {config.fsWriteAllowPaths.length > 0 && (
        <div className="flex flex-col">
          <span className="text-sm font-medium text-orange-600 dark:text-orange-400">
            Filesystem Write Restrictions:
          </span>
          <span className="text-sm text-gray-500 dark:text-gray-400">
            Allowed: {config.fsWriteAllowPaths.join(', ')}
          </span>
        </div>
      )}

      {(config.networkAllowedHosts.length > 0 || config.networkDeniedHosts.length > 0) && (
        <div className="flex flex-col">
          <span className="text-sm font-medium text-orange-600 dark:text-orange-400">
            Network Restrictions:
          </span>
          {config.networkAllowedHosts.length > 0 && (
            <span className="text-sm text-gray-500 dark:text-gray-400">
              Allowed: {config.networkAllowedHosts.join(', ')}
            </span>
          )}
          {config.networkDeniedHosts.length > 0 && (
            <span className="text-sm text-gray-500 dark:text-gray-400">
              Denied: {config.networkDeniedHosts.join(', ')}
            </span>
          )}
        </div>
      )}

      {warnings.length > 0 && (
        <div className="flex flex-col gap-1">
          {warnings.map((w, i) => (
            <div key={i} className="flex items-center gap-1">
              <AlertTriangle className="h-3 w-3 text-yellow-500" />
              <span className="text-sm text-yellow-600 dark:text-yellow-400">{w}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
