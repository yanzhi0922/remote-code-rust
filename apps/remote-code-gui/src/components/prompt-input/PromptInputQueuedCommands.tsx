import { X } from 'lucide-react';
import { cn } from '../../lib/utils';

/** PromptInputQueuedCommands 组件属性 */
export interface PromptInputQueuedCommandsProps {
  /** 排队命令列表 */
  commands: string[];
  /** 删除命令回调 */
  onRemove: (index: number) => void;
  /** 额外 CSS 类名 */
  className?: string;
}

/**
 * 排队命令列表。
 * 显示等待发送的命令，每条命令可单独删除。
 */
export function PromptInputQueuedCommands({
  commands,
  onRemove,
  className,
}: PromptInputQueuedCommandsProps) {
  if (commands.length === 0) return null;

  return (
    <div
      className={cn(
        'flex flex-wrap gap-1.5 px-3 pt-2',
        className,
      )}
      data-testid="prompt-queued-commands"
    >
      {commands.map((cmd, index) => (
        <span
          key={index}
          className="inline-flex items-center gap-1 rounded-md bg-amber-50 px-2 py-0.5 text-xs text-amber-700 dark:bg-amber-900/30 dark:text-amber-300"
        >
          ⏳ {cmd}
          <button
            type="button"
            onClick={() => onRemove(index)}
            className="ml-0.5 text-amber-400 hover:text-amber-600"
            aria-label={`移除命令: ${cmd}`}
          >
            <X className="h-3 w-3" />
          </button>
        </span>
      ))}
    </div>
  );
}
