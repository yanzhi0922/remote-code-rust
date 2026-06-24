import {
  Clipboard as ClipboardIcon,
  Cpu,
  FolderOpen,
  Layers,
  Network,
  Shield,
  Wifi,
  WifiOff,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { CodexTimelineKind } from '../../lib/codexTimeline';
import { collectCodexSurfaceStats } from '../../lib/codexTimeline';
import { formatSensitivePath, truncateMiddle } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';

// ── Utility ──────────────────────────────────────────────────────────────

function formatTokenCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatRelativeTimeShort(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 1) return 'just now';
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return new Date(iso).toLocaleDateString();
}

// ── Popover panel (shared) ───────────────────────────────────────────────

function SegmentPopover({
  open,
  onClose,
  children,
  label,
}: {
  open: boolean;
  onClose: () => void;
  children: React.ReactNode;
  label: string;
}) {
  if (!open) return null;
  return (
    <>
      <button
        type="button"
        aria-label={`Close ${label}`}
        className="fixed inset-0 z-30 cursor-default"
        onClick={onClose}
      />
      <div
        role="dialog"
        aria-label={label}
        className="codex-popover absolute bottom-full left-0 z-40 mb-2 w-80 animate-fade-in-up"
      >
        {children}
      </div>
    </>
  );
}

// ── Segment chip (clickable capsule) ─────────────────────────────────────

function SegmentChip({
  icon: Icon,
  label,
  value,
  warning,
  active,
  onClick,
  shortcut,
}: {
  icon: React.ElementType;
  label: string;
  value: string;
  warning?: boolean;
  active?: boolean;
  onClick: () => void;
  /** Optional keyboard hint surfaced via aria-keyshortcuts + tooltip. */
  shortcut?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={shortcut ? `${label} (${shortcut})` : label}
      aria-label={shortcut ? `${label} (${shortcut})` : label}
      aria-keyshortcuts={shortcut}
      aria-expanded={active ?? false}
      className={`flex items-center gap-1.5 rounded-md px-2 py-0.5 text-[11px] transition-colors ${
        active
          ? 'bg-rc-bg-surface text-rc-text-primary'
          : 'text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary'
      } ${warning ? 'text-rc-accent-warning' : ''}`}
    >
      <Icon size={12} className={warning ? 'text-rc-accent-warning' : 'text-rc-text-tertiary'} />
      <span className="max-w-[160px] truncate">{value}</span>
      {shortcut && (
        <span className="hidden rounded bg-rc-bg-tertiary px-1 font-mono text-[9px] text-rc-text-tertiary lg:inline">
          {shortcut}
        </span>
      )}
    </button>
  );
}

function WorkbenchChip({
  icon: Icon,
  label,
  onClick,
  warning,
}: {
  icon: React.ElementType;
  label: string;
  onClick: () => void;
  warning?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={label}
      className={`flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] transition-colors ${
        warning
          ? 'text-rc-accent-warning hover:bg-rc-accent-warning-bg'
          : 'text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary'
      }`}
    >
      <Icon size={12} />
      <span className="hidden lg:inline">{label}</span>
    </button>
  );
}
// ── Detail panels ────────────────────────────────────────────────────────

function ProjectDetail({
  projectPath,
  projectName,
  privacyMode,
  providerName,
  modelName,
}: {
  projectPath: string | null;
  projectName: string;
  privacyMode: boolean;
  providerName: string;
  modelName: string;
}) {
  return (
    <div className="space-y-3 p-3">
      <div>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
          Project
        </div>
        <div className="mt-1 text-sm text-rc-text-primary">
          {projectPath ? formatSensitivePath(projectPath, privacyMode) : 'No project'}
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Provider
          </div>
          <div className="mt-0.5 text-sm text-rc-text-primary">{providerName}</div>
        </div>
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Model
          </div>
          <div className="mt-0.5 font-mono text-sm text-rc-text-primary">{modelName}</div>
        </div>
      </div>
      <div>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
          Project name
        </div>
        <div className="mt-0.5 text-sm text-rc-text-primary">{projectName}</div>
      </div>
    </div>
  );
}

