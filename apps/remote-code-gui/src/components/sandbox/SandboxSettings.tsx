import React, { useState } from 'react';
import { Shield, Package, Settings2 } from 'lucide-react';
import { cn } from '../../lib/utils';
import { SandboxConfigTab } from './SandboxConfigTab';
import { SandboxDependenciesTab } from './SandboxDependenciesTab';
import type { DependencyCheckResult } from './SandboxDependenciesTab';
import { SandboxOverridesTab } from './SandboxOverridesTab';
import type { SandboxConfig } from './SandboxConfigTab';

type SandboxMode = 'auto-allow' | 'regular' | 'disabled';

type TabKey = 'config' | 'dependencies' | 'overrides';

type Props = {
  config: SandboxConfig;
  depCheck: DependencyCheckResult;
  isLocked?: boolean;
  onModeChange?: (mode: SandboxMode) => void;
  onOverrideChange?: (mode: 'open' | 'closed') => void;
};

const TABS: { key: TabKey; label: string; icon: React.ReactNode }[] = [
  { key: 'config', label: 'Config', icon: <Settings2 className="h-4 w-4" /> },
  { key: 'dependencies', label: 'Dependencies', icon: <Package className="h-4 w-4" /> },
  { key: 'overrides', label: 'Overrides', icon: <Shield className="h-4 w-4" /> },
];

export function SandboxSettings({
  config,
  depCheck,
  isLocked = false,
  onModeChange,
  onOverrideChange,
}: Props): React.ReactElement {
  const [activeTab, setActiveTab] = useState<TabKey>('config');

  return (
    <div data-testid="sandbox-settings" className="flex flex-col">
      <div className="flex items-center gap-1 border-b border-gray-200 dark:border-gray-700">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            data-testid={`sandbox-tab-${tab.key}`}
            className={cn(
              'flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm font-medium transition-colors',
              activeTab === tab.key
                ? 'border-cyan-500 text-cyan-600 dark:text-cyan-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300',
            )}
            onClick={() => setActiveTab(tab.key)}
          >
            {tab.icon}
            {tab.label}
          </button>
        ))}
      </div>

      <div className="p-3">
        {activeTab === 'config' && (
          <SandboxConfigTab config={config} warnings={depCheck.warnings} />
        )}
        {activeTab === 'dependencies' && (
          <SandboxDependenciesTab depCheck={depCheck} />
        )}
        {activeTab === 'overrides' && (
          <SandboxOverridesTab
            isEnabled={config.enabled}
            isLocked={isLocked}
            currentAllowUnsandboxed={config.autoAllowBashIfSandboxed}
            onModeChange={onOverrideChange}
          />
        )}
      </div>

      {onModeChange && (
        <div className="border-t border-gray-200 p-3 dark:border-gray-700">
          <span className="mb-2 block text-sm font-medium text-gray-700 dark:text-gray-300">
            Sandbox Mode:
          </span>
          <div className="flex gap-2">
            {(['auto-allow', 'regular', 'disabled'] as SandboxMode[]).map((mode) => (
              <button
                key={mode}
                data-testid={`sandbox-mode-${mode}`}
                className={cn(
                  'rounded-md border px-3 py-1.5 text-sm transition-colors',
                  'hover:border-cyan-300 dark:hover:border-cyan-600',
                )}
                onClick={() => onModeChange(mode)}
              >
                {mode === 'auto-allow'
                  ? 'Auto-allow'
                  : mode === 'regular'
                    ? 'Regular'
                    : 'Disabled'}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
