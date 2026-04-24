import React, { useState } from 'react';
import { X, HelpCircle, Terminal, BookOpen, Keyboard } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface HelpCommand {
  name: string;
  description: string;
  shortcut?: string;
}

type TabKey = 'general' | 'commands' | 'shortcuts';

type Props = {
  commands?: HelpCommand[];
  onClose: () => void;
};

const DEFAULT_COMMANDS: HelpCommand[] = [
  { name: '/help', description: 'Show help dialog', shortcut: 'Shift+?' },
  { name: '/model', description: 'Select a model' },
  { name: '/clear', description: 'Clear conversation' },
  { name: '/compact', description: 'Compact conversation' },
  { name: '/config', description: 'Open settings' },
  { name: '/status', description: 'Show status info' },
  { name: '/memory', description: 'Edit memory files' },
  { name: '/cost', description: 'Show token usage' },
  { name: '/doctor', description: 'Run diagnostics' },
];

const SHORTCUTS = [
  { key: 'Escape', action: 'Cancel current operation' },
  { key: 'Ctrl+C', action: 'Interrupt current response' },
  { key: 'Ctrl+L', action: 'Clear screen' },
  { key: '↑ / ↓', action: 'Navigate history' },
  { key: 'Tab', action: 'Autocomplete' },
  { key: 'Shift+?', action: 'Toggle help' },
];

const TABS: { key: TabKey; label: string; icon: React.ReactNode }[] = [
  { key: 'general', label: 'General', icon: <BookOpen className="h-4 w-4" /> },
  { key: 'commands', label: 'Commands', icon: <Terminal className="h-4 w-4" /> },
  { key: 'shortcuts', label: 'Shortcuts', icon: <Keyboard className="h-4 w-4" /> },
];

export function HelpDialog({ commands = DEFAULT_COMMANDS, onClose }: Props): React.ReactElement {
  const [activeTab, setActiveTab] = useState<TabKey>('general');

  return (
    <div
      data-testid="help-dialog"
      className="rounded-lg border border-gray-200 bg-white shadow-lg dark:border-gray-700 dark:bg-gray-800"
    >
      <div className="flex items-center justify-between border-b border-gray-200 px-4 py-3 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <HelpCircle className="h-5 w-5 text-cyan-500" />
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Help</h3>
        </div>
        <button
          data-testid="help-close-btn"
          aria-label="Close"
          onClick={onClose}
          className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
        >
          <X className="h-5 w-5" />
        </button>
      </div>

      <div className="flex gap-1 border-b border-gray-200 px-4 dark:border-gray-700">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            data-testid={`help-tab-${tab.key}`}
            className={cn(
              'flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm font-medium transition-colors',
              activeTab === tab.key
                ? 'border-cyan-500 text-cyan-600 dark:text-cyan-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400',
            )}
            onClick={() => setActiveTab(tab.key)}
          >
            {tab.icon}
            {tab.label}
          </button>
        ))}
      </div>

      <div className="max-h-80 overflow-y-auto p-4">
        {activeTab === 'general' && (
          <div data-testid="help-general" className="flex flex-col gap-3">
            <p className="text-sm text-gray-700 dark:text-gray-300">
              Welcome to Remote Code. Use the commands below to interact with the AI assistant.
            </p>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Type <span className="font-mono text-cyan-600 dark:text-cyan-400">/</span> in the
              input to see available commands.
            </p>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Use <span className="font-mono text-cyan-600 dark:text-cyan-400">@</span> to mention
              files in your project.
            </p>
          </div>
        )}

        {activeTab === 'commands' && (
          <div data-testid="help-commands" className="flex flex-col gap-1">
            {commands.map((cmd) => (
              <div
                key={cmd.name}
                className="flex items-center justify-between rounded-md px-3 py-2 hover:bg-gray-50 dark:hover:bg-gray-700/50"
              >
                <span className="font-mono text-sm font-medium text-gray-900 dark:text-gray-100">
                  {cmd.name}
                </span>
                <span className="text-sm text-gray-500 dark:text-gray-400">
                  {cmd.description}
                </span>
              </div>
            ))}
          </div>
        )}

        {activeTab === 'shortcuts' && (
          <div data-testid="help-shortcuts" className="flex flex-col gap-1">
            {SHORTCUTS.map((s) => (
              <div
                key={s.key}
                className="flex items-center justify-between rounded-md px-3 py-2 hover:bg-gray-50 dark:hover:bg-gray-700/50"
              >
                <span className="rounded bg-gray-100 px-2 py-0.5 font-mono text-sm text-gray-700 dark:bg-gray-700 dark:text-gray-300">
                  {s.key}
                </span>
                <span className="text-sm text-gray-500 dark:text-gray-400">{s.action}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
