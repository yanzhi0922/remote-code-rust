import {
  Blocks,
  Bot,
  Brain,
  Download,
  FileText,
  FolderPlus,
  GitBranch,
  HeartPulse,
  MessageSquarePlus,
  Palette,
  RotateCcw,
  Search,
  Settings2,
  Shield,
  Square,
  Target,
  Terminal,
  Trash2,
  X,
} from 'lucide-react';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import * as tauri from '../../lib/tauri';

interface CommandItem {
  id: string;
  label: string;
  description?: string;
  icon: React.ElementType;
  iconColor?: string;
  shortcut?: string;
  category?: string;
  disabled?: boolean;
  action: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  onNewSession: () => void;
  onAddProject: () => void;
  onOpenSettings: () => void;
  onOpenMcp: () => void;
  onToggleTheme: () => void;
}

type TFn = (key: string) => string;

function buildCommands(t: TFn, callbacks: {
  onNewSession: () => void;
  onAddProject: () => void;
  onOpenSettings: () => void;
  onOpenMcp: () => void;
  onToggleTheme: () => void;
  onClose: () => void;
  onSendSlash: (command: string) => void;
  onRunDoctor: () => void;
  onExportSession: () => void;
  onExportDiagnostics: () => void;
  onStopCodex: () => void;
  onRestartCodex: () => void;
  onUpdateSettings: (updates: Record<string, unknown>) => void;
  activeSessionId: string | null;
  activeAgentType: string | null;
}): CommandItem[] {
  const hasSession = !!callbacks.activeSessionId;
  const isCodex = callbacks.activeAgentType === 'remote_codex';
  const isClaude = callbacks.activeAgentType === 'remote_claude';
  const isRoo = callbacks.activeAgentType === 'remote_roo';
  return [
    {
      id: 'new-session',
      label: t('commandPalette.newSession'),
      description: t('commandPalette.newSessionDesc'),
      icon: MessageSquarePlus,
      iconColor: 'text-rc-accent-success',
      shortcut: '\u2318N',
      category: t('commandPalette.sessionCategory'),
      action: () => { callbacks.onNewSession(); callbacks.onClose(); },
    },
    {
      id: 'add-project',
      label: t('commandPalette.addProjectFolder'),
      description: t('commandPalette.addProjectFolderDesc'),
      icon: FolderPlus,
      iconColor: 'text-rc-accent-info',
      category: t('commandPalette.projectCategory'),
      action: () => { callbacks.onAddProject(); callbacks.onClose(); },
    },
    {
      id: 'settings',
      label: t('commandPalette.settingsCmd'),
      description: t('commandPalette.settingsDesc'),
      icon: Settings2,
      shortcut: '\u2318,',
      category: t('commandPalette.settingsCategory'),
      action: () => { callbacks.onOpenSettings(); callbacks.onClose(); },
    },
    {
      id: 'goal',
      label: t('commandPalette.goal'),
      description: t('commandPalette.goalDesc'),
      icon: Target,
      iconColor: 'text-rc-accent-success',
      category: t('commandPalette.sessionCategory'),
      disabled: !hasSession || !isCodex,
      action: () => { callbacks.onSendSlash('/goal '); callbacks.onClose(); },
    },
    {
      id: 'compact',
      label: t('commandPalette.compact'),
      description: t('commandPalette.compactDesc'),
      icon: RotateCcw,
      iconColor: 'text-rc-accent-info',
      category: t('commandPalette.sessionCategory'),
      disabled: !hasSession,
      action: () => { callbacks.onSendSlash('/compact'); callbacks.onClose(); },
    },
    {
      id: 'review',
      label: t('commandPalette.review'),
      description: t('commandPalette.reviewDesc'),
      icon: FileText,
      iconColor: 'text-rc-accent-warning',
      category: t('commandPalette.sessionCategory'),
      disabled: !hasSession,
      action: () => { callbacks.onSendSlash('/review'); callbacks.onClose(); },
    },
    {
      id: 'plan-mode',
      label: t('commandPalette.planMode'),
      description: t('commandPalette.planModeDesc'),
      icon: Shield,
      iconColor: 'text-rc-accent-primary',
      category: t('commandPalette.sessionCategory'),
      action: () => {
        if (isClaude) {
          callbacks.onUpdateSettings({ permission_mode: 'plan' });
        } else {
          callbacks.onSendSlash('/plan');
        }
        callbacks.onClose();
      },
    },
    {
      id: 'clear-session',
      label: t('commandPalette.clearSession'),
      description: t('commandPalette.clearSessionDesc'),
      icon: Trash2,
      iconColor: 'text-rc-accent-error',
      category: t('commandPalette.sessionCategory'),
      disabled: !hasSession,
      action: () => { callbacks.onSendSlash('/clear'); callbacks.onClose(); },
    },
    {
      id: 'mcp',
      label: t('commandPalette.mcpManagement'),
      description: t('commandPalette.mcpManagementDesc'),
      icon: Blocks,
      iconColor: 'text-rc-accent-warning',
      category: t('commandPalette.settingsCategory'),
      action: () => { callbacks.onOpenMcp(); callbacks.onClose(); },
    },
    {
      id: 'toggle-theme',
      label: t('commandPalette.toggleTheme'),
      description: t('commandPalette.toggleThemeDesc'),
      icon: Palette,
      iconColor: 'text-rc-accent-primary',
      shortcut: '\u2318\u21E7T',
      category: t('commandPalette.settingsCategory'),
      action: () => { callbacks.onToggleTheme(); callbacks.onClose(); },
    },
    {
      id: 'terminal',
      label: t('commandPalette.openTerminal'),
      description: t('commandPalette.openTerminalDesc'),
      icon: Terminal,
      iconColor: 'text-rc-accent-warning',
      shortcut: '\u2318`',
      category: t('commandPalette.toolsCategory'),
      disabled: !hasSession || !isCodex,
      action: () => { callbacks.onSendSlash('/terminal '); callbacks.onClose(); },
    },
    {
      id: 'search-files',
      label: t('commandPalette.searchFiles'),
      description: t('commandPalette.searchFilesDesc'),
      icon: Search,
      iconColor: 'text-rc-accent-info',
      shortcut: '\u2318P',
      category: t('commandPalette.toolsCategory'),
      action: () => { callbacks.onClose(); },
    },
    {
      id: 'doctor',
      label: t('commandPalette.doctor'),
      description: t('commandPalette.doctorDesc'),
      icon: HeartPulse,
      iconColor: 'text-rc-accent-success',
      category: t('commandPalette.toolsCategory'),
      action: () => { callbacks.onRunDoctor(); callbacks.onClose(); },
    },
    {
      id: 'export-session',
      label: t('commandPalette.exportSession'),
      description: t('commandPalette.exportSessionDesc'),
      icon: Download,
      iconColor: 'text-rc-accent-info',
      category: t('commandPalette.sessionCategory'),
      disabled: !hasSession,
      action: () => { callbacks.onExportSession(); callbacks.onClose(); },
    },
    {
      id: 'export-diagnostics',
      label: t('commandPalette.exportDiagnostics'),
      description: t('commandPalette.exportDiagnosticsDesc'),
      icon: Download,
      iconColor: 'text-rc-accent-warning',
      category: t('commandPalette.toolsCategory'),
      action: () => { callbacks.onExportDiagnostics(); callbacks.onClose(); },
    },
    {
      id: 'claude-default',
      label: t('commandPalette.claudeDefault'),
      description: t('commandPalette.claudeDefaultDesc'),
      icon: Brain,
      iconColor: 'text-rc-accent-primary',
      category: t('commandPalette.claudeCategory'),
      disabled: !isClaude,
      action: () => { callbacks.onUpdateSettings({ permission_mode: 'default' }); callbacks.onClose(); },
    },
    {
      id: 'claude-safe-edit',
      label: t('commandPalette.claudeSafeEdit'),
      description: t('commandPalette.claudeSafeEditDesc'),
      icon: Shield,
      iconColor: 'text-rc-accent-success',
      category: t('commandPalette.claudeCategory'),
      disabled: !isClaude,
      action: () => { callbacks.onUpdateSettings({ permission_mode: 'acceptEdits' }); callbacks.onClose(); },
    },
    {
      id: 'claude-bypass',
      label: t('commandPalette.claudeBypass'),
      description: t('commandPalette.claudeBypassDesc'),
      icon: Shield,
      iconColor: 'text-rc-accent-error',
      category: t('commandPalette.claudeCategory'),
      disabled: !isClaude,
      action: () => { callbacks.onUpdateSettings({ permission_mode: 'bypassPermissions' }); callbacks.onClose(); },
    },
    {
      id: 'roo-code',
      label: t('commandPalette.rooCode'),
      description: t('commandPalette.rooCodeDesc'),
      icon: GitBranch,
      iconColor: 'text-rc-accent-success',
      category: t('commandPalette.rooCategory'),
      disabled: !isRoo,
      action: () => { callbacks.onUpdateSettings({ roo_mode: 'code' }); callbacks.onClose(); },
    },
    {
      id: 'roo-architect',
      label: t('commandPalette.rooArchitect'),
      description: t('commandPalette.rooArchitectDesc'),
      icon: GitBranch,
      iconColor: 'text-rc-accent-warning',
      category: t('commandPalette.rooCategory'),
      disabled: !isRoo,
      action: () => { callbacks.onUpdateSettings({ roo_mode: 'architect' }); callbacks.onClose(); },
    },
    {
      id: 'roo-debug',
      label: t('commandPalette.rooDebug'),
      description: t('commandPalette.rooDebugDesc'),
      icon: HeartPulse,
      iconColor: 'text-rc-accent-info',
      category: t('commandPalette.rooCategory'),
      disabled: !isRoo,
      action: () => { callbacks.onUpdateSettings({ roo_mode: 'debug' }); callbacks.onClose(); },
    },
    {
      id: 'roo-orchestrator',
      label: t('commandPalette.rooOrchestrator'),
      description: t('commandPalette.rooOrchestratorDesc'),
      icon: Blocks,
      iconColor: 'text-rc-accent-primary',
      category: t('commandPalette.rooCategory'),
      disabled: !isRoo,
      action: () => { callbacks.onUpdateSettings({ roo_mode: 'orchestrator' }); callbacks.onClose(); },
    },
    {
      id: 'stop-codex',
      label: t('commandPalette.stopCodex'),
      description: t('commandPalette.stopCodexDesc'),
      icon: Square,
      iconColor: 'text-rc-accent-error',
      category: t('commandPalette.codexCategory'),
      disabled: !isCodex,
      action: () => { callbacks.onStopCodex(); callbacks.onClose(); },
    },
    {
      id: 'restart-codex',
      label: t('commandPalette.restartCodex'),
      description: t('commandPalette.restartCodexDesc'),
      icon: Bot,
      iconColor: 'text-rc-accent-success',
      category: t('commandPalette.codexCategory'),
      disabled: !isCodex,
      action: () => { callbacks.onRestartCodex(); callbacks.onClose(); },
    },
  ];
}

