import { Loader2 } from 'lucide-react';
import { cn } from '../../lib/utils';
import { PromptInputFooterLeftSide } from './PromptInputFooterLeftSide';

/** PromptInputFooter 组件属性 */
export interface PromptInputFooterProps {
  /** 模型名称 */
  modelName?: string;
  /** Token 用量 */
  tokenCount?: number;
  /** 最大 Token 数 */
  maxTokens?: number;
  /** 是否加载中 */
  isLoading?: boolean;
  /** 权限模式 */
  permissionMode?: string;
  /** 额外 CSS 类名 */
  className?: string;
}

/**
 * 输入框底部状态栏。
 * 显示模型名称、Token 用量、权限模式和加载状态。
 */
export function PromptInputFooter({
  modelName,
  tokenCount,
  maxTokens,
  isLoading = false,
  permissionMode,
  className,
}: PromptInputFooterProps) {
  const tokenDisplay =
    tokenCount != null
      ? maxTokens != null
        ? `${tokenCount.toLocaleString()} / ${maxTokens.toLocaleString()}`
        : `${tokenCount.toLocaleString()} tokens`
      : null;

  return (
    <div
      className={cn(
        'flex items-center justify-between px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400',
        className,
      )}
      data-testid="prompt-input-footer"
    >
      <PromptInputFooterLeftSide
        modelName={modelName}
        permissionMode={permissionMode}
      />

      <div className="flex items-center gap-2">
        {tokenDisplay && (
          <span className="tabular-nums">{tokenDisplay}</span>
        )}
        {isLoading && (
          <Loader2 className="h-3 w-3 animate-spin" data-testid="footer-spinner" />
        )}
      </div>
    </div>
  );
}
