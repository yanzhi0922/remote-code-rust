import {
  lazy,
  memo,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  type MutableRefObject,
} from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import type {
  ConversationEntry,
  ToolCallInfo,
  ToolProgressInfo,
  ToolResultInfo,
} from '../../lib/types';
import { truncateMiddle } from '../../lib/utils';
import {
  formatToolInput,
  summarizeToolInput,
  extractThinkingBlocks,
  estimateEntryHeight,
} from '../../lib/conversationUtils';
import { useAppStore } from '../../stores/useAppStore';
import CollapsibleBlock from './CollapsibleBlock';
import { GoalStatusBar } from './GoalStatusBar';

const LazyMarkdownRenderer = lazy(() => import('./MarkdownRenderer'));
const VIRTUALIZATION_THRESHOLD = 80;
const VIRTUALIZATION_OVERSCAN = 10;

function summarizeToolOutput(text: string): string {
  const compact = text.replace(/\s+/g, ' ').trim();
  return compact ? truncateMiddle(compact, 84) : '展开查看完整输出';
}

function conversationRowKey(entry: ConversationEntry, index: number): string {
  return `${entry.role}-${entry.tool_call_id ?? entry.name ?? 'entry'}-${index}`;
}

function EmptyState({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <div className="flex h-full min-h-[400px] items-center justify-center px-6">
      <div className="max-w-md text-center space-y-4">
        <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-rc-accent-primary to-purple-500 shadow-lg">
          <span className="text-2xl font-bold text-white">RC</span>
        </div>
        <div>
          <h2 className="text-xl font-semibold text-rc-text-primary">{title}</h2>
          <p className="mt-2 text-sm leading-6 text-rc-text-secondary">{description}</p>
        </div>
      </div>
    </div>
  );
}

function AssistantToolCalls({ toolCalls }: { toolCalls: ToolCallInfo[] }) {
  if (toolCalls.length === 0) return null;

  return (
    <div className="mt-5 space-y-3">
      {toolCalls.map((toolCall) => (
        <CollapsibleBlock
          key={toolCall.id}
          summary={
            <div className="flex min-w-0 items-center gap-2.5">
              <span className="rounded-md bg-rc-accent-success-bg px-2 py-0.5 text-[11px] font-semibold uppercase tracking-wide text-rc-accent-success">
                Tool
              </span>
              <span className="font-mono text-sm font-medium text-rc-text-primary">{toolCall.name}</span>
              <span className="truncate text-sm text-rc-text-tertiary">{summarizeToolInput(toolCall)}</span>
            </div>
          }
          iconColor="text-rc-accent-success"
        >
          <pre className="overflow-x-auto whitespace-pre-wrap rounded-xl bg-rc-bg-code p-4 text-xs font-mono leading-relaxed text-slate-300">
            {formatToolInput(toolCall.input)}
          </pre>
        </CollapsibleBlock>
      ))}
    </div>
  );
}

function ToolMessage({ entry }: { entry: ConversationEntry }) {
  const label = entry.name ?? 'tool';

  return (
    <CollapsibleBlock
      summary={
        <div className="flex min-w-0 items-center gap-2.5">
          <span
            className={`rounded-md px-2 py-0.5 text-[11px] font-semibold uppercase tracking-wide ${
              entry.is_error
                ? 'bg-rc-accent-error-bg text-rc-accent-error'
                : 'bg-rc-bg-active text-rc-text-tertiary'
            }`}
          >
            {entry.is_error ? 'Error' : 'Result'}
          </span>
          <span className="font-mono text-sm font-medium text-rc-text-primary">{label}</span>
          <span className="truncate text-sm text-rc-text-tertiary">{summarizeToolOutput(entry.text)}</span>
        </div>
      }
      iconColor={entry.is_error ? 'text-rc-accent-error' : 'text-rc-text-tertiary'}
    >
      <pre
        className={`overflow-x-auto whitespace-pre-wrap rounded-xl p-4 text-xs font-mono leading-relaxed ${
          entry.is_error
            ? 'bg-rc-accent-error-bg text-rc-accent-error'
            : 'bg-rc-bg-code text-slate-300'
        }`}
      >
        {entry.text}
      </pre>
    </CollapsibleBlock>
  );
}

