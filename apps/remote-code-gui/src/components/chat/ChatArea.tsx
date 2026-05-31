import {
  lazy,
  memo,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MutableRefObject,
} from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useTranslation } from 'react-i18next';
import {
  ArrowDown,
  CheckCircle2,
  ChevronRight,
  Copy,
  FileText,
  GitBranch,
  Loader2,
  MoreHorizontal,
  Terminal,
  Timer,
  Wrench,
  XCircle,
} from 'lucide-react';
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
import * as tauri from '../../lib/tauri';
import i18n from '../../i18n';
import { WorkspaceOverview } from '../layout/WorkspaceOverview';
import CollapsibleBlock from './CollapsibleBlock';
import { InlineDiffView, detectAndRenderDiff } from './InlineDiffView';
import { GoalStatusBar } from './GoalStatusBar';

const LazyMarkdownRenderer = lazy(() => import('./MarkdownRenderer'));
const VIRTUALIZATION_THRESHOLD = 80;
const VIRTUALIZATION_OVERSCAN = 10;

function summarizeToolOutput(text: string): string {
  const compact = text.replace(/\s+/g, ' ').trim();
  return compact ? truncateMiddle(compact, 84) : i18n.t('chatArea.toolOutput');
}

function conversationRowKey(entry: ConversationEntry, index: number): string {
  return `${entry.role}-${entry.tool_call_id ?? entry.name ?? 'entry'}-${index}`;
}

function CopyButton({ text }: { text: string }) {
  const { t } = useTranslation();
  const handleCopy = () => {
    void navigator.clipboard.writeText(text);
  };

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="flex h-6 w-6 items-center justify-center rounded text-rc-text-tertiary opacity-0 transition-all hover:bg-rc-bg-hover hover:text-rc-text-primary group-hover:opacity-100"
      aria-label={t('chatArea.copyContent')}
      title={t('chatArea.copyContent')}
    >
      <Copy size={12} />
    </button>
  );
}

function ToolIcon({ name }: { name: string }) {
  if (name.includes('shell') || name.includes('bash') || name.includes('exec')) {
    return <Terminal size={13} className="text-rc-accent-warning" />;
  }
  if (name.includes('file') || name.includes('read') || name.includes('write') || name.includes('edit') || name.includes('patch')) {
    return <FileText size={13} className="text-rc-accent-info" />;
  }
  if (name.includes('git')) {
    return <GitBranch size={13} className="text-rc-accent-success" />;
  }
  return <Wrench size={13} className="text-rc-text-tertiary" />;
}

function EmptyState({ title }: { title: string }) {
  return (
    <div className="flex h-full min-h-[320px] items-center justify-center px-6">
      <div className="rounded-xl border border-dashed border-rc-border-primary bg-rc-bg-elevated px-5 py-4 text-sm text-rc-text-tertiary shadow-xs">
        {title}
      </div>
    </div>
  );
}

function ToolCallCard({ toolCall }: { toolCall: ToolCallInfo }) {
  const formattedInput = formatToolInput(toolCall.input);
  const diffResult = useMemo(() => detectAndRenderDiff(formattedInput), [formattedInput]);

  if (diffResult.isDiff && diffResult.element) {
    return (
      <CollapsibleBlock
        summary={
          <div className="flex min-w-0 items-center gap-2.5">
            <ToolIcon name={toolCall.name} />
            <span className="font-mono text-xs font-medium text-rc-text-primary">{toolCall.name}</span>
            <span className="truncate text-xs text-rc-text-tertiary">{summarizeToolInput(toolCall)}</span>
          </div>
        }
        buttonLabel={`Toggle diff ${toolCall.name}`}
        iconColor="text-rc-accent-info"
      >
        {diffResult.element}
      </CollapsibleBlock>
    );
  }

  return (
    <CollapsibleBlock
      summary={
        <div className="flex min-w-0 items-center gap-2.5">
          <ToolIcon name={toolCall.name} />
          <span className="font-mono text-xs font-medium text-rc-text-primary">{toolCall.name}</span>
          <span className="truncate text-xs text-rc-text-tertiary">{summarizeToolInput(toolCall)}</span>
        </div>
      }
      buttonLabel={`Toggle tool call ${toolCall.name}`}
      iconColor="text-rc-accent-success"
    >
      <pre className="overflow-x-auto whitespace-pre-wrap rounded bg-rc-bg-code p-3 text-xs font-mono leading-relaxed text-rc-text-primary">
        {formattedInput}
      </pre>
    </CollapsibleBlock>
  );
}

