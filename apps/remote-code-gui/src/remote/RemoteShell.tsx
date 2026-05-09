/**
 * RemoteShell — 远程 Web 端的布局壳层组件。
 *
 * 负责：
 * - 左侧 session 列表（桌面侧边栏 / 移动端抽屉）
 * - 顶部 header（连接状态、会话标题）
 * - 错误/状态消息横幅
 * - mobile / desktop 响应式容器布局
 *
 * 不负责：
 * - 认证流程（由 RemoteAuthGate 处理）
 * - transport / WebSocket 生命周期（由 transport.ts 处理）
 * - 时间线渲染（由 children 传入）
 */

import {
  LoaderCircle,
  LogOut,
  Menu,
  RotateCcw,
  Wifi,
  WifiOff,
  X,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { cn, truncateMiddle } from '../lib/utils';
import {
  formatRemoteRelativeTime,
  type RemoteConnectionState,
  type RemoteCopy,
  type RemoteLocale,
  resolveRemoteLocale,
} from './i18n';
import { resolveRemoteSessionTitle } from '../session/normalize/fromRemote';
import type {
  RemoteSessionRecord,
  RemoteSessionState,
} from './types';

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface RemoteShellProps {
  sessions: RemoteSessionRecord[];
  sessionsLoading: boolean;
  activeSessionId: string | null;
  activeSession: RemoteSessionRecord | null;
  connectionState: RemoteConnectionState;
  sidebarOpen: boolean;
  errorMessage: string | null;
  statusMessage: string | null;
  baseUrl: string;
  copy: RemoteCopy;
  locale: RemoteLocale;
  transportStrategy: string | null;
  transportLatencyMs: number | null;

  onToggleSidebar: (open: boolean) => void;
  onSelectSession: (sessionId: string) => void;
  onRefreshSessions: () => void;
  onSignOut: () => void;

  /** 主内容区域（时间线 + 侧面板） */
  children: ReactNode;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function RemoteShell({
  sessions,
  sessionsLoading,
  activeSessionId,
  activeSession,
  connectionState,
  sidebarOpen,
  errorMessage,
  statusMessage,
  baseUrl,
  copy,
  locale,
  onToggleSidebar,
  onSelectSession,
  onRefreshSessions,
  onSignOut,
  transportStrategy,
  transportLatencyMs,
  children,
}: RemoteShellProps) {
  return (
    <RemoteFrame>
      {sidebarOpen && (
        <button
          aria-label={copy.selectRemoteSession}
          className="fixed inset-0 z-30 bg-slate-950/30 lg:hidden"
          onClick={() => onToggleSidebar(false)}
        />
      )}

      <div className="mx-auto flex min-h-screen max-w-[1580px] flex-col lg:flex-row">
        {/* ── Session sidebar ── */}
        <aside
          className={cn(
            'fixed inset-y-0 left-0 z-40 w-[320px] transform border-r border-[#e5ddcf] bg-[#f5efe4] transition-transform lg:static lg:z-0 lg:translate-x-0',
            sidebarOpen ? 'translate-x-0' : '-translate-x-full',
          )}
        >
          <div className="border-b border-[#e5ddcf] px-5 py-5">
            <div className="text-[11px] font-semibold uppercase tracking-[0.28em] text-slate-400">
              {copy.remoteShellEyebrow}
            </div>
            <div className="mt-2 text-2xl font-semibold text-slate-900">remote-code</div>
            <div className="mt-3 text-sm leading-6 text-slate-500">
              {copy.remoteShellDescription}
            </div>
            <button
              type="button"
              onClick={onRefreshSessions}
              className="mt-4 inline-flex items-center gap-2 rounded-full border border-[#ddd4c5] bg-white px-3 py-1.5 text-sm text-slate-700 transition-colors hover:bg-[#faf6ef]"
            >
              <RotateCcw size={14} />
              {copy.refreshSessions}
            </button>
          </div>

          <div className="h-[calc(100vh-181px)] overflow-y-auto px-3 py-4">
            {sessionsLoading ? (
              <div role="status" className="flex items-center gap-2 rounded-2xl bg-white/80 px-4 py-3 text-sm text-slate-500">
                <LoaderCircle size={16} className="animate-spin" />
                {copy.loadingRemoteSessions}
              </div>
            ) : sessions.length === 0 ? (
              <EmptyCard
                title={copy.noSessionsTitle}
                description={copy.noSessionsDescription}
              />
            ) : (
              <div className="space-y-2">
                {sessions.map((session) => {
                  const selected = session.session_id === activeSessionId;
                  return (
                    <button
                      key={session.session_id}
                      type="button"
                      onClick={() => onSelectSession(session.session_id)}
                      className={cn(
                        'w-full rounded-[22px] border px-4 py-3 text-left transition-colors',
                        selected
                          ? 'border-[#d7cdbe] bg-white shadow-[0_12px_28px_rgba(34,32,28,0.08)]'
                          : 'border-transparent bg-white/60 hover:bg-white',
                      )}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="truncate text-sm font-semibold text-slate-900">
                            {resolveRemoteSessionTitle(session)}
                          </div>
                          <div className="mt-1 text-xs text-slate-500">
                            {truncateMiddle(session.workspace_id, 48)}
                          </div>
                        </div>
                        <StatePill copy={copy} state={session.state} />
                      </div>
                      <div className="mt-3 flex items-center gap-2 text-[11px] text-slate-500">
                        <span>{formatRelativeTime(session.updated_at, locale, copy)}</span>
                        {session.metadata.agent_type && (
                          <>
                            <span>•</span>
                            <span className="rounded bg-slate-200 px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider text-slate-600">
                              {session.metadata.agent_type}
                            </span>
                          </>
                        )}
                        <span>•</span>
                        <span>
                          {session.owner_runner_id ?? copy.runnerUnassigned}
                          {session.owner_runner_id && session.owner_runner_available === false
                            ? ` · ${copy.runnerOfflineLabel}`
                            : ''}
                        </span>
                      </div>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </aside>

        {/* ── Main content area ── */}
        <div className="flex min-h-screen min-w-0 flex-1 flex-col">
          <header className="border-b border-[#e5ddcf] bg-white/90 px-4 py-3 backdrop-blur sm:px-6 sm:py-4">
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3">
                <button
                  type="button"
                  aria-label={copy.openSessionDrawer}
                  className="inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-[#ddd4c5] bg-[#faf6ef] text-slate-700 lg:hidden"
                  onClick={() => onToggleSidebar(true)}
                >
                  <Menu size={18} />
                </button>
                <div className="min-w-0">
                  <div className="truncate text-base font-semibold text-slate-900 sm:text-lg">
                    {activeSession ? resolveRemoteSessionTitle(activeSession) : copy.selectRemoteSession}
                  </div>
                  <div className="mt-0.5 hidden items-center gap-2 text-sm text-slate-500 sm:flex">
                    <span>{truncateMiddle(baseUrl, 48)}</span>
                    {activeSession && (
                      <>
                        <span>•</span>
                        <span>{activeSession.workspace_id}</span>
                      </>
                    )}
                  </div>
                </div>
              </div>

              <div className="flex shrink-0 items-center gap-2">
                {transportStrategy && (
                  <span className="hidden items-center gap-1.5 rounded-full border border-[#e5ddcf] bg-white/80 px-2.5 py-1 text-[11px] font-medium text-slate-500 sm:inline-flex">
                    <span>{strategyLabel(copy, transportStrategy)}</span>
                    {transportLatencyMs != null && (
                      <>
                        <span className="text-slate-300">·</span>
                        <span>{transportLatencyMs}ms</span>
                      </>
                    )}
                  </span>
                )}
                <button
                  type="button"
                  onClick={onSignOut}
                  title={copy.signOutAction}
                  className="inline-flex h-9 w-9 items-center justify-center rounded-2xl border border-[#ddd4c5] bg-white text-slate-500 transition-colors hover:bg-[#faf6ef] hover:text-slate-700"
                >
                  <LogOut size={16} />
                </button>
                <ConnectionPill copy={copy} state={connectionState} />
                {activeSession && <StatePill copy={copy} state={activeSession.state} compact />}
              </div>
            </div>
          </header>

          {errorMessage && (
            <div role="alert" className="border-b border-[#f1d2c9] bg-[#fff4f1] px-4 py-3 text-sm text-[#9b3b32] sm:px-6">
              {errorMessage}
            </div>
          )}

          {statusMessage && (
            <div role="status" className="border-b border-[#d9eadf] bg-[#edf7ef] px-4 py-3 text-sm text-[#226140] sm:px-6">
              {statusMessage}
            </div>
          )}

          {children}
        </div>
      </div>
    </RemoteFrame>
  );
}

// ---------------------------------------------------------------------------
// Shared presentational helpers
// ---------------------------------------------------------------------------

function RemoteFrame({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen bg-[radial-gradient(circle_at_top_left,#fbf6ec,transparent_28%),linear-gradient(180deg,#f4efe4_0%,#efe8db_100%)] text-slate-900">
      {children}
    </div>
  );
}

function strategyLabel(copy: RemoteCopy, strategy: string): string {
  switch (strategy) {
    case 'direct_ws': return copy.strategyDirect;
    case 'server_relay': return copy.strategyRelay;
    case 'outbound_polling': return copy.strategyPolling;
    case 'hybrid': return copy.strategyHybrid;
    case 'quic': return copy.strategyQuic;
    default: return strategy;
  }
}

export function EmptyCard({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <div className="max-w-md rounded-[28px] border border-[#e1d7c8] bg-white px-6 py-6 text-center shadow-[0_16px_38px_rgba(34,32,28,0.08)]">
      <div className="text-lg font-semibold text-slate-900">{title}</div>
      <div className="mt-3 text-sm leading-6 text-slate-500">{description}</div>
    </div>
  );
}

function StatePill({
  copy,
  state,
  compact = false,
}: {
  copy: RemoteCopy;
  state: RemoteSessionState;
  compact?: boolean;
}) {
  return (
    <div
      className={cn(
        'rounded-full border px-3 py-1 text-xs font-medium',
        sessionStateClassName(state),
        compact && 'px-2.5 py-1',
      )}
    >
      {copy.sessionStateLabels[state]}
    </div>
  );
}

function ConnectionPill({
  copy,
  state,
}: {
  copy: RemoteCopy;
  state: RemoteConnectionState;
}) {
  return (
    <div
      className={cn(
        'inline-flex items-center gap-2 rounded-full border px-2 py-1.5 text-sm sm:px-3',
        connectionClassName(state),
      )}
      title={connectionLabel(state, copy)}
    >
      {state === 'open' ? <Wifi size={14} /> : state === 'error' ? <X size={14} /> : <WifiOff size={14} />}
      <span className="hidden sm:inline">{connectionLabel(state, copy)}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Style / label helpers
// ---------------------------------------------------------------------------

function sessionStateClassName(state: RemoteSessionState): string {
  switch (state) {
    case 'running':
      return 'border-[#cfe4d7] bg-[#edf7ef] text-[#236342]';
    case 'waiting_approval':
      return 'border-[#ead9b7] bg-[#fbf3df] text-[#7c5d12]';
    case 'completed':
      return 'border-[#d9e7ef] bg-[#eef7fb] text-[#265f7a]';
    case 'failed':
      return 'border-[#f0d2ce] bg-[#fff3f1] text-[#9b3b32]';
    case 'cancelled':
      return 'border-[#e5ddd4] bg-[#f6f1eb] text-slate-600';
    default:
      return 'border-[#e5ddd4] bg-[#f6f1eb] text-slate-600';
  }
}

function connectionClassName(state: RemoteConnectionState): string {
  switch (state) {
    case 'open':
      return 'border-[#cfe4d7] bg-[#edf7ef] text-[#236342]';
    case 'error':
      return 'border-[#f0d2ce] bg-[#fff3f1] text-[#9b3b32]';
    case 'connecting':
    case 'reconnecting':
      return 'border-[#ead9b7] bg-[#fbf3df] text-[#7c5d12]';
    default:
      return 'border-[#e5ddd4] bg-[#f6f1eb] text-slate-600';
  }
}

function connectionLabel(state: RemoteConnectionState, copy: RemoteCopy): string {
  return copy.connectionLabels[state];
}

function formatRelativeTime(
  iso: string,
  locale: ReturnType<typeof resolveRemoteLocale>,
  copy: RemoteCopy,
): string {
  return formatRemoteRelativeTime(iso, locale, copy);
}