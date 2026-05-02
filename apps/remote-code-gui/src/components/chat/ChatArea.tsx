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
import { useAppStore } from '../../stores/useAppStore';
import CollapsibleBlock from './CollapsibleBlock';

const LazyMarkdownRenderer = lazy(() => import('./MarkdownRenderer'));
const VIRTUALIZATION_THRESHOLD = 80;
const VIRTUALIZATION_OVERSCAN = 10;

function formatToolInput(input: unknown): string {
  try {
    const normalized = typeof input === 'string' ? JSON.parse(input) : input;
    return JSON.stringify(normalized, null, 2);
  } catch {
    return typeof input === 'string' ? input : JSON.stringify(input, null, 2);
  }
}

function summarizeToolInput(toolCall: ToolCallInfo): string {
  try {
    const normalized = typeof toolCall.input === 'string' ? JSON.parse(toolCall.input) : toolCall.input;
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
    // Ignore summary parsing failures.
  }
  return toolCall.name;
}

function summarizeToolOutput(text: string): string {
  const compact = text.replace(/\s+/g, ' ').trim();
  return compact ? truncateMiddle(compact, 84) : '展开查看完整输出';
}

function extractThinkingBlocks(entry: ConversationEntry): string[] {
  return entry.content_blocks
    .filter((block): block is Record<string, unknown> => !!block && typeof block === 'object')
    .filter((block) => block.type === 'thinking' && typeof block.thinking === 'string')
    .map((block) => block.thinking as string);
}

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
    <div className="flex h-full min-h-[320px] items-center justify-center px-6 py-10">
      <div className="max-w-xl space-y-3 text-center">
        <h2 className="text-xl font-semibold text-rc-text-primary">{title}</h2>
        <p className="text-sm leading-6 text-rc-text-secondary">{description}</p>
      </div>
    </div>
  );
}

function AssistantToolCalls({ toolCalls }: { toolCalls: ToolCallInfo[] }) {
  if (toolCalls.length === 0) return null;

  return (
    <div className="mt-4 space-y-2">
      {toolCalls.map((toolCall) => (
        <CollapsibleBlock
          key={toolCall.id}
          summary={
            <div className="flex min-w-0 items-center gap-2">
              <span className="text-xs font-semibold uppercase tracking-[0.16em] text-rc-accent-success">
                Tool
              </span>
              <span className="font-medium text-rc-accent-success">{toolCall.name}</span>
              <span className="truncate text-sm text-rc-text-secondary">{summarizeToolInput(toolCall)}</span>
            </div>
          }
          iconColor="text-rc-accent-success"
        >
          <pre className="overflow-x-auto whitespace-pre-wrap rounded-xl bg-rc-bg-secondary p-3 text-xs text-rc-text-primary">
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
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-xs font-semibold uppercase tracking-[0.16em] text-rc-text-tertiary">
            {entry.is_error ? 'Tool Error' : 'Tool Result'}
          </span>
          <span
            className={`rounded-full px-2 py-0.5 text-[11px] ${
              entry.is_error ? 'bg-rc-accent-error-bg text-rc-accent-error' : 'bg-rc-bg-tertiary text-rc-text-secondary'
            }`}
          >
            {label}
          </span>
          <span className="truncate text-sm text-rc-text-secondary">{summarizeToolOutput(entry.text)}</span>
        </div>
      }
      iconColor={entry.is_error ? 'text-rc-accent-error' : 'text-rc-text-tertiary'}
    >
      <pre
        className={`overflow-x-auto whitespace-pre-wrap rounded-xl p-3 text-xs leading-6 ${
          entry.is_error ? 'bg-rc-accent-error-bg text-rc-accent-error' : 'bg-rc-bg-secondary text-rc-text-primary'
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
            <div className="flex min-w-0 items-center gap-2">
              <span className="text-xs font-semibold uppercase tracking-[0.16em] text-rc-accent-warning">
                Thinking
              </span>
              <span className="truncate text-sm text-rc-text-secondary">{summarizeToolOutput(block)}</span>
            </div>
          }
          iconColor="text-rc-accent-warning"
        >
          <div className="whitespace-pre-wrap text-sm leading-7 text-rc-text-primary">{block}</div>
        </CollapsibleBlock>
      ))}
    </div>
  );
}

function AssistantMessage({ entry }: { entry: ConversationEntry }) {
  const thinkingBlocks = extractThinkingBlocks(entry);

  return (
    <div className="rounded-[24px] border border-rc-border-primary bg-rc-bg-assistant-card px-5 py-4 shadow-md">
      <div className="mb-3 text-xs font-semibold uppercase tracking-[0.22em] text-rc-text-tertiary">
        Assistant
      </div>

      <AssistantThinking blocks={thinkingBlocks} />

      {entry.text ? (
        <div className="prose prose-slate max-w-none">
          <Suspense fallback={<div className="text-sm text-rc-text-secondary">正在渲染回复…</div>}>
            <LazyMarkdownRenderer content={entry.text} />
          </Suspense>
        </div>
      ) : (
        <div className="text-sm text-rc-text-secondary">模型请求了工具调用。</div>
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
          <div className="max-w-3xl rounded-[24px] bg-rc-bg-user-bubble px-5 py-4 text-[15px] leading-7 text-rc-text-inverse shadow-lg">
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
        <div className="rounded-2xl border border-rc-border-primary bg-rc-bg-assistant-card px-5 py-4 text-sm text-rc-text-secondary shadow-md">
          <div className="flex items-center gap-3">
            <div className="h-4 w-4 animate-spin rounded-full border-2 border-rc-border-primary border-t-rc-text-primary" />
            <span>正在处理当前请求…</span>
          </div>

          {compactProgress.length > 0 && (
            <div className="mt-4 space-y-2">
              {compactProgress.map((progress, index) => (
                <div
                  key={`${progress.tool_name}-${progress.tool_call_id}-${index}`}
                  className="rounded-xl bg-rc-bg-secondary px-3 py-2 text-xs text-rc-text-secondary"
                >
                  <span className="font-medium text-rc-text-primary">{progress.tool_name || 'tool'}</span>
                  <span className="mx-2 text-rc-text-tertiary">·</span>
                  <span>{truncateMiddle(progress.active_form ?? progress.message, 120)}</span>
                </div>
              ))}
            </div>
          )}

          {compactResults.length > 0 && (
            <div className="mt-4 space-y-2">
              {compactResults.map((result, index) => (
                <div
                  key={`${result.tool_name}-${result.tool_call_id}-${index}`}
                  className={`rounded-xl px-3 py-2 text-xs ${
                    result.is_error ? 'bg-rc-accent-error-bg text-rc-accent-error' : 'bg-rc-accent-success-bg text-rc-accent-success'
                  }`}
                >
                  <span className="font-medium">{result.tool_name}</span>
                  <span className="mx-2 opacity-60">·</span>
                  <span>{truncateMiddle(result.output, 110)}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {sendError && (
        <div className="rounded-2xl border border-rc-accent-error bg-rc-accent-error-bg px-5 py-4 text-sm text-rc-accent-error">
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
    <div ref={scrollContainerRef} className="flex-1 min-h-0 overflow-y-auto bg-rc-bg-chat px-4 py-5 sm:px-6">
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-4">
        {shouldVirtualize ? (
          <div className="relative w-full" style={{ height: `${rowVirtualizer.getTotalSize()}px` }}>
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
                  <MessageCard entry={entry} />
                </div>
              );
            })}
          </div>
        ) : (
          conversation.map((entry, index) => (
            <MessageCard key={conversationRowKey(entry, index)} entry={entry} />
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