function AssistantToolCalls({ toolCalls }: { toolCalls: ToolCallInfo[] }) {
  if (toolCalls.length === 0) return null;

  // Group consecutive tool calls into a summary when there are 3+
  if (toolCalls.length >= 3) {
    return (
      <CollapsibleBlock
        summary={
          <div className="flex min-w-0 items-center gap-2.5">
            <Wrench size={13} className="text-rc-accent-info" />
            <span className="text-xs font-medium text-rc-text-primary">
              {toolCalls.length} tool calls
            </span>
            <span className="truncate text-xs text-rc-text-tertiary">
              {toolCalls.map((tc) => tc.name).join(', ')}
            </span>
          </div>
        }
        buttonLabel={`Toggle ${toolCalls.length} tool calls`}
        iconColor="text-rc-accent-info"
        className="mt-3"
      >
        <div className="space-y-2">
          {toolCalls.map((toolCall) => (
            <ToolCallCard key={toolCall.id} toolCall={toolCall} />
          ))}
        </div>
      </CollapsibleBlock>
    );
  }

  return (
    <div className="mt-3 space-y-2">
      {toolCalls.map((toolCall) => (
        <ToolCallCard key={toolCall.id} toolCall={toolCall} />
      ))}
    </div>
  );
}

function ToolMessage({ entry }: { entry: ConversationEntry }) {
  const label = entry.name ?? 'tool';
  const diffResult = useMemo(() => detectAndRenderDiff(entry.text), [entry.text]);

  return (
    <CollapsibleBlock
      summary={
        <div className="flex min-w-0 items-center gap-2.5">
          {entry.is_error ? (
            <XCircle size={13} className="text-rc-accent-error" />
          ) : (
            <CheckCircle2 size={13} className="text-rc-accent-success" />
          )}
          <span className="font-mono text-xs font-medium text-rc-text-primary">{label}</span>
          <span className="truncate text-xs text-rc-text-tertiary">{summarizeToolOutput(entry.text)}</span>
        </div>
      }
      buttonLabel={`Toggle tool result ${label}`}
      iconColor={entry.is_error ? 'text-rc-accent-error' : 'text-rc-accent-success'}
    >
      {diffResult.isDiff && diffResult.element ? (
        diffResult.element
      ) : (
        <pre
          className={`overflow-x-auto whitespace-pre-wrap rounded p-3 text-xs font-mono leading-relaxed ${
            entry.is_error
              ? 'bg-rc-accent-error-bg text-rc-accent-error'
              : 'bg-rc-bg-code text-rc-text-primary'
          }`}
        >
          {entry.text}
        </pre>
      )}
    </CollapsibleBlock>
  );
}

