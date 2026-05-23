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
import { GitBranch, MoreHorizontal } from 'lucide-react';
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
import { WorkspaceOverview } from '../layout/WorkspaceOverview';
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
}: {
  title: string;
}) {
  return (
    <div className="flex h-full min-h-[320px] items-center justify-center px-6">
      <div className="rounded-md border border-dashed border-rc-border-primary bg-rc-bg-surface px-5 py-4 text-sm text-rc-text-tertiary">
        {title}
      </div>
    </div>
  );
}

function AssistantToolCalls({ toolCalls }: { toolCalls: ToolCallInfo[] }) {
  if (toolCalls.length === 0) return null;

  return (
    <div className="mt-3 space-y-2">
      {toolCalls.map((toolCall) => (
        <CollapsibleBlock
          key={toolCall.id}
          summary={
            <div className="flex min-w-0 items-center gap-2.5">
              <span className="rounded bg-rc-accent-success-bg px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-rc-accent-success">
                Tool
              </span>
              <span className="font-mono text-xs font-medium text-rc-text-primary">{toolCall.name}</span>
              <span className="truncate text-xs text-rc-text-tertiary">{summarizeToolInput(toolCall)}</span>
            </div>
          }
          iconColor="text-rc-accent-success"
        >
          <pre className="overflow-x-auto whitespace-pre-wrap rounded bg-rc-bg-code p-3 text-xs font-mono leading-relaxed text-rc-text-primary">
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
            className={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-[0.08em] ${
              entry.is_error
                ? 'bg-rc-accent-error-bg text-rc-accent-error'
                : 'bg-rc-bg-active text-rc-text-tertiary'
            }`}
          >
            {entry.is_error ? 'Error' : 'Result'}
          </span>
          <span className="font-mono text-xs font-medium text-rc-text-primary">{label}</span>
          <span className="truncate text-xs text-rc-text-tertiary">{summarizeToolOutput(entry.text)}</span>
        </div>
      }
      iconColor={entry.is_error ? 'text-rc-accent-error' : 'text-rc-text-tertiary'}
    >
      <pre
        className={`overflow-x-auto whitespace-pre-wrap rounded p-3 text-xs font-mono leading-relaxed ${
          entry.is_error
            ? 'bg-rc-accent-error-bg text-rc-accent-error'
            : 'bg-rc-bg-code text-rc-text-primary'
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
    <div className="mb-3 space-y-2">
      {blocks.map((block, index) => (
        <CollapsibleBlock
          key={`${index}-${block.slice(0, 20)}`}
          summary={
            <div className="flex min-w-0 items-center gap-2.5">
              <span className="rounded bg-rc-accent-warning-bg px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-rc-accent-warning">
                Thinking
              </span>
              <span className="truncate text-xs text-rc-text-tertiary">{summarizeToolOutput(block)}</span>
            </div>
          }
          iconColor="text-rc-accent-warning"
        >
          <div className="rounded bg-rc-bg-secondary p-3 text-xs leading-6 text-rc-text-secondary">
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
    <div className="rounded-lg border border-rc-border-secondary bg-rc-bg-assistant px-4 py-3 shadow-xs">
      <div className="mb-3 flex items-center gap-2">
        <span className="h-2 w-2 rounded-full bg-rc-accent-info" />
        <span className="text-[10px] font-semibold uppercase tracking-[0.08em] text-rc-text-tertiary">
          Assistant
        </span>
      </div>

      <AssistantThinking blocks={thinkingBlocks} />

      {entry.text ? (
        <div className="markdown-body max-w-none text-rc-text-primary">
          <Suspense fallback={<div className="space-y-2"><div className="h-4 w-3/4 animate-pulse rounded bg-rc-bg-code" /><div className="h-4 w-1/2 animate-pulse rounded bg-rc-bg-code" /></div>}>
            <LazyMarkdownRenderer content={entry.text} />
          </Suspense>
        </div>
      ) : (
        <div className="text-sm text-rc-text-tertiary">Tool request</div>
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
          <div className="max-w-[720px] rounded-lg border border-rc-border-secondary bg-rc-bg-selected px-4 py-3 text-sm leading-6 text-rc-text-primary">
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
        <div role="status" className="rounded-lg border border-rc-border-primary bg-rc-bg-assistant px-4 py-3 text-sm text-rc-text-secondary shadow-xs">
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
                  className="flex items-center gap-2 rounded bg-rc-bg-secondary px-2.5 py-1.5 text-xs"
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
        <div role="alert" className="rounded-lg border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-3 text-sm text-rc-accent-error">
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
  const scrollContainerRef = useRef<HTMLElement>(null);
  const shouldVirtualize = conversation.length >= VIRTUALIZATION_THRESHOLD;
  const rowVirtualizer = useVirtualizer({
    count: conversation.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: (index) => estimateEntryHeight(conversation[index] ?? conversation[0]),
    overscan: VIRTUALIZATION_OVERSCAN,
    getItemKey: (index) => conversationRowKey(conversation[index] ?? conversation[0], index),
  });

  return (
    <section
      ref={scrollContainerRef}
      aria-label="Conversation transcript"
      className="flex-1 min-h-0 overflow-y-auto bg-rc-bg-chat px-5 py-5"
    >
      <div className="mx-auto flex w-full max-w-[920px] flex-col gap-3">
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
                  <div className="pb-3">
                    <MessageCard entry={entry} />
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          conversation.map((entry, index) => (
            <div key={conversationRowKey(entry, index)} className="pb-3">
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
    </section>
  );
}

function ConversationHeader({
  title,
  model,
  provider,
}: {
  title: string;
  model?: string | null;
  provider?: string | null;
}) {
  return (
    <div className="flex h-12 shrink-0 items-center justify-between border-b border-rc-border-secondary bg-rc-bg-surface px-5">
      <div className="flex min-w-0 items-center gap-2">
        <div className="min-w-0 truncate text-sm font-semibold text-rc-text-primary">{title}</div>
        <button
          type="button"
          aria-label="Session actions"
          title="Session actions"
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <MoreHorizontal size={16} />
        </button>
      </div>

      <div className="hidden min-w-0 items-center gap-2 text-xs text-rc-text-tertiary md:flex">
        <GitBranch size={14} />
        <span className="truncate">{provider ?? 'provider'}</span>
        {model && (
          <>
            <span>·</span>
            <span className="truncate font-mono">{model}</span>
          </>
        )}
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
  const sessions = useAppStore((state) => state.sessions);
  const bottomRef = useRef<HTMLDivElement>(null);

  const compactProgress = useMemo(() => liveToolProgress.slice(-6), [liveToolProgress]);
  const compactResults = useMemo(() => liveToolResults.slice(-4), [liveToolResults]);
  const activeSession = useMemo(
    () => sessions.find((session) => session.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }, [conversation, sending, liveToolProgress, liveToolResults]);

  if (!activeSessionId) {
    return <WorkspaceOverview />;
  }

  if (conversationLoading) {
    return (
      <div className="flex h-full min-h-0 flex-1 flex-col bg-rc-bg-chat">
        <ConversationHeader
          title={activeSession?.title ?? 'Session'}
          provider={activeSession?.provider_name}
          model={activeSession?.model}
        />
        <div className="flex-1 overflow-y-auto">
          <EmptyState title="Loading session" />
        </div>
      </div>
    );
  }

  if (conversation.length === 0) {
    return (
      <div className="flex h-full min-h-0 flex-1 flex-col bg-rc-bg-chat">
        <ConversationHeader
          title={activeSession?.title ?? 'Session'}
          provider={activeSession?.provider_name}
          model={activeSession?.model}
        />
        <div className="flex-1 overflow-y-auto">
          <EmptyState title="No messages" />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col bg-rc-bg-chat">
      <ConversationHeader
        title={activeSession?.title ?? 'Session'}
        provider={activeSession?.provider_name}
        model={activeSession?.model}
      />
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
