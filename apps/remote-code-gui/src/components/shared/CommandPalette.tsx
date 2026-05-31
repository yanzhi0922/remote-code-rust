import {
  Blocks,
  FileText,
  FolderPlus,
  MessageSquarePlus,
  Moon,
  Palette,
  Search,
  Settings2,
  Sun,
  Terminal,
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

interface CommandItem {
  id: string;
  label: string;
  description?: string;
  icon: React.ElementType;
  iconColor?: string;
  shortcut?: string;
  category?: string;
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
}): CommandItem[] {
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
      action: () => { callbacks.onClose(); },
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

  const commands: CommandItem[] = useMemo(
    () => buildCommands(t, { onNewSession, onAddProject, onOpenSettings, onOpenMcp, onToggleTheme, onClose }),
    [t, onNewSession, onAddProject, onOpenSettings, onOpenMcp, onToggleTheme, onClose],
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
    if (cmd) cmd.action();
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
      <div className="relative w-full max-w-[560px] animate-scale-in rounded-xl border border-rc-border-primary bg-rc-bg-surface shadow-2xl overflow-hidden">
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
                className={cn(
                  'flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors',
                  index === selectedIndex
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