export function CommandPalette({
  open,
  onClose,
  onNewSession,
  onAddProject,
  onOpenSettings,
  onOpenMcp,
  onToggleTheme,
}: CommandPaletteProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const addAssistantMessage = useAppStore((state) => state.addAssistantMessage);
  const injectChatAttachment = useAppStore((state) => state.injectChatAttachment);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);

  const postSystemNote = useCallback((text: string) => {
    if (activeSessionId) {
      addAssistantMessage(activeSessionId, text);
      return;
    }
    console.info('[CommandPalette]', text);
  }, [activeSessionId, addAssistantMessage]);

  const commands: CommandItem[] = useMemo(
    () => buildCommands(t, {
      onNewSession,
      onAddProject,
      onOpenSettings,
      onOpenMcp,
      onToggleTheme,
      onClose,
      activeSessionId,
      activeAgentType,
      onSendSlash: (command) => {
        if (command.endsWith(' ')) {
          injectChatAttachment(command);
        } else {
          void useAppStore.getState().sendMessage(command);
        }
      },
      onRunDoctor: () => {
        void tauri.runDoctorReport(true, true, true, true)
          .then((report) => {
            const issues = report.issues.length;
            const warnings = report.warnings.length;
            postSystemNote(`Doctor ${report.ok ? 'passed' : 'found issues'}: ${issues} issues, ${warnings} warnings.`);
          })
          .catch((error) => postSystemNote(`Doctor failed: ${String(error)}`));
      },
      onExportSession: () => {
        if (!activeSessionId) return;
        void tauri.exportSessionBundle(activeSessionId, 'json')
          .then((result) => postSystemNote(`Session exported to ${result.path}`))
          .catch((error) => postSystemNote(`Session export failed: ${String(error)}`));
      },
      onExportDiagnostics: () => {
        void tauri.exportDiagnosticBundle({ includeLogs: true, includeSettings: true })
          .then((result) => postSystemNote(`Diagnostic bundle exported to ${result.path}`))
          .catch((error) => postSystemNote(`Diagnostic export failed: ${String(error)}`));
      },
      onStopCodex: () => {
        void tauri.codexAdapterStop(activeSessionId)
          .then(() => postSystemNote('Codex adapter stopped.'))
          .catch((error) => postSystemNote(`Failed to stop Codex adapter: ${String(error)}`));
      },
      onRestartCodex: () => {
        void tauri.codexAdapterRestart(activeSessionId)
          .then(() => postSystemNote('Codex adapter restarted.'))
          .catch((error) => postSystemNote(`Failed to restart Codex adapter: ${String(error)}`));
      },
      onUpdateSettings: (updates) => {
        void useAppStore.getState().updateSettings(updates)
          .catch((error) => postSystemNote(`Failed to update settings: ${String(error)}`));
      },
    }),
    [
      t,
      onNewSession,
      onAddProject,
      onOpenSettings,
      onOpenMcp,
      onToggleTheme,
      onClose,
      activeSessionId,
      activeAgentType,
      injectChatAttachment,
      postSystemNote,
    ],
  );

  const filteredCommands = useMemo(() => {
    if (!query.trim()) return commands;
    const q = query.toLowerCase();
    return commands.filter(
      (cmd) =>
        cmd.label.toLowerCase().includes(q) ||
        (cmd.description ?? '').toLowerCase().includes(q) ||
        (cmd.category ?? '').toLowerCase().includes(q),
    );
  }, [commands, query]);

  useEffect(() => {
    if (open) {
      setQuery('');
      setSelectedIndex(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  const executeSelected = useCallback(() => {
    const cmd = filteredCommands[selectedIndex];
    if (cmd && !cmd.disabled) cmd.action();
  }, [filteredCommands, selectedIndex]);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      switch (event.key) {
        case 'ArrowDown':
          event.preventDefault();
          setSelectedIndex((prev) =>
            prev < filteredCommands.length - 1 ? prev + 1 : 0,
          );
          break;
        case 'ArrowUp':
          event.preventDefault();
          setSelectedIndex((prev) =>
            prev > 0 ? prev - 1 : filteredCommands.length - 1,
          );
          break;
        case 'Enter':
          event.preventDefault();
          executeSelected();
          break;
        case 'Escape':
          event.preventDefault();
          onClose();
          break;
      }
    },
    [filteredCommands.length, executeSelected, onClose],
  );

  useEffect(() => {
    const scrollToSelected = () => {
      const listEl = listRef.current;
      if (!listEl) return;
      const selected = listEl.querySelector('[data-selected="true"]');
      if (selected) {
        selected.scrollIntoView({ block: 'nearest' });
      }
    };
    scrollToSelected();
  }, [selectedIndex]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]">
      <div
        className="fixed inset-0 bg-rc-bg-overlay animate-fade-in"
        onClick={onClose}
      />
      <div className="relative w-full max-w-[560px] animate-scale-in rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-sm overflow-hidden">
        <div className="flex items-center gap-3 border-b border-rc-border-secondary px-4 py-3">
          <Search size={16} className="shrink-0 text-rc-text-tertiary" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('commandPalette.searchPlaceholder')}
            className="flex-1 bg-transparent text-sm text-rc-text-primary outline-none placeholder:text-rc-text-tertiary"
          />
          <button
            type="button"
            onClick={onClose}
            className="flex h-6 items-center gap-1 rounded border border-rc-border-primary bg-rc-bg-tertiary px-1.5 text-[10px] text-rc-text-tertiary"
          >
            Esc
          </button>
        </div>

        <div ref={listRef} className="max-h-[320px] overflow-y-auto py-1">
          {filteredCommands.length === 0 ? (
            <div className="px-4 py-6 text-center text-sm text-rc-text-tertiary">
              {t('commandPalette.noCommandFound')}
            </div>
          ) : (
            filteredCommands.map((cmd, index) => (
              <button
                key={cmd.id}
                data-selected={index === selectedIndex}
                disabled={cmd.disabled}
                className={cn(
                  'flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-45',
                  index === selectedIndex && !cmd.disabled
                    ? 'bg-rc-bg-selected text-rc-text-primary'
                    : 'text-rc-text-primary hover:bg-rc-bg-hover',
                )}
                onClick={cmd.action}
                onMouseEnter={() => setSelectedIndex(index)}
              >
                <cmd.icon size={16} className={cn('shrink-0', cmd.iconColor ?? 'text-rc-text-tertiary')} />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{cmd.label}</div>
                  {cmd.description && (
                    <div className="text-[11px] text-rc-text-tertiary">{cmd.description}</div>
                  )}
                </div>
                {cmd.shortcut && (
                  <span className="shrink-0 rounded border border-rc-border-primary bg-rc-bg-tertiary px-1.5 py-0.5 text-[10px] font-mono text-rc-text-tertiary">
                    {cmd.shortcut}
                  </span>
                )}
              </button>
            ))
          )}
        </div>

        <div className="flex items-center gap-4 border-t border-rc-border-secondary px-4 py-2 text-[10px] text-rc-text-tertiary">
          <span>{t('commandPalette.navUpdown')}</span>
          <span>{t('commandPalette.navEnter')}</span>
          <span>{t('commandPalette.navEsc')}</span>
        </div>
      </div>
    </div>
  );
}
