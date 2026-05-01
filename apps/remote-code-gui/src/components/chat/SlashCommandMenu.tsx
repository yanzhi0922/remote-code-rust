import { useEffect, useRef, useState } from 'react';

export interface SlashCommand {
  id: string;
  label: string;
  description: string;
  icon?: string;
}

interface SlashCommandMenuProps {
  open: boolean;
  filter: string;
  commands: SlashCommand[];
  onSelect: (command: SlashCommand) => void;
  onClose: () => void;
}

export function SlashCommandMenu({ open, filter, commands, onSelect, onClose }: SlashCommandMenuProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const menuRef = useRef<HTMLDivElement>(null);

  const filtered = commands.filter(
    (cmd) =>
      cmd.label.toLowerCase().includes(filter.toLowerCase()) ||
      cmd.description.toLowerCase().includes(filter.toLowerCase()),
  );

  useEffect(() => {
    setSelectedIndex(0);
  }, [filter]);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((i) => (i + 1) % filtered.length);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((i) => (i - 1 + filtered.length) % filtered.length);
      } else if (e.key === 'Enter' && filtered[selectedIndex]) {
        e.preventDefault();
        onSelect(filtered[selectedIndex]);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [open, filtered, selectedIndex, onSelect, onClose]);

  if (!open || filtered.length === 0) return null;

  return (
    <div
      ref={menuRef}
      className="absolute bottom-full left-0 right-0 z-20 mb-2 max-h-64 overflow-y-auto rounded-xl border border-rc-border-primary bg-rc-bg-primary shadow-xl"
    >
      <div className="p-1.5">
        {filtered.map((cmd, index) => (
          <button
            key={cmd.id}
            onClick={() => onSelect(cmd)}
            className={`flex w-full items-start gap-2 rounded-lg px-3 py-2 text-left transition-colors ${
              index === selectedIndex ? 'bg-rc-bg-active text-rc-text-primary' : 'text-rc-text-primary hover:bg-rc-bg-hover'
            }`}
          >
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                {cmd.icon && <span className="text-sm">{cmd.icon}</span>}
                <span className="text-sm font-medium">{cmd.label}</span>
              </div>
              <div className="mt-0.5 text-xs text-rc-text-secondary">{cmd.description}</div>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

// Default slash commands
export const DEFAULT_SLASH_COMMANDS: SlashCommand[] = [
  { id: 'compact', label: '/compact', description: '压缩对话上下文以释放 token 空间', icon: '📦' },
  { id: 'clear', label: '/clear', description: '清除当前对话历史', icon: '🧹' },
  { id: 'model', label: '/model', description: '切换模型', icon: '🤖' },
  { id: 'mode', label: '/mode', description: '切换权限模式', icon: '🛡️' },
  { id: 'help', label: '/help', description: '显示帮助信息', icon: '❓' },
  { id: 'doctor', label: '/doctor', description: '运行诊断检查', icon: '🩺' },
];
