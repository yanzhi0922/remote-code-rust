import { memo, useRef, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import type { ConversationEntry } from '../lib/types';
import { cn } from '../lib/utils';
import { MessageRow } from './MessageRow';
import { MessageResponse } from './MessageResponse';
import { UserTextMessage } from './messages/UserTextMessage';
import { UserToolResultMessage } from './messages/UserToolResultMessage';

/** 虚拟化阈值：消息数超过此值启用虚拟滚动 */
const VIRTUALIZATION_THRESHOLD = 80;
/** 虚拟滚动 overscan 数量 */
const VIRTUALIZATION_OVERSCAN = 10;

/** 消息列表属性 */
export interface MessagesProps {
  /** 对话条目列表 */
  conversation: ConversationEntry[];
  /** 是否正在发送 */
  sending?: boolean;
  /** 发送错误信息 */
  sendError?: string | null;
  /** 是否为 transcript 模式 */
  isTranscriptMode?: boolean;
  /** 重新发送回调 */
  onResend?: (messageId: string) => void;
  /** 编辑回调 */
  onEdit?: (messageId: string) => void;
  /** 额外的 CSS 类名 */
  className?: string;
}

/** 估算消息行高度 */
function estimateEntryHeight(entry: ConversationEntry): number {
  switch (entry.role) {
    case 'assistant':
      return 320;
    case 'tool':
      return 180;
    case 'user':
      return 120;
    default:
      return 64;
  }
}

/** 生成消息行唯一 key */
function messageRowKey(entry: ConversationEntry, index: number): string {
  return `${entry.role}-${entry.tool_call_id ?? entry.name ?? 'entry'}-${index}`;
}

/** 空状态组件 */
function EmptyState({ title, description }: { title: string; description: string }) {
  return (
    <div className="flex h-full min-h-[320px] items-center justify-center px-6 py-10">
      <div className="max-w-xl space-y-3 text-center">
        <h2 className="text-xl font-semibold text-slate-800 dark:text-slate-200">{title}</h2>
        <p className="text-sm leading-6 text-slate-500 dark:text-slate-400">{description}</p>
      </div>
    </div>
  );
}

/** 单条消息卡片渲染 */
const MessageCard = memo(
  function MessageCard({
    entry,
    index,
    isTranscriptMode,
    onResend,
    onEdit,
  }: {
    entry: ConversationEntry;
    index: number;
    isTranscriptMode: boolean;
    onResend?: (messageId: string) => void;
    onEdit?: (messageId: string) => void;
  }) {
    if (entry.role === 'system') return null;

    if (entry.role === 'tool') {
      return (
        <MessageRow
          role="tool"
          timestamp={null}
          messageText={entry.text}
          messageId={`${entry.tool_call_id ?? index}`}
          isTranscriptMode={isTranscriptMode}
          onResend={onResend}
          onEdit={onEdit}
        >
          <UserToolResultMessage entry={entry} />
        </MessageRow>
      );
    }

    if (entry.role === 'user') {
      return (
        <MessageRow
          role="user"
          timestamp={null}
          messageText={entry.text}
          messageId={`${index}`}
          isTranscriptMode={isTranscriptMode}
          onResend={onResend}
          onEdit={onEdit}
        >
          <UserTextMessage entry={entry} />
        </MessageRow>
      );
    }

    // assistant
    return (
      <MessageRow
        role="assistant"
        timestamp={null}
        messageText={entry.text}
        messageId={`${index}`}
        isTranscriptMode={isTranscriptMode}
        showActions
      >
        <MessageResponse entry={entry} isTranscriptMode={isTranscriptMode} />
      </MessageRow>
    );
  },
  (prev, next) => prev.entry === next.entry && prev.isTranscriptMode === next.isTranscriptMode,
);

/**
 * 消息列表组件。
 * 渲染所有消息的虚拟滚动列表，超过阈值自动启用虚拟化。
 */
export const Messages = memo(function Messages({
  conversation,
  sending = false,
  sendError = null,
  isTranscriptMode = false,
  onResend,
  onEdit,
  className,
}: MessagesProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  const shouldVirtualize = conversation.length >= VIRTUALIZATION_THRESHOLD;

  const rowVirtualizer = useVirtualizer({
    count: conversation.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: (index) => estimateEntryHeight(conversation[index] ?? conversation[0]),
    overscan: VIRTUALIZATION_OVERSCAN,
    getItemKey: (index) =>
      messageRowKey(conversation[index] ?? conversation[0], index),
  });

  // 自动滚动到底部
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }, [conversation, sending]);

  if (conversation.length === 0) {
    return (
      <div className={cn('flex-1 overflow-y-auto bg-[#f8f7f4] dark:bg-slate-900', className)}>
        <EmptyState
          title="会话已创建"
          description="直接在下方输入框中发送需求。消息会以对话形式展示，工具调用和输出默认折叠。"
        />
      </div>
    );
  }

  return (
    <div
      ref={scrollContainerRef}
      className={cn(
        'flex-1 min-h-0 overflow-y-auto bg-[#f8f7f4] px-4 py-5 sm:px-6 dark:bg-slate-900',
        className,
      )}
    >
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-4">
        {shouldVirtualize ? (
          <div
            className="relative w-full"
            style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
          >
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const entry = conversation[virtualRow.index];
              return (
                <div
                  key={virtualRow.key}
                  data-index={virtualRow.index}
                  ref={rowVirtualizer.measureElement}
                  className="absolute left-0 top-0 w-full pb-4"
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                >
                  <MessageCard
                    entry={entry}
                    index={virtualRow.index}
                    isTranscriptMode={isTranscriptMode}
                    onResend={onResend}
                    onEdit={onEdit}
                  />
                </div>
              );
            })}
          </div>
        ) : (
          conversation.map((entry, index) => (
            <MessageCard
              key={messageRowKey(entry, index)}
              entry={entry}
              index={index}
              isTranscriptMode={isTranscriptMode}
              onResend={onResend}
              onEdit={onEdit}
            />
          ))
        )}

        {/* 发送状态 */}
        {sending && (
          <div className="rounded-2xl border border-[#e3ddd2] bg-white px-5 py-4 text-sm text-slate-600 shadow-[0_10px_24px_rgba(23,24,26,0.05)] dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400">
            <div className="flex items-center gap-3">
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-slate-300 border-t-slate-700 dark:border-slate-600 dark:border-t-slate-300" />
              <span>正在处理当前请求…</span>
            </div>
          </div>
        )}

        {/* 发送错误 */}
        {sendError && (
          <div className="rounded-2xl border border-[#f3cbc6] bg-[#fff6f4] px-5 py-4 text-sm text-[#9c2f2f] dark:border-rose-800 dark:bg-rose-950/30 dark:text-rose-300">
            {sendError}
          </div>
        )}

        <div ref={bottomRef} />
      </div>
    </div>
  );
});