function ThreadDetail({
  sessionTitle,
  sessionId,
  sessionUpdatedAt,
  agentType,
  conversationLength,
  timelineStats,
}: {
  sessionTitle: string;
  sessionId: string | null;
  sessionUpdatedAt?: string;
  agentType: string;
  conversationLength: number;
  timelineStats: Record<CodexTimelineKind, number>;
}) {
  return (
    <div className="space-y-3 p-3">
      <div>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
          Session
        </div>
        <div className="mt-1 text-sm text-rc-text-primary">{sessionTitle}</div>
        {sessionId && (
          <div className="mt-0.5 font-mono text-xs text-rc-text-tertiary" title={sessionId}>
            {sessionId.slice(0, 12)}…
          </div>
        )}
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Agent
          </div>
          <div className="mt-0.5 text-sm text-rc-text-primary">{agentType}</div>
        </div>
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Messages
          </div>
          <div className="mt-0.5 text-sm text-rc-text-primary">{conversationLength}</div>
        </div>
      </div>
      <div className="flex items-center justify-between gap-2 border-t border-rc-border-secondary/40 pt-2">
        <div className="text-[10px] text-rc-text-tertiary">
          {sessionUpdatedAt ? `Last active ${formatRelativeTimeShort(sessionUpdatedAt)}` : 'Idle'}
        </div>
        <button
          type="button"
          data-testid="statusbar-thread-reveal"
          onClick={() => {
            if (sessionId) {
              navigator.clipboard.writeText(sessionId).catch(() => {});
            }
          }}
          className="inline-flex items-center gap-1 rounded-md border border-rc-border-primary px-2 py-0.5 text-[10px] font-medium text-rc-text-secondary hover:bg-rc-bg-hover"
        >
          <ClipboardIcon size={9} />
          Copy ID
        </button>
      </div>
      <div>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
          Timeline
        </div>
        <div className="mt-1 grid grid-cols-4 gap-1.5 text-center text-[10px]">
          <div className="rounded bg-rc-bg-tertiary px-1.5 py-1">
            <div className="font-mono text-rc-text-primary">{timelineStats.command}</div>
            <div className="text-rc-text-tertiary">cmd</div>
          </div>
          <div className="rounded bg-rc-bg-tertiary px-1.5 py-1">
            <div className="font-mono text-rc-text-primary">{timelineStats.file}</div>
            <div className="text-rc-text-tertiary">file</div>
          </div>
          <div className="rounded bg-rc-bg-tertiary px-1.5 py-1">
            <div className="font-mono text-rc-text-primary">{timelineStats.mcp}</div>
            <div className="text-rc-text-tertiary">mcp</div>
          </div>
          <div className="rounded bg-rc-bg-tertiary px-1.5 py-1">
            <div className="font-mono text-rc-text-primary">{timelineStats.reasoning}</div>
            <div className="text-rc-text-tertiary">think</div>
          </div>
        </div>
      </div>
    </div>
  );
}

function PermissionDetail({
  permissionMode,
  agentType,
  allowedTools,
  disallowedTools,
}: {
  permissionMode: string;
  agentType: string;
  allowedTools: number;
  disallowedTools: number;
}) {
  return (
    <div className="space-y-3 p-3">
      <div>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
          Permission Mode
        </div>
        <div className="mt-1 flex items-center gap-1.5 text-sm text-rc-text-primary">
          <Shield size={13} className="text-rc-text-tertiary" />
          {permissionMode}
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Agent
          </div>
          <div className="mt-0.5 text-sm text-rc-text-primary">{agentType}</div>
        </div>
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Tools
          </div>
          <div className="mt-0.5 text-sm">
            <span className="text-rc-accent-success">{allowedTools}</span>
            <span className="text-rc-text-tertiary"> allowed · </span>
            <span className="text-rc-accent-error">{disallowedTools}</span>
            <span className="text-rc-text-tertiary"> blocked</span>
          </div>
        </div>
      </div>
    </div>
  );
}

