import { X } from 'lucide-react';
import { cn } from '../../lib/utils';

/** PromptInputHelpMenu 组件属性 */
export interface PromptInputHelpMenuProps {
  /** 是否可见 */
  visible: boolean;
  /** 关闭回调 */
  onClose: () => void;
  /** 额外 CSS 类名 */
  className?: string;
}

/** 快捷键列表 */
const SHORTCUTS = [
  { key: 'Enter', description: '发送消息' },
  { key: 'Shift+Enter', description: '换行' },
  { key: '!command', description: 'Bash 模式' },
  { key: '/command', description: '斜杠命令' },
  { key: 'Ctrl+C', description: '取消' },
  { key: 'Ctrl+L', description: '清屏' },
];

/**
 * 帮助菜单覆盖层。
 * 显示快捷键列表，visible=false 时返回 null。
 */
export function PromptInputHelpMenu({
  visible,
  onClose,
  className,
}: PromptInputHelpMenuProps) {
  if (!visible) return null;

  return (
    <div
      className={cn(
        'fixed inset-0 z-50 flex items-center justify-center bg-black/40',
        className,
      )}
      data-testid="prompt-help-menu"
      onClick={onClose}
    >
      <div
        className="w-80 rounded-lg border border-slate-200 bg-white p-4 shadow-xl dark:border-slate-700 dark:bg-slate-800"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-sm font-semibold text-slate-900 dark:text-slate-100">
            快捷键
          </h3>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-700"
            aria-label="关闭"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <ul className="space-y-2">
          {SHORTCUTS.map((shortcut) => (
            <li
              key={shortcut.key}
              className="flex items-center justify-between text-sm"
            >
              <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 font-mono text-xs text-slate-600 dark:border-slate-600 dark:bg-slate-700 dark:text-slate-300">
                {shortcut.key}
              </kbd>
              <span className="text-slate-600 dark:text-slate-400">
                {shortcut.description}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
