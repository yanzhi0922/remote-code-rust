import { memo } from 'react';
import { CheckCircle2, XCircle, Ban, Clock, AlertTriangle, FileX2 } from 'lucide-react';
import type { ConversationEntry } from '../../lib/types';
import { cn, truncateMiddle } from '../../lib/utils';

/** 工具结果消息的基础属性 */
interface ToolResultBaseProps {
  /** 工具名称 */
  toolName: string;
  /** 工具输出文本 */
  output: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/** 工具成功结果消息属性 */
export interface UserToolSuccessMessageProps extends ToolResultBaseProps {
  /** 工具调用 ID */
  toolCallId?: string;
}

/** 工具错误结果消息属性 */
export interface UserToolErrorMessageProps extends ToolResultBaseProps {}

/** 工具被拒绝消息属性 */
export interface UserToolRejectMessageProps extends ToolResultBaseProps {
  /** 拒绝原因 */
  reason?: string;
}

/** 工具被取消消息属性 */
export interface UserToolCanceledMessageProps extends ToolResultBaseProps {}

/** 工具使用被拒绝消息属性 */
export interface RejectedToolUseMessageProps {
  /** 工具名称 */
  toolName: string;
  /** 工具输入 */
  input: unknown;
  /** 额外的 CSS 类名 */
  className?: string;
}

/** 计划被拒绝消息属性 */
export interface RejectedPlanMessageProps {
  /** 计划内容 */
  planContent: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 工具成功结果消息 — 绿色 ✓ 图标
 */
export const UserToolSuccessMessage = memo(function UserToolSuccessMessage({
  toolName,
  output,
  className,
}: UserToolSuccessMessageProps) {
  return (
    <div
      className={cn(
        'rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 dark:border-emerald-800 dark:bg-emerald-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-600 dark:text-emerald-400" />
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wider text-emerald-700 dark:text-emerald-400">
              工具成功
            </span>
            <span className="rounded-full bg-emerald-100 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:bg-emerald-900 dark:text-emerald-300">
              {toolName}
            </span>
          </div>
          <pre className="overflow-x-auto whitespace-pre-wrap text-xs leading-5 text-slate-700 dark:text-slate-300">
            {truncateMiddle(output, 500)}
          </pre>
        </div>
      </div>
    </div>
  );
});

/**
 * 工具错误结果消息 — 红色 ✗ 图标
 */
export const UserToolErrorMessage = memo(function UserToolErrorMessage({
  toolName,
  output,
  className,
}: UserToolErrorMessageProps) {
  return (
    <div
      className={cn(
        'rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 dark:border-rose-800 dark:bg-rose-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <XCircle className="mt-0.5 h-4 w-4 shrink-0 text-rose-600 dark:text-rose-400" />
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wider text-rose-700 dark:text-rose-400">
              工具错误
            </span>
            <span className="rounded-full bg-rose-100 px-2 py-0.5 text-[11px] font-medium text-rose-700 dark:bg-rose-900 dark:text-rose-300">
              {toolName}
            </span>
          </div>
          <pre className="overflow-x-auto whitespace-pre-wrap text-xs leading-5 text-[#9c2f2f] dark:text-rose-300">
            {truncateMiddle(output, 500)}
          </pre>
        </div>
      </div>
    </div>
  );
});

/**
 * 工具被拒绝消息 — 禁止图标
 */
export const UserToolRejectMessage = memo(function UserToolRejectMessage({
  toolName,
  output,
  reason,
  className,
}: UserToolRejectMessageProps) {
  return (
    <div
      className={cn(
        'rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 dark:border-amber-800 dark:bg-amber-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <Ban className="mt-0.5 h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" />
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wider text-amber-700 dark:text-amber-400">
              工具被拒绝
            </span>
            <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[11px] font-medium text-amber-700 dark:bg-amber-900 dark:text-amber-300">
              {toolName}
            </span>
          </div>
          {reason && (
            <p className="mb-1 text-xs text-amber-700 dark:text-amber-300">{reason}</p>
          )}
          <pre className="overflow-x-auto whitespace-pre-wrap text-xs leading-5 text-slate-600 dark:text-slate-400">
            {truncateMiddle(output, 300)}
          </pre>
        </div>
      </div>
    </div>
  );
});

/**
 * 工具被取消消息 — 时钟图标
 */
export const UserToolCanceledMessage = memo(function UserToolCanceledMessage({
  toolName,
  output,
  className,
}: UserToolCanceledMessageProps) {
  return (
    <div
      className={cn(
        'rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 dark:border-slate-700 dark:bg-slate-800/50',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <Clock className="mt-0.5 h-4 w-4 shrink-0 text-slate-500 dark:text-slate-400" />
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
              工具已取消
            </span>
            <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:bg-slate-700 dark:text-slate-300">
              {toolName}
            </span>
          </div>
          {output && (
            <pre className="overflow-x-auto whitespace-pre-wrap text-xs leading-5 text-slate-500 dark:text-slate-400">
              {truncateMiddle(output, 300)}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
});

/**
 * 工具使用被拒绝消息 — 警告图标
 */
export const RejectedToolUseMessage = memo(function RejectedToolUseMessage({
  toolName,
  input,
  className,
}: RejectedToolUseMessageProps) {
  return (
    <div
      className={cn(
        'rounded-xl border border-orange-200 bg-orange-50 px-4 py-3 dark:border-orange-800 dark:bg-orange-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-orange-600 dark:text-orange-400" />
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wider text-orange-700 dark:text-orange-400">
              工具使用被拒绝
            </span>
            <span className="rounded-full bg-orange-100 px-2 py-0.5 text-[11px] font-medium text-orange-700 dark:bg-orange-900 dark:text-orange-300">
              {toolName}
            </span>
          </div>
          <pre className="overflow-x-auto whitespace-pre-wrap text-xs leading-5 text-slate-600 dark:text-slate-400">
            {typeof input === 'string' ? input : JSON.stringify(input, null, 2)}
          </pre>
        </div>
      </div>
    </div>
  );
});

/**
 * 计划被拒绝消息 — 文件拒绝图标
 */
export const RejectedPlanMessage = memo(function RejectedPlanMessage({
  planContent,
  className,
}: RejectedPlanMessageProps) {
  return (
    <div
      className={cn(
        'rounded-xl border border-violet-200 bg-violet-50 px-4 py-3 dark:border-violet-800 dark:bg-violet-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <FileX2 className="mt-0.5 h-4 w-4 shrink-0 text-violet-600 dark:text-violet-400" />
        <div className="min-w-0 flex-1">
          <div className="mb-1">
            <span className="text-xs font-semibold uppercase tracking-wider text-violet-700 dark:text-violet-400">
              计划被拒绝
            </span>
          </div>
          <pre className="overflow-x-auto whitespace-pre-wrap text-xs leading-5 text-slate-600 dark:text-slate-400">
            {planContent}
          </pre>
        </div>
      </div>
    </div>
  );
});

/** 工具结果消息统一组件 — 根据 entry.is_error 和其他条件自动选择子组件 */
export interface UserToolResultMessageProps {
  /** 对话条目 */
  entry: ConversationEntry;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 工具执行结果消息统一入口。
 * 根据 `entry.is_error` 自动选择成功或错误样式。
 */
export const UserToolResultMessage = memo(function UserToolResultMessage({
  entry,
  className,
}: UserToolResultMessageProps) {
  const toolName = entry.name ?? 'tool';

  if (entry.is_error) {
    return <UserToolErrorMessage toolName={toolName} output={entry.text} className={className} />;
  }

  return <UserToolSuccessMessage toolName={toolName} output={entry.text} className={className} />;
});