function McpDetail({
  connected,
  enabled,
  failed,
  needsAuth,
  warnings,
  enabledSevers,
}: {
  connected: number;
  enabled: number;
  failed: number;
  needsAuth: number;
  warnings: number;
  enabledSevers: number;
}) {
  return (
    <div className="space-y-3 p-3">
      <div>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
          MCP Servers
        </div>
        <div className="mt-1 grid grid-cols-2 gap-3 text-sm">
          <div>
            <div className="text-rc-text-tertiary text-[10px] uppercase tracking-wider">Connected</div>
            <div className="font-mono text-rc-accent-success">{connected}</div>
          </div>
          <div>
            <div className="text-rc-text-tertiary text-[10px] uppercase tracking-wider">Enabled</div>
            <div className="font-mono text-rc-text-primary">{enabled} / {enabledSevers}</div>
          </div>
          {failed > 0 && (
            <div>
              <div className="text-rc-text-tertiary text-[10px] uppercase tracking-wider">Failed</div>
              <div className="font-mono text-rc-accent-error">{failed}</div>
            </div>
          )}
          {needsAuth > 0 && (
            <div>
              <div className="text-rc-text-tertiary text-[10px] uppercase tracking-wider">Needs auth</div>
              <div className="font-mono text-rc-accent-warning">{needsAuth}</div>
            </div>
          )}
        </div>
      </div>
      {warnings > 0 && (
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Warnings
          </div>
          <div className="mt-0.5 font-mono text-sm text-rc-accent-warning">{warnings}</div>
        </div>
      )}
    </div>
  );
}

