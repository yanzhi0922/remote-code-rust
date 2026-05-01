import { useEffect, useRef, useState } from 'react';
import { Search } from 'lucide-react';

export interface CommandItem {
  id: string;
  label: string;
  description?: string;
  shortcut?: string;
  icon?: string;
  action: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  commands: CommandItem[];
}

export function CommandPalette({ open, onClose, commands }: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const filtered = commands.filter(
    (cmd) =>
      cmd.label.toLowerCase().includes(query.toLowerCase()) ||
      (cmd.description?.toLowerCase().includes(query.toLowerCase()) ?? false),
  );

  useEffect(() => {
    if (open) {
      setQuery('');
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && filtered[selectedIndex]) {
        e.preventDefault();
        filtered[selectedIndex].action();
        onClose();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [open, filtered, selectedIndex, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-modal flex items-start justify-center pt-[20vh]" onClick={onClose}>
      <div
        className="w-full max-w-lg rounded-xl border border-rc-border-primary bg-rc-bg-primary shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search input */}
        <div className="flex items-center gap-2 border-b border-rc-border-primary px-4 py-3">
          <Search size={16} className="text-rc-text-tertiary shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="输入命令或搜索..."
            className="flex-1 bg-transparent text-sm text-rc-text-primary outline-none placeholder:text-rc-text-tertiary"
          />
          <kbd className="rounded border border-rc-border-primary px-1.5 py-0.5 text-2xs text-rc-text-tertiary">ESC</kbd>
        </div>

        {/* Command list */}
        <div className="max-h-72 overflow-y-auto p-1.5">
          {filtered.length === 0 ? (
            <div className="px-3 py-4 text-center text-sm text-rc-text-tertiary">
              没有匹配的命令
            </div>
          ) : (
            filtered.map((cmd, index) => (
              <button
                key={cmd.id}
                onClick={() => {
                  cmd.action();
                  onClose();
                }}
                onMouseEnter={() => setSelectedIndex(index)}
                className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors ${
                  index === selectedIndex ? 'bg-rc-bg-active text-rc-text-primary' : 'text-rc-text-primary hover:bg-rc-bg-hover'
                }`}
              >
                {cmd.icon && <span className="text-sm shrink-0">{cmd.icon}</span>}
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium truncate">{cmd.label}</div>
                  {cmd.description && (
                    <div className="text-xs text-rc-text-secondary truncate">{cmd.description}</div>
                  )}
                </div>
                {cmd.shortcut && (
                  <kbd className="shrink-0 rounded border border-rc-border-primary px-1.5 py-0.5 text-2xs text-rc-text-tertiary">
                    {cmd.shortcut}
                  </kbd>
                )}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