function AssistantThinking({ blocks }: { blocks: string[] }) {
  const { t } = useTranslation();
  if (blocks.length === 0) return null;

  return (
    <div className="mb-3 space-y-2">
      {blocks.map((block, index) => (
        <CollapsibleBlock
          key={`${index}-${block.slice(0, 20)}`}
          summary={
            <div className="flex min-w-0 items-center gap-2.5">
              <span className="rounded bg-rc-accent-warning-bg px-1.5 py-0.5 text-[10px] font-semibold uppercase text-rc-accent-warning">
                {t('chatArea.thinking')}
              </span>
              <span className="truncate text-xs text-rc-text-tertiary">{summarizeToolOutput(block)}</span>
            </div>
          }
          buttonLabel={t('chatArea.toggleReasoning')}
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
  const { t } = useTranslation();
  const thinkingBlocks = extractThinkingBlocks(entry);

  return (
    <article className="group border-b border-rc-border-secondary py-6 last:border-b-0">
      <div className="mb-3 flex items-center gap-2">
        <span className="h-2 w-2 rounded-full bg-rc-accent-info" />
        <span className="text-[10px] font-semibold uppercase text-rc-text-tertiary">
          {t('chatArea.assistant')}
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
        <div className="text-sm text-rc-text-tertiary">{t('chatArea.toolRequest')}</div>
      )}

      <AssistantToolCalls toolCalls={entry.tool_calls} />
    </article>
  );
}

const MessageCard = memo(
  function MessageCard({ entry }: { entry: ConversationEntry }) {
    const { t } = useTranslation();
    if (entry.role === 'system') return null;

    if (entry.role === 'tool') {
      return <ToolMessage entry={entry} />;
    }

    if (entry.role === 'user') {
      const hasAttachments = entry.attachments && entry.attachments.length > 0;
      return (
        <div className="flex justify-end border-b border-rc-border-secondary py-6 last:border-b-0">
          <div className="max-w-[720px] rounded-xl border border-rc-border-primary bg-rc-bg-elevated px-4 py-3 text-sm leading-6 text-rc-text-primary shadow-xs">
            <div className="mb-2 flex items-center justify-between">
              <span className="text-[10px] font-semibold uppercase text-rc-text-tertiary">{t('chatArea.user')}</span>
              <CopyButton text={entry.text} />
            </div>
            {hasAttachments && (
              <div className="mb-2 flex flex-wrap gap-2">
                {entry.attachments!.map((att, idx) => {
                  if (att.media_type.startsWith('image/')) {
                    return (
                      <img
                        key={idx}
                        src={`data:${att.media_type};base64,${att.data}`}
                        alt={att.filename ?? 'Attachment'}
                        className="max-h-48 max-w-full rounded-md border border-rc-border-secondary object-contain"
                      />
                    );
                  }
                  return (
                    <div key={idx} className="flex items-center gap-1.5 rounded-md border border-rc-border-secondary bg-rc-bg-tertiary px-2 py-1 text-xs text-rc-text-secondary">
                      <FileText size={12} />
                      <span>{att.filename ?? att.media_type}</span>
                    </div>
                  );
                })}
              </div>
            )}
            {entry.text && <div className="whitespace-pre-wrap break-words">{entry.text}</div>}
          </div>
        </div>
      );
    }

    return <AssistantMessage entry={entry} />;
  },
  (previous, next) => previous.entry === next.entry,
);

// ── Working Indicator with elapsed timer (from CodexMonitor pattern) ──

function WorkingIndicator({ sending }: { sending: boolean }) {
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    if (!sending) {
      setElapsed(0);
      return;
    }
    const start = Date.now();
    const timer = setInterval(() => {
      setElapsed(Math.floor((Date.now() - start) / 1000));
    }, 1000);
    return () => clearInterval(timer);
  }, [sending]);

  if (!sending) return null;

  const minutes = Math.floor(elapsed / 60);
  const seconds = elapsed % 60;
  const timeStr = minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;

  return (
    <div className="flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-elevated px-3 py-2 text-xs text-rc-text-secondary shadow-xs animate-fade-in">
      <Loader2 size={14} className="animate-spin text-rc-accent-primary" />
      <span className="font-medium">{i18n.t('chatArea.processing')}</span>
      <span className="flex items-center gap-1 text-rc-text-tertiary">
        <Timer size={11} />
        {timeStr}
      </span>
    </div>
  );
}

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
        <div role="status" className="rounded-md border border-rc-border-primary bg-rc-bg-elevated px-4 py-3 text-sm text-rc-text-secondary shadow-xs">
          <div className="flex items-center gap-3">
            <div className="flex h-5 w-5 items-center justify-center">
              <div className="h-5 w-5 animate-spin rounded-full border-2 border-rc-border-primary border-t-rc-accent-primary" />
            </div>
            <span className="font-medium">{i18n.t('chatArea.processingRequest')}</span>
          </div>

          {compactProgress.length > 0 && (
            <div className="mt-4 space-y-2">
              {compactProgress.map((progress, index) => (
                <div
                  key={`${progress.tool_name}-${progress.tool_call_id}-${index}`}
                    className="flex items-center gap-2 rounded bg-rc-bg-code px-2.5 py-1.5 text-xs"
                >
                  <ToolIcon name={progress.tool_name} />
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
                  className={`flex items-center gap-2 rounded-md px-3 py-2 text-xs ${
                    result.is_error
                      ? 'bg-rc-accent-error-bg text-rc-accent-error'
                      : 'bg-rc-accent-success-bg text-rc-accent-success'
                  }`}
                >
                  {result.is_error ? <XCircle size={13} /> : <CheckCircle2 size={13} />}
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
        <div role="alert" className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-3 text-sm text-rc-accent-error">
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
  const [showScrollFab, setShowScrollFab] = useState(false);
  const shouldVirtualize = conversation.length >= VIRTUALIZATION_THRESHOLD;
  const getScrollElement = useCallback(() => scrollContainerRef.current, []);
  const estimateSize = useCallback((index: number) => {
    const entry = conversation[index];
    if (entry) return estimateEntryHeight(entry);
    if (conversation.length > 0) return estimateEntryHeight(conversation[0]);
    return 80;
  }, [conversation]);
  const rowVirtualizer = useVirtualizer({
    count: conversation.length,
    getScrollElement,
    estimateSize,
    overscan: VIRTUALIZATION_OVERSCAN,
    getItemKey: (index) => conversationRowKey(conversation[index] ?? conversation[0], index),
  });

  const handleScroll = useCallback(() => {
    const el = scrollContainerRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 120;
    setShowScrollFab(!atBottom);
  }, []);

  const scrollToBottom = useCallback(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }, [bottomRef]);

  return (
    <section
      ref={scrollContainerRef}
      onScroll={handleScroll}
      aria-label="Conversation transcript"
      className="flex-1 min-h-0 overflow-y-auto bg-rc-bg-chat px-5 py-5"
    >
      <div className="mx-auto flex w-full max-w-chat flex-col">
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
                  <div>
                    <MessageCard entry={entry} />
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          conversation.map((entry, index) => (
            <div key={conversationRowKey(entry, index)}>
              <MessageCard entry={entry} />
            </div>
          ))
        )}

        <WorkingIndicator sending={sending} />

        <StatusCards
          sending={sending}
          compactProgress={compactProgress}
          compactResults={compactResults}
          sendError={sendError}
          bottomRef={bottomRef}
        />
      </div>

      {showScrollFab && (
        <button
          type="button"
          onClick={scrollToBottom}
          aria-label="Scroll to bottom"
          className="absolute bottom-4 right-6 z-10 flex h-9 w-9 items-center justify-center rounded-full border border-rc-border-primary bg-rc-bg-surface shadow-lg transition-all hover:bg-rc-bg-hover hover:shadow-xl"
        >
          <ArrowDown size={16} className="text-rc-text-secondary" />
        </button>
      )}
    </section>
  );
}

function ConversationHeader({
  title,
  model,
  provider,
  sessionId,
}: {
  title: string;
  model?: string | null;
  provider?: string | null;
  sessionId?: string | null;
}) {
  const { t } = useTranslation();
  const [menuOpen, setMenuOpen] = useState(false);

  const copySessionId = () => {
    if (sessionId) void navigator.clipboard.writeText(sessionId);
    setMenuOpen(false);
  };
  const exportSession = () => {
    if (sessionId) void tauri.exportSessionBundle(sessionId, 'json');
    setMenuOpen(false);
  };

  return (
    <div className="flex h-12 shrink-0 items-center justify-between border-b border-rc-border-secondary bg-rc-bg-surface px-4">
      <div className="flex min-w-0 items-center gap-2">
        <div className="min-w-0 truncate text-sm font-semibold text-rc-text-primary">{title}</div>
        <div className="relative">
          <button
            type="button"
            aria-label={t('chatArea.sessionActions')}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
            onClick={() => setMenuOpen((v) => !v)}
          >
            <MoreHorizontal size={16} />
          </button>
          {menuOpen && (
            <>
              <button
                type="button"
                aria-label={t('chatInput.closeDropdown')}
                className="fixed inset-0 z-10 cursor-default"
                onClick={() => setMenuOpen(false)}
              />
              <div className="absolute left-0 top-full z-20 mt-1 min-w-[180px] overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-lg animate-fade-in-up">
                <div className="p-1.5">
                  <button
                    type="button"
                    onClick={exportSession}
                    className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-xs text-rc-text-primary hover:bg-rc-bg-hover"
                  >
                    <Copy size={13} className="text-rc-text-tertiary" />
                    {t('chatArea.exportSession')}
                  </button>
                  <button
                    type="button"
                    onClick={copySessionId}
                    className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-xs text-rc-text-primary hover:bg-rc-bg-hover"
                  >
                    <FileText size={13} className="text-rc-text-tertiary" />
                    {t('chatArea.copySessionId')}
                  </button>
                </div>
              </div>
            </>
          )}
        </div>
      </div>

      <div className="hidden min-w-0 items-center gap-2 rounded-lg border border-rc-border-secondary bg-rc-bg-elevated px-2.5 py-1.5 text-xs text-rc-text-tertiary md:flex">
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
  const { t } = useTranslation();
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const conversation = useAppStore((state) => state.conversation);
  const conversationLoading = useAppStore((state) => state.conversationLoading);
  const sending = useAppStore((state) => state.sending);
  const sendError = useAppStore((state) => state.sendError);
  const liveToolProgress = useAppStore((state) => state.liveToolProgress);
  const liveToolResults = useAppStore((state) => state.liveToolResults);
  const sessions = useAppStore((state) => state.sessions);
  const bottomRef = useRef<HTMLDivElement>(null);
  const lastScrollRef = useRef(0);

  const compactProgress = useMemo(() => liveToolProgress.slice(-6), [liveToolProgress]);
  const compactResults = useMemo(() => liveToolResults.slice(-4), [liveToolResults]);
  const activeSession = useMemo(
    () => sessions.find((session) => session.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );

  useEffect(() => {
    const now = Date.now();
    if (now - lastScrollRef.current < 100) return;
    lastScrollRef.current = now;
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }, [conversation, sending, liveToolProgress, liveToolResults]);

  if (!activeSessionId) {
    return <WorkspaceOverview />;
  }

  if (conversationLoading) {
    return (
      <div className="flex min-h-0 flex-1 flex-col bg-rc-bg-chat">
        <ConversationHeader
          title={activeSession?.title ?? 'Session'}
          provider={activeSession?.provider_name}
          model={activeSession?.model}
          sessionId={activeSessionId}
        />
        <div className="flex-1 overflow-y-auto">
          <EmptyState title={t('chatArea.loadingSession')} />
        </div>
      </div>
    );
  }

  if (conversation.length === 0) {
    return (
      <div className="flex min-h-0 flex-1 flex-col bg-rc-bg-chat">
        <ConversationHeader
          title={activeSession?.title ?? 'Session'}
          provider={activeSession?.provider_name}
          model={activeSession?.model}
          sessionId={activeSessionId}
        />
        <div className="flex-1 overflow-y-auto">
          <EmptyState title={t('chatArea.noMessages')} />
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-rc-bg-chat">
      <ConversationHeader
        title={activeSession?.title ?? 'Session'}
        provider={activeSession?.provider_name}
        model={activeSession?.model}
        sessionId={activeSessionId}
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
