import { memo, useCallback, useState } from 'react';
import { Copy, RotateCcw, Pencil, Check, ChevronDown, ChevronUp } from 'lucide-react';
import { cn } from '../lib/utils';

/** 消息操作按钮属性 */
export interface MessageActionsProps {
  /** 消息文本内容（用于复制） */
  text: string;
  /** 消息 ID */
  messageId?: string;
  /** 是否可以重新发送 */
  canResend?: boolean;
  /** 是否可以编辑 */
  canEdit?: boolean;
  /** 重新发送回调 */
  onResend?: (messageId: string) => void;
  /** 编辑回调 */
  onEdit?: (messageId: string) => void;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 消息操作按钮组件。
 * 提供复制、重新发送、编辑等操作。
 */
export const MessageActions = memo(function MessageActions({
  text,
  messageId,
  canResend = false,
  canEdit = false,
  onResend,
  onEdit,
  className,
}: MessageActionsProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // 剪贴板 API 不可用时的降级处理
    }
  }, [text]);

  const handleResend = useCallback(() => {
    if (messageId && onResend) {
      onResend(messageId);
    }
  }, [messageId, onResend]);

  const handleEdit = useCallback(() => {
    if (messageId && onEdit) {
      onEdit(messageId);
    }
  }, [messageId, onEdit]);

  return (
    <div
      className={cn(
        'flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100',
        className,
      )}
    >
      {/* 复制按钮 */}
      <button
        type="button"
        onClick={handleCopy}
        className="rounded-md p-1.5 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-700 dark:hover:text-slate-300"
        title={copied ? '已复制' : '复制'}
        aria-label={copied ? '已复制' : '复制'}
      >
        {copied ? (
          <Check className="h-3.5 w-3.5 text-emerald-500" />
        ) : (
          <Copy className="h-3.5 w-3.5" />
        )}
      </button>

      {/* 重新发送按钮 */}
      {canResend && (
        <button
          type="button"
          onClick={handleResend}
          className="rounded-md p-1.5 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-700 dark:hover:text-slate-300"
          title="重新发送"
          aria-label="重新发送"
        >
          <RotateCcw className="h-3.5 w-3.5" />
        </button>
      )}

      {/* 编辑按钮 */}
      {canEdit && (
        <button
          type="button"
          onClick={handleEdit}
          className="rounded-md p-1.5 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-700 dark:hover:text-slate-300"
          title="编辑"
          aria-label="编辑"
        >
          <Pencil className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
});

/** 折叠/展开按钮属性 */
export interface ToggleCollapseProps {
  /** 是否已展开 */
  isExpanded: boolean;
  /** 切换回调 */
  onToggle: () => void;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 折叠/展开切换按钮。
 */
export const ToggleCollapse = memo(function ToggleCollapse({
  isExpanded,
  onToggle,
  className,
}: ToggleCollapseProps) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className={cn(
        'rounded-md p-1 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-700 dark:hover:text-slate-300',
        className,
      )}
      title={isExpanded ? '收起' : '展开'}
      aria-label={isExpanded ? '收起' : '展开'}
      aria-expanded={isExpanded}
    >
      {isExpanded ? (
        <ChevronUp className="h-3.5 w-3.5" />
      ) : (
        <ChevronDown className="h-3.5 w-3.5" />
      )}
    </button>
  );
});
