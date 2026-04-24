import { Sparkles } from 'lucide-react';
import { cn } from '../../lib/utils';

/** PromptInputFooterLeftSide 组件属性 */
export interface PromptInputFooterLeftSideProps {
  /** 模型名称 */
  modelName?: string;
  /** 权限模式 */
  permissionMode?: string;
  /** 额外 CSS 类名 */
  className?: string;
}

/**
 * 底部状态栏左侧。
 * 显示模型名称徽章和权限模式标签。
 */
export function PromptInputFooterLeftSide({
  modelName,
  permissionMode,
  className,
}: PromptInputFooterLeftSideProps) {
  return (
    <div
      className={cn('flex items-center gap-2', className)}
      data-testid="prompt-footer-left"
    >
      {modelName && (
        <span className="inline-flex items-center gap-1 rounded-md bg-slate-100 px-1.5 py-0.5 text-xs font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300">
          <Sparkles className="h-3 w-3" />
          {modelName}
        </span>
      )}
      {permissionMode && (
        <span className="rounded-md bg-emerald-50 px-1.5 py-0.5 text-xs text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300">
          {permissionMode}
        </span>
      )}
    </div>
  );
}
