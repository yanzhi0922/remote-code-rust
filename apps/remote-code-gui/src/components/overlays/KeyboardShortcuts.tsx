interface ShortcutEntry {
  keys: string;
  description: string;
  group: string;
}

const shortcuts: ShortcutEntry[] = [
  { keys: 'Ctrl+`', description: '切换终端面板', group: '面板' },
  { keys: 'Cmd+Shift+P', description: '命令面板', group: '面板' },
  { keys: 'Cmd+Shift+E', description: '切换文件树', group: '面板' },
  { keys: 'Cmd+N', description: '新建会话', group: '会话' },
  { keys: 'Cmd+W', description: '关闭当前面板', group: '面板' },
  { keys: 'Cmd+\\', description: '水平分割面板', group: '面板' },
  { keys: 'Cmd+Shift+\\', description: '垂直分割面板', group: '面板' },
  { keys: 'Enter', description: '发送消息', group: '聊天' },
  { keys: 'Shift+Enter', description: '换行', group: '聊天' },
  { keys: 'Escape', description: '取消/关闭', group: '通用' },
];

interface KeyboardShortcutsProps {
  open: boolean;
  onClose: () => void;
}

export function KeyboardShortcuts({ open, onClose }: KeyboardShortcutsProps) {
  if (!open) return null;

  const groups = [...new Set(shortcuts.map((s) => s.group))];

  return (
    <div className="fixed inset-0 z-modal flex items-center justify-center bg-rc-bg-overlay" onClick={onClose}>
      <div
        className="w-full max-w-md rounded-xl border border-rc-border-primary bg-rc-bg-primary shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="border-b border-rc-border-primary px-4 py-3">
          <h2 className="text-sm font-semibold text-rc-text-primary">键盘快捷键</h2>
        </div>
        <div className="max-h-96 overflow-y-auto p-4 space-y-4">
          {groups.map((group) => (
            <div key={group}>
              <div className="text-xs font-semibold uppercase tracking-wider text-rc-text-tertiary mb-2">{group}</div>
              <div className="space-y-1.5">
                {shortcuts
                  .filter((s) => s.group === group)
                  .map((shortcut) => (
                    <div key={shortcut.keys} className="flex items-center justify-between">
                      <span className="text-sm text-rc-text-primary">{shortcut.description}</span>
                      <kbd className="rounded border border-rc-border-primary bg-rc-bg-secondary px-2 py-0.5 text-xs text-rc-text-secondary font-mono">
                        {shortcut.keys}
                      </kbd>
                    </div>
                  ))}
              </div>
            </div>
          ))}
        </div>
        <div className="border-t border-rc-border-primary px-4 py-2">
          <button
            onClick={onClose}
            className="text-xs text-rc-text-tertiary hover:text-rc-text-primary transition-colors"
          >
            按 ESC 或点击外部关闭
          </button>
        </div>
      </div>
    </div>
  );
}