function AssistantThinking({ blocks }: { blocks: string[] }) {
  if (blocks.length === 0) return null;

  return (
    <div className="mb-4 space-y-2">
      {blocks.map((block, index) => (
        <CollapsibleBlock
          key={`${index}-${block.slice(0, 20)}`}
          summary={
            <div className="flex min-w-0 items-center gap-2.5">
              <span className="rounded-md bg-rc-accent-warning-bg px-2 py-0.5 text-[11px] font-semibold uppercase tracking-wide text-rc-accent-warning">
                Thinking
              </span>
              <span className="truncate text-sm text-rc-text-tertiary">{summarizeToolOutput(block)}</span>
            </div>
          }
          iconColor="text-rc-accent-warning"
        >
          <div className="rounded-xl bg-rc-bg-secondary p-4 text-sm leading-7 text-rc-text-secondary">
            {block}
          </div>
        </CollapsibleBlock>
      ))}
    </div>
  );
}

function AssistantMessage({ entry }: { entry: ConversationEntry }) {
  const thinkingBlocks = extractThinkingBlocks(entry);

  return (
    <div className="rounded-2xl border border-rc-border-primary bg-rc-bg-assistant p-6 shadow-sm">
      <div className="mb-4 flex items-center gap-2">
        <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-gradient-to-br from-rc-accent-primary to-purple-500">
          <span className="text-white text-xs font-bold">A</span>
        </div>
        <span className="text-xs font-semibold uppercase tracking-wider text-rc-text-tertiary">
          Assistant
        </span>
      </div>

      <AssistantThinking blocks={thinkingBlocks} />

      {entry.text ? (
        <div className="prose prose-slate max-w-none prose-p:leading-relaxed prose-code:rounded prose-code:bg-rc-bg-code prose-code:px-1.5 prose-code:py-0.5 prose-code:text-sm">
          <Suspense fallback={<div className="space-y-2"><div className="h-4 w-3/4 animate-pulse rounded bg-rc-bg-code" /><div className="h-4 w-1/2 animate-pulse rounded bg-rc-bg-code" /></div>}>
            <LazyMarkdownRenderer content={entry.text} />
          </Suspense>
        </div>
      ) : (
        <div className="text-sm text-rc-text-tertiary">模型请求了工具调用。</div>
      )}

      <AssistantToolCalls toolCalls={entry.tool_calls} />
    </div>
  );
}

const MessageCard = memo(
  function MessageCard({ entry }: { entry: ConversationEntry }) {
    if (entry.role === 'system') return null;

    if (entry.role === 'tool') {
      return <ToolMessage entry={entry} />;
    }

    if (entry.role === 'user') {
      return (
        <div className="flex justify-end">
          <div className="max-w-3xl rounded-2xl bg-gradient-to-br from-rc-accent-primary to-rc-accent-primary-hover px-5 py-4 text-[15px] leading-7 text-white shadow-lg">
            <div className="whitespace-pre-wrap break-words">{entry.text}</div>
          </div>
        </div>
      );
    }

    return <AssistantMessage entry={entry} />;
  },
  (previous, next) => previous.entry === next.entry,
);