function ContextDetail({
  ratio,
  estimatedTokens,
  maxInputTokens,
  thresholdTokens,
  inputTokens,
  outputTokens,
  totalTokens,
}: {
  ratio: number;
  estimatedTokens: number;
  maxInputTokens: number;
  thresholdTokens: number;
  inputTokens?: number;
  outputTokens?: number;
  totalTokens?: number;
}) {
  const percent = Math.round(ratio * 100);
  const color =
    percent > 90
      ? 'bg-rc-accent-error'
      : percent > 75
        ? 'bg-rc-accent-warning'
        : 'bg-rc-accent-success';

  return (
    <div className="space-y-3 p-3">
      <div>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
          Context Window
        </div>
        <div className="mt-1.5 h-2 w-full overflow-hidden rounded-full bg-rc-bg-tertiary">
          <div
            className={`h-full rounded-full transition-all duration-500 ${color}`}
            style={{ width: `${Math.min(percent, 100)}%` }}
          />
        </div>
        <div className="mt-1 flex justify-between text-xs text-rc-text-tertiary">
          <span>{formatTokenCount(estimatedTokens)} used</span>
          <span>{formatTokenCount(maxInputTokens)} max</span>
        </div>
      </div>
      <div>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
          Last turn
        </div>
        <div className="mt-0.5 grid grid-cols-3 gap-2 text-xs">
          <div>
            <div className="text-rc-text-tertiary">In</div>
            <div className="font-mono text-rc-accent-info">{formatTokenCount(inputTokens ?? 0)}</div>
          </div>
          <div>
            <div className="text-rc-text-tertiary">Out</div>
            <div className="font-mono text-rc-accent-success">{formatTokenCount(outputTokens ?? 0)}</div>
          </div>
          <div>
            <div className="text-rc-text-tertiary">Total</div>
            <div className="font-mono text-rc-text-primary">{formatTokenCount(totalTokens ?? 0)}</div>
          </div>
        </div>
      </div>
      {thresholdTokens > 0 && (
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Compaction Threshold
          </div>
          <div className="mt-0.5 font-mono text-sm text-rc-text-primary">
            {formatTokenCount(thresholdTokens)}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Main StatusBar ───────────────────────────────────────────────────────

export function StatusBar() {
  const { t } = useTranslation();
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const sessions = useAppStore((state) => state.sessions);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);
  const lastPromptResult = useAppStore((state) => state.lastPromptResult);
  const settings = useAppStore((state) => state.settings);
  const conversation = useAppStore((state) => state.conversation);
  const pendingPermission = useAppStore((state) => state.pendingPermission);

  const [expandedSegment, setExpandedSegment] = useState<
    'project' | 'thread' | 'permission' | 'mcp' | 'context' | null
  >(null);
  const barRef = useRef<HTMLDivElement>(null);

  // Close the popover when clicking outside the bar.
  useEffect(() => {
    if (!expandedSegment) return;
    const handle = (event: MouseEvent) => {
      if (!barRef.current?.contains(event.target as Node)) {
        setExpandedSegment(null);
      }
    };
    document.addEventListener('mousedown', handle);
    return () => document.removeEventListener('mousedown', handle);
  }, [expandedSegment]);

  // Close the popover on Escape.
  useEffect(() => {
    if (!expandedSegment) return;
    const handle = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setExpandedSegment(null);
    };
    window.addEventListener('keydown', handle);
    return () => window.removeEventListener('keydown', handle);
  }, [expandedSegment]);

  const toggleSegment = useCallback(
    (segment: 'project' | 'thread' | 'permission' | 'mcp' | 'context') => {
      setExpandedSegment((prev) => (prev === segment ? null : segment));
    },
    [],
  );

  const activeSession = useMemo(
    () => sessions.find((s) => s.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );

  const contextUsage = activeSessionId ? contextUsageBySession[activeSessionId] : null;

  const modelName = activeSession?.model
    ?? runtimeStatus?.provider.model
    ?? provider?.model
    ?? '—';

  const agentLabel = activeAgentType ?? 'remote_claude';
  const providerName = runtimeStatus?.provider.name ?? provider?.name ?? '—';
  const mcpSummary = runtimeStatus?.mcp ?? null;
  const mcpIssueCount = mcpSummary
    ? mcpSummary.status_counts.failed + mcpSummary.status_counts.needs_auth + mcpSummary.warning_count
    : 0;
  const mcpLabel = mcpSummary
    ? `MCP ${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers}`
    : 'MCP —';

  const projectLabel = activeProjectPath
    ? truncateMiddle(activeProjectPath, 32)
    : t('statusBar.noProject');
  const projectName = activeProjectPath
    ? activeProjectPath.split(/[\\/]/).pop() ?? ''
    : '';
  const sessionLabel = activeSession
    ? truncateMiddle(privacyMode ? t('statusBar.hiddenSession') : activeSession.title, 28)
    : t('statusBar.noSession');

  const permissionMode = settings?.permission_mode ?? '—';
  const allowedTools = runtimeStatus?.allowed_tools?.length ?? 0;
  const disallowedTools = runtimeStatus?.disallowed_tools?.length ?? 0;
  const contextWarning = !!contextUsage && contextUsage.ratio > 0.75;
  const contextRatio = contextUsage?.ratio ?? 0;
  const contextLabel = contextUsage
    ? `${formatTokenCount(contextUsage.estimated_tokens)} / ${formatTokenCount(contextUsage.max_input_tokens)}`
    : 'ctx —';
  const timelineStats = useMemo(() => collectCodexSurfaceStats(conversation), [conversation]);

  return (
    <div
      ref={barRef}
      role="toolbar"
      aria-label={t('statusBar.title')}
      data-tauri-drag-region=""
      className="pointer-events-auto absolute bottom-6 left-1/2 z-20 hidden -translate-x-1/2 items-center gap-1 rounded-lg border border-rc-border-primary bg-rc-bg-elevated p-1 text-[11px] text-rc-text-tertiary opacity-35 shadow-sm transition-opacity hover:opacity-100 focus-within:opacity-100 select-none lg:flex"
    >
      <div className="relative">
        <SegmentChip
          icon={FolderOpen}
          label={t('statusBar.project')}
          value={projectLabel}
          onClick={() => toggleSegment('project')}
          active={expandedSegment === 'project'}
        />
        <SegmentPopover
          open={expandedSegment === 'project'}
          onClose={() => setExpandedSegment(null)}
          label={t('statusBar.project')}
        >
          <ProjectDetail
            projectPath={activeProjectPath}
            projectName={projectName}
            privacyMode={privacyMode}
            providerName={providerName}
            modelName={modelName}
          />
        </SegmentPopover>
      </div>

      <span className="mx-0.5 h-3 w-px bg-rc-border-primary/40" />

      <div className="relative">
        <SegmentChip
          icon={Layers}
          label={t('statusBar.session')}
          value={sessionLabel}
          onClick={() => toggleSegment('thread')}
          active={expandedSegment === 'thread'}
        />
        <SegmentPopover
          open={expandedSegment === 'thread'}
          onClose={() => setExpandedSegment(null)}
          label={t('statusBar.session')}
        >
          <ThreadDetail
            sessionTitle={activeSession?.title ?? t('statusBar.noSession')}
            sessionId={activeSessionId}
            sessionUpdatedAt={activeSession?.updated_at}
            agentType={agentLabel}
            conversationLength={conversation.length}
            timelineStats={timelineStats}
          />
        </SegmentPopover>
      </div>

      <span className="mx-0.5 h-3 w-px bg-rc-border-primary/40" />

      <div className="relative">
        <SegmentChip
          icon={Shield}
          label={t('statusBar.permission')}
          value={permissionMode}
          onClick={() => toggleSegment('permission')}
          active={expandedSegment === 'permission'}
        />
        <SegmentPopover
          open={expandedSegment === 'permission'}
          onClose={() => setExpandedSegment(null)}
          label={t('statusBar.permission')}
        >
          <PermissionDetail
            permissionMode={permissionMode}
            agentType={agentLabel}
            allowedTools={allowedTools}
            disallowedTools={disallowedTools}
          />
        </SegmentPopover>
      </div>

      {mcpSummary && (
        <>
          <span className="mx-0.5 h-3 w-px bg-rc-border-primary/40" />
          <div className="relative">
            <SegmentChip
              icon={Network}
              label="MCP"
              value={mcpLabel}
              warning={mcpIssueCount > 0}
              onClick={() => toggleSegment('mcp')}
              active={expandedSegment === 'mcp'}
            />
            <SegmentPopover
              open={expandedSegment === 'mcp'}
              onClose={() => setExpandedSegment(null)}
              label="MCP"
            >
              <McpDetail
                connected={mcpSummary.status_counts.connected}
                enabled={mcpSummary.status_counts.connected + mcpSummary.status_counts.failed + mcpSummary.status_counts.needs_auth + mcpSummary.status_counts.pending}
                failed={mcpSummary.status_counts.failed}
                needsAuth={mcpSummary.status_counts.needs_auth}
                warnings={mcpSummary.warning_count}
                enabledSevers={mcpSummary.enabled_servers}
              />
            </SegmentPopover>
          </div>
        </>
      )}

      <span className="mx-0.5 h-3 w-px bg-rc-border-primary/40" />

      <div className="relative">
        <SegmentChip
          icon={Cpu}
          label={t('statusBar.context')}
          value={contextLabel}
          warning={contextWarning}
          onClick={() => toggleSegment('context')}
          active={expandedSegment === 'context'}
        />
        <SegmentPopover
          open={expandedSegment === 'context'}
          onClose={() => setExpandedSegment(null)}
          label={t('statusBar.context')}
        >
          <ContextDetail
            ratio={contextRatio}
            estimatedTokens={contextUsage?.estimated_tokens ?? 0}
            maxInputTokens={contextUsage?.max_input_tokens ?? 0}
            thresholdTokens={contextUsage?.threshold_tokens ?? 0}
            inputTokens={lastPromptResult?.usage.input_tokens}
            outputTokens={lastPromptResult?.usage.output_tokens}
            totalTokens={lastPromptResult?.usage.total_tokens}
          />
        </SegmentPopover>
      </div>

      <span className="mx-0.5 h-3 w-px bg-rc-border-primary/40" />

      <div className="flex items-center gap-1.5 px-1 text-rc-text-tertiary" title={runtimeStatus ? t('statusBar.online') : t('statusBar.offline')}>
        {runtimeStatus ? (
          <Wifi size={12} className="text-rc-accent-success" />
        ) : (
          <WifiOff size={12} />
        )}
      </div>
    </div>
  );
}
