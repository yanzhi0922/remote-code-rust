import { Terminal } from 'lucide-react';
import { cn } from '../../lib/utils';

/** PromptInputModeIndicator 组件属性 */
export interface PromptInputModeIndicatorProps {
  /** 当前输入模式 */
  mode: 'prompt' | 'bash' | 'vim-normal' | 'vim-insert';
  /** 额外 CSS 类名 */
  className?: string;
}

/** 模式配置映射 */
const MODE_CONFIG: Record<
  string,
  { label: string; colorClass: string; showIcon: boolean } | null
> = {
  prompt: null,
  bash: {
    label: 'BASH',
    colorClass:
      'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300',
    showIcon: true,
  },
  'vim-normal': {
    label: 'NORMAL',
    colorClass:
      'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300',
    showIcon: false,
  },
  'vim-insert': {
    label: 'INSERT',
    colorClass:
      'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300',
    showIcon: false,
  },
};

/**
 * 输入模式指示器。
 * prompt 模式下不显示，其他模式显示对应标签。
 */
export function PromptInputModeIndicator({
  mode,
  className,
}: PromptInputModeIndicatorProps) {
  const config = MODE_CONFIG[mode];
  if (!config) return null;

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-xs font-semibold',
        config.colorClass,
        className,
      )}
      data-testid="prompt-mode-indicator"
    >
      {config.showIcon && <Terminal className="h-3 w-3" />}
      {config.label}
    </span>
  );
}
