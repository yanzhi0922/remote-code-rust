import { memo } from 'react';
import { AlertTriangle, AlertCircle, Info, Minimize2 } from 'lucide-react';
import type { ConversationEntry } from '../../lib/types';
import { cn } from '../../lib/utils';

/** 系统文本消息组件属性 */
export interface SystemTextMessageProps {
  /** 对话条目 */
  entry: ConversationEntry;
  /** 是否显示详细信息 */
  verbose?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

type SystemKind = 'compacted' | 'error' | 'warning' | 'default';

/**
 * 检测系统消息子类型。
 */
function detectSystemKind(text: string): SystemKind {
  const lower = text.toLowerCase();
  if (lower.includes('compacted') || lower.includes('compaction')) {
    return 'compacted';
  }
  if (lower.includes('error')) {
    return 'error';
  }
  if (lower.includes('warning')) {
    return 'warning';
  }
  return 'default';
}

/**
 * 系统文本消息渲染组件。
 * 根据文本内容自动检测子类型并应用对应样式。
 */
export const SystemTextMessage = memo(function SystemTextMessage({
  entry,
  verbose = false,
  className,
}: SystemTextMessageProps) {
  const kind = detectSystemKind(entry.text);

  return (
    <div
      data-testid="system-text-message"
      className={cn(
        'rounded-lg border px-4 py-3 text-xs',
        kind === 'compacted' && 'border-slate-300 bg-slate-100 text-slate-600 dark:border-slate-600 dark:bg-slate-800/50 dark:text-slate-400',
        kind === 'error' && 'border-red-200 bg-red-50 text-red-700 dark:border-red-800 dark:bg-red-950/30 dark:text-red-400',
        kind === 'warning' && 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-800 dark:bg-amber-950/30 dark:text-amber-400',
        kind === 'default' && 'border-slate-200 bg-slate-50 text-slate-500 dark:border-slate-700 dark:bg-slate-800/30 dark:text-slate-400',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        {kind === 'compacted' && (
          <Minimize2 className="mt-0.5 h-3.5 w-3.5 shrink-0" />
        )}
        {kind === 'error' && (
          <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
        )}
        {kind === 'warning' && (
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
        )}
        {kind === 'default' && (
          <Info className="mt-0.5 h-3.5 w-3.5 shrink-0" />
        )}
        <span className="whitespace-pre-wrap break-words leading-5">
          {verbose ? entry.text : entry.text.slice(0, 200)}
        </span>
      </div>
    </div>
  );
});
