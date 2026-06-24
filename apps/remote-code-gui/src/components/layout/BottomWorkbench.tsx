import {
  AlertTriangle,
  Bot,
  ChevronDown,
  Code2,
  ExternalLink,
  FileArchive,
  GitCompare,
  Globe2,
  LoaderCircle,
  TerminalSquare,
} from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { BottomWorkbenchTab } from '../../lib/useWorkbenchLayout';
import type { ConversationEntry } from '../../lib/types';
import { formatSensitivePath, truncateMiddle } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';
import { InlineDiffView } from '../chat/InlineDiffView';

interface BottomWorkbenchProps {
  open: boolean;
  activeTab: BottomWorkbenchTab;
  height: number;
  onTabChange: (tab: BottomWorkbenchTab) => void;
  onClose: () => void;
  onHeightChange: (height: number) => void;
}

type TFn = (key: string) => string;

function tabs(t: TFn): Array<{ key: BottomWorkbenchTab; label: string; icon: React.ElementType }> {
  return [
    { key: 'terminal', label: t('bottomWorkbench.terminal'), icon: TerminalSquare },
    { key: 'diff', label: t('bottomWorkbench.diff'), icon: GitCompare },
    { key: 'approvals', label: t('bottomWorkbench.approvals'), icon: AlertTriangle },
    { key: 'logs', label: t('bottomWorkbench.logs'), icon: Code2 },
    { key: 'artifacts', label: t('bottomWorkbench.artifacts'), icon: FileArchive },
    { key: 'browser', label: t('bottomWorkbench.browser'), icon: Globe2 },
  ];
}

function isDiffText(text: string) {
  return /(^|\n)(diff --git|---\s+\S+|\+\+\+\s+\S+|@@\s)/.test(text);
}

function collectDiffEntries(conversation: ConversationEntry[]) {
  return conversation
    .filter((entry) => isDiffText(entry.text))
    .map((entry, index) => ({
      id: `${entry.role}-${entry.tool_call_id ?? index}`,
      title: entry.name ?? entry.tool_call_id ?? `diff-${index + 1}`,
      text: entry.text,
    }));
}

function collectAttachments(conversation: ConversationEntry[]) {
  return conversation.flatMap((entry) => entry.attachments ?? []);
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="flex h-full min-h-0 items-center justify-center p-6 text-center">
      <div className="max-w-sm">
        <div className="text-sm font-medium text-rc-text-primary">{title}</div>
        <p className="mt-2 text-xs leading-5 text-rc-text-tertiary">{body}</p>
      </div>
    </div>
  );
}

