import { lazy, memo, Suspense, useMemo } from 'react';
import { ChevronRight, Wrench, Brain, FileText } from 'lucide-react';
import type { ConversationEntry, ToolCallInfo } from '../lib/types';
import { cn, truncateMiddle } from '../lib/utils';
import CollapsibleBlock from './chat/CollapsibleBlock';

const LazyMarkdownRenderer = lazy(() => import('./chat/MarkdownRenderer'));

/** 助手响应消息属性 */
export interface MessageResponseProps {
  /** 对话条目 */
  entry: ConversationEntry;
  /** 是否为 transcript 模式 */
  isTranscriptMode?: boolean;
  /** 是否使用紧凑样式 */
  compact?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

/** 从 content_blocks 中提取 thinking 块 */
function extractThinkingBlocks(entry: ConversationEntry): string[] {
  return entry.content_blocks
    .filter((block): block is Record<string, unknown> => !!block && typeof block === 'object')
    .filter((block) => block.type === 'thinking' && typeof block.thinking === 'string')
    .map((block) => block.thinking as string);
}

/** 格式化工具输入为可读字符串 */
function formatToolInput(input: unknown): string {
  try {
    const normalized = typeof input === 'string' ? JSON.parse(input) : input;
    return JSON.stringify(normalized, null, 2);
  } catch {
    return typeof input === 'string' ? input : JSON.stringify(input, null, 2);
  }
}

/** 摘要工具输入 */
function summarizeToolInput(toolCall: ToolCallInfo): string {
  try {
    const normalized =
      typeof toolCall.input === 'string' ? JSON.parse(toolCall.input) : toolCall.input;
    if (normalized && typeof normalized === 'object') {
      const objectValue = normalized as Record<string, unknown>;
      const preview =
        objectValue.path ??
        objectValue.file_path ??
        objectValue.command ??
        objectValue.query ??
        objectValue.prompt ??
        Object.values(objectValue)[0];
      if (typeof preview === 'string') {
        return truncateMiddle(preview, 84);
      }
    }
  } catch {
    // 忽略摘要解析失败
  }
  return toolCall.name;
}

/** Thinking 块渲染 */
function ThinkingSection({ blocks }: { blocks: string[] }) {
  if (blocks.length === 0) return null;

  return (
    <div className="mb-4 space-y-2">
      {blocks.map((block, index) => (
        <CollapsibleBlock
          key={`thinking-${index}-${block.slice(0, 20)}`}
          summary={
            <div className="flex min-w-0 items-center gap-2">
              <Brain className="h-3.5 w-3.5 text-amber-600 dark:text-amber-400" />
              <span className="text-xs font-semibold uppercase tracking-[0.16em] text-amber-700 dark:text-amber-400">
                思考
              </span>
              <span className="truncate text-sm text-slate-500 dark:text-slate-400">
                {truncateMiddle(block.replace(/\s+/g, ' ').trim(), 60)}
              </span>
            </div>
          }
          iconColor="text-amber-600"
        >
          <div className="whitespace-pre-wrap text-sm leading-7 text-slate-700 dark:text-slate-300">
            {block}
          </div>
        </CollapsibleBlock>
      ))}
    </div>
  );
}

/** 工具调用渲染 */
function ToolCallsSection({ toolCalls }: { toolCalls: ToolCallInfo[] }) {
  if (toolCalls.length === 0) return null;

  return (
    <div className="mt-4 space-y-2">
      {toolCalls.map((toolCall) => (
        <CollapsibleBlock
          key={toolCall.id}
          summary={
            <div className="flex min-w-0 items-center gap-2">
              <Wrench className="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
              <span className="text-xs font-semibold uppercase tracking-[0.16em] text-emerald-700 dark:text-emerald-400">
                工具
              </span>
              <span className="font-medium text-emerald-700 dark:text-emerald-300">
                {toolCall.name}
              </span>
              <span className="truncate text-sm text-slate-500 dark:text-slate-400">
                {summarizeToolInput(toolCall)}
              </span>
            </div>
          }
          iconColor="text-emerald-600"
        >
          <pre className="overflow-x-auto whitespace-pre-wrap rounded-xl bg-[#f7f5ef] p-3 text-xs text-slate-700 dark:bg-slate-800 dark:text-slate-300">
            {formatToolInput(toolCall.input)}
          </pre>
        </CollapsibleBlock>
      ))}
    </div>
  );
}

/**
 * 助手响应消息渲染组件。
 * 支持 thinking、tool_use、text 等不同类型的 assistant message。
 */
export const MessageResponse = memo(function MessageResponse({
  entry,
  compact = false,
  className,
}: MessageResponseProps) {
  const thinkingBlocks = useMemo(() => extractThinkingBlocks(entry), [entry]);
  const hasThinking = thinkingBlocks.length > 0;
  const hasToolCalls = entry.tool_calls.length > 0;
  const hasText = !!entry.text.trim();

  return (
    <div
      className={cn(
        'rounded-[24px] border border-[#e8e2d8] bg-white px-5 py-4 shadow-[0_10px_28px_rgba(23,24,26,0.05)] dark:border-slate-700 dark:bg-slate-800 dark:shadow-[0_10px_28px_rgba(0,0,0,0.2)]',
        compact && 'px-4 py-3',
        className,
      )}
    >
      {/* 标题 */}
      <div className="mb-3 flex items-center gap-2">
        <FileText className="h-3.5 w-3.5 text-slate-400 dark:text-slate-500" />
        <span className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-400 dark:text-slate-500">
          助手
        </span>
        {hasThinking && (
          <span className="rounded-full bg-amber-50 px-2 py-0.5 text-[10px] text-amber-600 dark:bg-amber-900/50 dark:text-amber-400">
            含思考过程
          </span>
        )}
        {hasToolCalls && (
          <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[10px] text-emerald-600 dark:bg-emerald-900/50 dark:text-emerald-400">
            {entry.tool_calls.length} 个工具调用
          </span>
        )}
      </div>

      {/* Thinking 块 */}
      <ThinkingSection blocks={thinkingBlocks} />

      {/* 文本内容 */}
      {hasText ? (
        <div className="prose prose-slate max-w-none dark:prose-invert">
          <Suspense
            fallback={
              <div className="text-sm text-slate-500 dark:text-slate-400">正在渲染回复…</div>
            }
          >
            <LazyMarkdownRenderer content={entry.text} />
          </Suspense>
        </div>
      ) : (
        !hasToolCalls && (
          <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-slate-400">
            <ChevronRight className="h-3.5 w-3.5 animate-pulse" />
            <span>正在生成回复…</span>
          </div>
        )
      )}

      {/* 工具调用 */}
      <ToolCallsSection toolCalls={entry.tool_calls} />
    </div>
  );
});