function StatusCards({
  sending,
  compactProgress,
  compactResults,
  sendError,
  bottomRef,
}: {
  sending: boolean;
  compactProgress: ToolProgressInfo[];
  compactResults: ToolResultInfo[];
  sendError: string | null;
  bottomRef: MutableRefObject<HTMLDivElement | null>;
}) {
  return (
    <>
      {sending && (
        <div role="status" className="rounded-2xl border border-rc-border-primary bg-rc-bg-assistant px-5 py-4 text-sm text-rc-text-secondary shadow-sm">
          <div className="flex items-center gap-3">
            <div className="flex h-5 w-5 items-center justify-center">
              <div className="h-5 w-5 animate-spin rounded-full border-2 border-rc-border-primary border-t-rc-accent-primary" />
            </div>
            <span className="font-medium">正在处理当前请求…</span>
          </div>

          {compactProgress.length > 0 && (
            <div className="mt-4 space-y-2">
              {compactProgress.map((progress, index) => (
                <div
                  key={`${progress.tool_name}-${progress.tool_call_id}-${index}`}
                  className="flex items-center gap-2 rounded-lg bg-rc-bg-secondary px-3 py-2 text-xs"
                >
                  <span className="font-mono font-medium text-rc-text-primary">{progress.tool_name || 'tool'}</span>
                  <span className="text-rc-text-tertiary">·</span>
                  <span className="truncate text-rc-text-secondary">{truncateMiddle(progress.active_form ?? progress.message, 120)}</span>
                </div>
              ))}
            </div>
          )}

          {compactResults.length > 0 && (
            <div className="mt-4 space-y-2">
              {compactResults.map((result, index) => (
                <div
                  key={`${result.tool_name}-${result.tool_call_id}-${index}`}
                  className={`flex items-center gap-2 rounded-lg px-3 py-2 text-xs ${
                    result.is_error
                      ? 'bg-rc-accent-error-bg text-rc-accent-error'
                      : 'bg-rc-accent-success-bg text-rc-accent-success'
                  }`}
                >
                  <span className="font-mono font-medium">{result.tool_name}</span>
                  <span className="opacity-60">·</span>
                  <span className="truncate">{truncateMiddle(result.output, 110)}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {sendError && (
        <div role="alert" className="rounded-2xl border border-rc-accent-error-border bg-rc-accent-error-bg px-5 py-4 text-sm text-rc-accent-error">
          {sendError}
        </div>
      )}

      <div ref={bottomRef} />
    </>
  );
}

function ConversationTimeline({
  conversation,
  sending,
  compactProgress,
  compactResults,
  sendError,
  bottomRef,
}: {
  conversation: ConversationEntry[];
  sending: boolean;
  compactProgress: ToolProgressInfo[];
  compactResults: ToolResultInfo[];
  sendError: string | null;
  bottomRef: MutableRefObject<HTMLDivElement | null>;
}) {
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const shouldVirtualize = conversation.length >= VIRTUALIZATION_THRESHOLD;
  const rowVirtualizer = useVirtualizer({
    count: conversation.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: (index) => estimateEntryHeight(conversation[index] ?? conversation[0]),
    overscan: VIRTUALIZATION_OVERSCAN,
    getItemKey: (index) => conversationRowKey(conversation[index] ?? conversation[0], index),
  });

  return (
    <div
      ref={scrollContainerRef}
      className="flex-1 min-h-0 overflow-y-auto bg-rc-bg-chat px-6 py-6"
    >
      <div className="mx-auto flex w-full max-w-chat flex-col gap-5">
        {shouldVirtualize ? (
          <div className="relative w-full" style={{ height: `${rowVirtualizer.getTotalSize()}px` }}>
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const entry = conversation[virtualRow.index];
              return (
                <div
                  key={virtualRow.key}
                  data-index={virtualRow.index}
                  ref={rowVirtualizer.measureElement}
                  className="absolute left-0 top-0 w-full"
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                >
                  <div className="pb-5">
                    <MessageCard entry={entry} />
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          conversation.map((entry, index) => (
            <div key={conversationRowKey(entry, index)} className="pb-5">
              <MessageCard entry={entry} />
            </div>
          ))
        )}

        <StatusCards
          sending={sending}
          compactProgress={compactProgress}
          compactResults={compactResults}
          sendError={sendError}
          bottomRef={bottomRef}
        />
      </div>
    </div>
  );
}

export function ChatArea() {
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const conversation = useAppStore((state) => state.conversation);
  const conversationLoading = useAppStore((state) => state.conversationLoading);
  const sending = useAppStore((state) => state.sending);
  const sendError = useAppStore((state) => state.sendError);
  const liveToolProgress = useAppStore((state) => state.liveToolProgress);
  const liveToolResults = useAppStore((state) => state.liveToolResults);
  const bottomRef = useRef<HTMLDivElement>(null);

  const compactProgress = useMemo(() => liveToolProgress.slice(-6), [liveToolProgress]);
  const compactResults = useMemo(() => liveToolResults.slice(-4), [liveToolResults]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }, [conversation, sending, liveToolProgress, liveToolResults]);

  if (!activeSessionId) {
    return (
      <div className="flex-1 overflow-y-auto bg-rc-bg-chat">
        <EmptyState
          title="选择一个项目或会话"
          description="左侧按项目、会话、子任务三层组织。选中后，右侧会渲染完整对话、公式、工具调用和折叠详情。"
        />
      </div>
    );
  }

  if (conversationLoading) {
    return (
      <div className="flex-1 overflow-y-auto bg-rc-bg-chat">
        <EmptyState title="正在加载会话" description="正在读取本地会话历史与工具调用记录。" />
      </div>
    );
  }

  if (conversation.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto bg-rc-bg-chat">
        <EmptyState
          title="会话已创建"
          description="直接在下方输入框中发送需求。公式会渲染，工具调用、代码块和工具输出会默认折叠。"
        />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col bg-rc-bg-chat">
      <GoalStatusBar />
      <ConversationTimeline
        conversation={conversation}
        sending={sending}
        compactProgress={compactProgress}
        compactResults={compactResults}
        sendError={sendError}
        bottomRef={bottomRef}
      />
    </div>
  );
}