export function BottomWorkbench({
  open,
  activeTab,
  height,
  onTabChange,
  onClose,
  onHeightChange,
}: BottomWorkbenchProps) {
  const { t } = useTranslation();
  const conversation = useAppStore((state) => state.conversation);
  const liveToolProgress = useAppStore((state) => state.liveToolProgress);
  const liveToolResults = useAppStore((state) => state.liveToolResults);
  const pendingPermission = useAppStore((state) => state.pendingPermission);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const settings = useAppStore((state) => state.settings);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);

  const diffEntries = useMemo(() => collectDiffEntries(conversation), [conversation]);
  const attachments = useMemo(() => collectAttachments(conversation), [conversation]);

  if (!open) return null;

  const startResize = (event: React.MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    const startY = event.clientY;
    const startHeight = height;
    const onMove = (moveEvent: MouseEvent) => {
      onHeightChange(startHeight + startY - moveEvent.clientY);
    };
    const onUp = () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  };

  return (
    <section
      aria-label={t('bottomWorkbench.title')}
      className="mx-4 mb-4 mt-2 flex shrink-0 flex-col overflow-hidden rounded-[28px] border border-rc-border-secondary bg-rc-bg-surface/90 shadow-2xl backdrop-blur-2xl"
      style={{ height }}
    >
      <div
        role="separator"
        aria-orientation="horizontal"
        aria-label={t('bottomWorkbench.resize')}
        onMouseDown={startResize}
        className="flex h-4 cursor-row-resize items-center justify-center bg-transparent transition-colors hover:bg-rc-bg-hover"
      />
      <header className="flex h-12 shrink-0 items-center justify-between border-b border-rc-border-secondary/70 bg-rc-bg-elevated/50 px-3">
        <div role="tablist" aria-label={t('bottomWorkbench.tabs')} className="flex min-w-0 items-center gap-1.5">
          {tabs(t).map((tab) => {
            const Icon = tab.icon;
            const selected = activeTab === tab.key;
            return (
              <button
                key={tab.key}
                type="button"
                role="tab"
                aria-selected={selected}
                onClick={() => onTabChange(tab.key)}
                className={`inline-flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-medium transition-all ${
                  selected
                    ? 'bg-rc-bg-surface text-rc-text-primary shadow-sm'
                    : 'text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary hover:shadow-xs'
                }`}
              >
                <Icon size={13} />
                {tab.label}
              </button>
            );
          })}
        </div>
        <button
          type="button"
          onClick={onClose}
          className="inline-flex h-8 items-center gap-1 rounded-full px-3 text-xs text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
          aria-label={t('bottomWorkbench.close')}
        >
          <ChevronDown size={14} />
          {t('bottomWorkbench.collapse')}
        </button>
      </header>

      <div className="min-h-0 flex-1 overflow-auto bg-transparent">
        {activeTab === 'terminal' && (
          <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_260px]">
            <div className="min-h-0 overflow-auto p-3 font-mono text-xs">
              {liveToolProgress.length === 0 && liveToolResults.length === 0 ? (
                <EmptyState title={t('bottomWorkbench.terminalEmpty')} body={t('bottomWorkbench.terminalEmptyDesc')} />
              ) : (
                <div className="space-y-2">
                  {liveToolProgress.map((item) => (
                    <div key={item.tool_call_id} className="rounded-2xl border border-rc-border-secondary bg-rc-bg-surface/80 p-3 shadow-xs">
                      <div className="flex items-center gap-2 text-rc-accent-warning">
                        <LoaderCircle size={13} className="animate-spin" />
                        {item.tool_name}
                      </div>
                      <div className="mt-2 text-rc-text-secondary">{item.message}</div>
                      {item.active_form && <pre className="mt-2 whitespace-pre-wrap text-rc-text-tertiary">{item.active_form}</pre>}
                    </div>
                  ))}
                  {liveToolResults.map((item) => (
                    <div key={item.tool_call_id} className="rounded-2xl border border-rc-border-secondary bg-rc-bg-surface/80 p-3 shadow-xs">
                      <div className={item.is_error ? 'text-rc-accent-error' : 'text-rc-accent-success'}>{item.tool_name}</div>
                      <pre className="mt-2 whitespace-pre-wrap text-rc-text-secondary">{truncateMiddle(item.output, 4000)}</pre>
                    </div>
                  ))}
                </div>
              )}
            </div>
            <aside className="border-l border-rc-border-secondary/70 bg-rc-bg-elevated/35 p-4 text-xs">
              <div className="text-[10px] font-semibold uppercase tracking-wide text-rc-text-tertiary">{t('bottomWorkbench.session')}</div>
              <div className="mt-1 font-mono text-rc-text-secondary">{activeSessionId?.slice(0, 12) ?? '—'}</div>
              <div className="mt-4 text-[10px] font-semibold uppercase tracking-wide text-rc-text-tertiary">{t('bottomWorkbench.cwd')}</div>
              <div className="mt-1 break-all font-mono text-rc-text-secondary">{activeProjectPath ? formatSensitivePath(activeProjectPath, privacyMode) : '—'}</div>
            </aside>
          </div>
        )}

        {activeTab === 'diff' && (
          <div className="h-full min-h-0 overflow-auto p-3">
            {diffEntries.length === 0 ? (
              <EmptyState title={t('bottomWorkbench.diffEmpty')} body={t('bottomWorkbench.diffEmptyDesc')} />
            ) : (
              <div className="space-y-3">
                {diffEntries.map((entry) => (
                  <div key={entry.id} className="overflow-hidden rounded-2xl border border-rc-border-secondary bg-rc-bg-surface/80 shadow-xs">
                    <div className="border-b border-rc-border-secondary/70 px-3 py-2 text-xs font-medium text-rc-text-secondary">{entry.title}</div>
                    <InlineDiffView content={entry.text} />
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === 'approvals' && (
          <div className="h-full min-h-0 overflow-auto p-3">
            {pendingPermission ? (
              <div className="rounded-2xl border border-rc-accent-warning-border bg-rc-accent-warning-bg p-4 shadow-xs">
                <div className="text-sm font-semibold text-rc-text-primary">{pendingPermission.title}</div>
                <p className="mt-2 text-xs leading-5 text-rc-text-secondary">{pendingPermission.description}</p>
                {pendingPermission.blocked_path && (
                  <div className="mt-3 rounded-2xl bg-rc-bg-surface px-3 py-2 font-mono text-xs text-rc-text-tertiary">
                    {formatSensitivePath(pendingPermission.blocked_path, privacyMode)}
                  </div>
                )}
              </div>
            ) : (
              <EmptyState title={t('bottomWorkbench.approvalsEmpty')} body={t('bottomWorkbench.approvalsEmptyDesc')} />
            )}
          </div>
        )}

        {activeTab === 'logs' && (
          <div className="h-full min-h-0 overflow-auto p-3 font-mono text-xs">
            <div className="space-y-2">
              {conversation.slice(-20).map((entry, index) => (
                <div key={`${entry.role}-${entry.tool_call_id ?? index}`} className="grid grid-cols-[88px_minmax(0,1fr)] gap-3 rounded-2xl border border-rc-border-secondary bg-rc-bg-surface/80 px-3 py-2 shadow-xs">
                  <span className="text-rc-text-tertiary">{entry.role}</span>
                  <span className="truncate text-rc-text-secondary">{entry.name ?? entry.tool_call_id ?? truncateMiddle(entry.text.replace(/\s+/g, ' '), 180)}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {activeTab === 'artifacts' && (
          <div className="h-full min-h-0 overflow-auto p-3">
            {attachments.length === 0 ? (
              <EmptyState title={t('bottomWorkbench.artifactsEmpty')} body={t('bottomWorkbench.artifactsEmptyDesc')} />
            ) : (
              <div className="grid gap-2 md:grid-cols-2">
                {attachments.map((attachment, index) => (
                  <div key={`${attachment.filename ?? attachment.media_type}-${index}`} className="rounded-2xl border border-rc-border-secondary bg-rc-bg-surface/80 p-3 shadow-xs">
                    <div className="text-sm font-medium text-rc-text-primary">{attachment.filename ?? t('bottomWorkbench.artifact')}</div>
                    <div className="mt-1 break-all font-mono text-xs text-rc-text-tertiary">{attachment.media_type}</div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === 'browser' && (
          <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_280px]">
            <div className="flex min-h-0 items-center justify-center bg-[radial-gradient(circle_at_50%_20%,rgba(109,143,123,0.16),transparent_34%),linear-gradient(135deg,rgba(255,255,255,0.2),transparent)] p-6">
              <div className="max-w-md rounded-3xl border border-rc-border-secondary bg-rc-bg-surface/86 p-6 text-center shadow-lg">
                <Globe2 size={24} className="mx-auto text-rc-accent-info" />
                <div className="mt-3 text-sm font-semibold text-rc-text-primary">{t('bottomWorkbench.browserPreview')}</div>
                <p className="mt-2 text-xs leading-5 text-rc-text-tertiary">{t('bottomWorkbench.browserPreviewDesc')}</p>
              </div>
            </div>
            <aside className="border-l border-rc-border-secondary/70 bg-rc-bg-elevated/35 p-4">
              <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-rc-text-tertiary">
                <Bot size={13} />
                {t('bottomWorkbench.browserComments')}
              </div>
              <div className="mt-3 rounded-2xl border border-dashed border-rc-border-secondary p-3 text-xs leading-5 text-rc-text-tertiary">
                {t('bottomWorkbench.browserCommentsDesc')}
              </div>
              <button type="button" className="mt-3 inline-flex items-center gap-1.5 rounded-full border border-rc-border-primary px-3 py-2 text-xs text-rc-text-secondary opacity-60">
                <ExternalLink size={13} />
                {t('bottomWorkbench.openPreview')}
              </button>
            </aside>
          </div>
        )}
      </div>
    </section>
  );
}
