/**
 * MobileRemoteApp — 移动端专用远程控制 UI。
 *
 * 与桌面端 RemoteApp 的核心区别：
 * - 底部 Tab 导航（会话 / 时间线 / 审批）替代侧边栏 + 侧面板
 * - 全屏视图，大触摸目标（最小 44px）
 * - 简化认证界面（单列、可折叠次要选项）
 * - 浮动 prompt 输入栏（类似聊天 app）
 *
 * 职责：发送提示词、审批/拒绝、查看时间线 —— 不执行任何本地代码。
 */

import {
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  FileOutput,
  List,
  LoaderCircle,
  MessageSquare,
  Shield,
  Square,
  Wifi,
  WifiOff,
} from 'lucide-react';
import {
  startTransition,
  useDeferredValue,
  useCallback,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from 'react';
import { Virtuoso } from 'react-virtuoso';
import {
  clearRemoteActiveSessionId,
  clearRemoteAccessToken,
  clearRemotePairingContext,
  deriveUserKey,
  hydrateRemoteAuthTokensFromSecureStore,
  persistRemoteAccessToken,
  persistRemoteActiveSessionId,
  persistRemoteRefreshToken,
  resolveRemoteActiveSessionId,
  resolveRemoteAccessToken,
  resolveRemoteBaseUrl,
  resolveRemotePairingContext,
  stripRemoteSensitiveQueryParams,
} from '../lib/runtime';
import { downloadRemoteArtifact } from '../lib/fileDownload';
import { shareFile } from '../lib/mobile/fileDownload';
import { getBiometricEnabled, setBiometricEnabled } from '../lib/mobile/biometric';
import {
  initPushNotifications,
  registerPushTokenWithControlPlane,
  showLocalNotification,
} from '../lib/mobile/pushNotifications';
import { initDeepLinks, parsePairingUrl } from '../lib/mobile/deepLink';
import { initAppLifecycle } from '../lib/mobile/appLifecycle';
import { ApprovalPanel } from '../components/shared/ApprovalPanel';
import { ArtifactPanel } from '../components/shared/ArtifactPanel';
import { formatBytes } from '../components/shared/formatBytes';
import {
  acceptPairingOffer,
  bootstrapControlPlane,
  buildArtifactDownloadUrl,
  getControlPlaneHealth,
  interruptSession,
  listSessionApprovals,
  listSessionArtifacts,
  listSessions,
  respondToApproval,
  sendPrompt,
} from './api';
import {
  formatRemoteRelativeTime,
  getRemoteCopy,
  resolveRemoteLocale,
  type RemoteConnectionState,
} from './i18n';
import { appendRemoteTimelineEvent, resolveRemoteSessionTitle } from '../session/normalize/fromRemote';
import { loadRemoteSessionBundle } from './transport';
import { isDirectRunnerEnabled, resolveRemoteRunnerBaseUrl, resolveRemoteTransportStrategy } from './transportMode';
import { useConnection } from './useConnection';
import { useRemoteSessionController } from './useRemoteSessionController';
import { extractErrorMessage } from './utils';
import { TimelineCard, describeSessionControl } from './RemoteTimelineShared';
import { destroyConnectionManager } from './connection-manager';
import type { TransportConfig } from './connection-manager';
import type {
  RemoteApprovalDecision,
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteControlPlaneHealth,
  RemoteSessionRecord,
  RemoteTimelineEvent,
} from './types';

type MobileTab = 'sessions' | 'timeline' | 'approvals';

const APPROVAL_DECISIONS: Array<{
  decision: RemoteApprovalDecision;
  className: string;
}> = [
  { decision: 'approved', className: 'bg-[#1d6b45] text-white active:bg-[#145033]' },
  { decision: 'denied', className: 'bg-[#a13a30] text-white active:bg-[#7e2b24]' },
  { decision: 'cancelled', className: 'bg-rc-bg-active text-rc-text-secondary active:bg-rc-bg-active' },
];

// ═══════════════════════════════════════════════════════════════════════════
// MobileRemoteApp
// ═══════════════════════════════════════════════════════════════════════════

export default function MobileRemoteApp() {
  const {
    baseUrl,
    locale,
    copy,
    health,
    authLoading,
    authErrorMessage,
    manualAccessToken,
    signInUsername,
    signInPassword,
    deviceName,
    pairingOfferId,
    pairingSecret,
    bootstrapSecret,
    setBootstrapSecret,
    setDeviceName,
    setManualAccessToken,
    setPairingOfferId,
    setPairingSecret,
    setSignInUsername,
    setSignInPassword,
    sessions,
    sessionsLoading,
    activeSession,
    setActiveSessionId,
    selectedSessionId,
    deferredEvents,
    eventsLoading,
    pendingApprovals,
    artifacts,
    composer,
    setComposer,
    sending,
    interrupting,
    approvingId,
    downloadingArtifactId,
    errorMessage,
    statusMessage,
    showAuthGate,
    activeSessionControlStatus,
    connectionState,
    refreshSessions,
    handleBootstrapClaim,
    handlePairingAccept,
    handleManualTokenSave,
    handleClearSavedToken,
    handleUserSignIn,
    handleSendPrompt,
    handleInterrupt,
    handleApprovalDecision,
    handleArtifactDownload,
    handleArtifactShare,
  } = useRemoteSessionController({ defaultDeviceName: 'Mobile' });

  // Destroy the ConnectionManager singleton on unmount so transports and
  // listeners are cleaned up when the remote app is navigated away from.
  useEffect(() => {
    return () => destroyConnectionManager();
  }, []);

  const [activeTab, setActiveTab] = useState<MobileTab>('sessions');
  const [bioEnabled, setBioEnabled] = useState(false);

  // Load biometric enabled state on mount.
  useEffect(() => {
    void getBiometricEnabled().then(setBioEnabled);
  }, []);

  const handleToggleBiometric = useCallback(async () => {
    const next = !bioEnabled;
    const ok = await setBiometricEnabled(next);
    if (ok) setBioEnabled(next);
  }, [bioEnabled]);
  const approvalActions = useMemo(
    () => APPROVAL_DECISIONS.map((item) => ({ ...item, label: copy.approvalDecisionLabels[item.decision] })),
    [copy],
  );
  const handleSelectSession = useCallback((id: string) => {
    setActiveSessionId(id);
    setActiveTab('timeline');
  }, [setActiveSessionId]);
  const hasPending = pendingApprovals.length > 0;

  // ═══════════════════════════════════════════════════════════════════════
  // Render
  // ═══════════════════════════════════════════════════════════════════════
  // Early exits
  if (!baseUrl) {
    return (
      <div className="flex h-dvh items-center justify-center bg-rc-bg-base px-6">
        <div className="max-w-sm rounded-3xl border border-rc-border-primary bg-rc-bg-surface p-8 text-center shadow-lg">
          <div className="text-lg font-semibold text-rc-text-primary">{copy.remoteModeNotConfiguredTitle}</div>
          <div className="mt-3 text-sm leading-6 text-rc-text-tertiary">{copy.remoteModeNotConfiguredDescription}</div>
        </div>
      </div>
    );
  }

  if (!health) {
    return (
      <div className="flex h-dvh items-center justify-center bg-rc-bg-base">
        <div className="flex items-center gap-3 rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-5 py-4 text-sm text-rc-text-secondary shadow-lg">
          <LoaderCircle size={16} className="animate-spin" />
          {copy.contactingControlPlane}
        </div>
      </div>
    );
  }

  if (showAuthGate) {
    return (
      <MobileAuthScreen
        copy={copy}
        authErrorMessage={authErrorMessage}
        authLoading={authLoading}
        bootstrapEnabled={!health.owner_claimed && health.bootstrap_secret_configured}
        health={health}
        deviceName={deviceName}
        manualAccessToken={manualAccessToken}
        bootstrapSecret={bootstrapSecret}
        username={signInUsername}
        password={signInPassword}
        pairingOfferId={pairingOfferId}
        pairingSecret={pairingSecret}
        onBootstrapClaim={() => { void handleBootstrapClaim(); }}
        onClearSavedToken={handleClearSavedToken}
        onManualTokenSave={handleManualTokenSave}
        onPairingAccept={() => { void handlePairingAccept(); }}
        onUserSignIn={() => { void handleUserSignIn(); }}
        setBootstrapSecret={setBootstrapSecret}
        setDeviceName={setDeviceName}
        setManualAccessToken={setManualAccessToken}
        setPairingOfferId={setPairingOfferId}
        setPairingSecret={setPairingSecret}
        setUsername={setSignInUsername}
        setPassword={setSignInPassword}
      />
    );
  }

  // ── Main mobile shell ─────────────────────────────────────────────────
  return (
    <div className="flex h-dvh flex-col bg-rc-bg-base text-rc-text-primary">
      {/* ── Status / error bars ── */}
      {errorMessage && (
        <div role="alert" className="shrink-0 border-b border-[#f1d2c9] bg-[#fff4f1] px-4 py-2 text-sm text-[#9b3b32]">
          {errorMessage}
        </div>
      )}
      {statusMessage && (
        <div role="status" className="shrink-0 border-b border-[#d9eadf] bg-[#edf7ef] px-4 py-2 text-sm text-[#226140]">
          {statusMessage}
        </div>
      )}

      {/* ── Compact header ── */}
      <header className="flex shrink-0 items-center justify-between border-b border-rc-border-primary bg-rc-bg-surface/90 px-4 py-3 backdrop-blur">
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold">
            {activeSession ? resolveRemoteSessionTitle(activeSession) : copy.selectRemoteSession}
          </div>
          <div className="mt-0.5 flex items-center gap-2 text-xs text-rc-text-tertiary">
            <ConnectionPill state={connectionState} copy={copy} />
            {activeSession && <StatePill state={activeSession.state} copy={copy} />}
          </div>
        </div>
        <div className="ml-3 flex shrink-0 items-center gap-2">
          <button
            type="button"
            onClick={handleToggleBiometric}
            className={`rounded-xl border px-3 py-2 text-xs font-medium active:bg-rc-bg-surface ${
              bioEnabled
                ? 'border-emerald-600/40 bg-emerald-500/10 text-emerald-400'
                : 'border-rc-border-primary bg-rc-bg-hover text-rc-text-secondary'
            }`}
            title={bioEnabled ? '禁用生物识别' : '启用生物识别'}
          >
            {bioEnabled ? '🔒' : '🔓'}
          </button>
          <button
            type="button"
            onClick={handleClearSavedToken}
            className="shrink-0 rounded-xl border border-rc-border-primary bg-rc-bg-hover px-3 py-2 text-xs font-medium text-rc-text-secondary active:bg-rc-bg-surface"
          >
            {copy.signOutAction}
          </button>
        </div>
      </header>

      {/* ── Tab content (full screen) ── */}
      <div className="min-h-0 flex-1 overflow-hidden">
        {activeTab === 'sessions' && (
          <MobileSessionsTab
            sessions={sessions}
            sessionsLoading={sessionsLoading}
            activeSessionId={selectedSessionId}
            copy={copy}
            locale={locale}
            onSelectSession={handleSelectSession}
            onRefresh={() => { void refreshSessions(); }}
          />
        )}
        {activeTab === 'timeline' && (
          <MobileTimelineTab
            activeSession={activeSession}
            events={deferredEvents}
            eventsLoading={eventsLoading}
            composer={composer}
            sending={sending}
            interrupting={interrupting}
            controlStatus={activeSessionControlStatus}
            copy={copy}
            locale={locale}
            onComposerChange={setComposer}
            onSend={() => { void handleSendPrompt(); }}
            onInterrupt={() => { void handleInterrupt(); }}
          />
        )}
        {activeTab === 'approvals' && (
          <MobileApprovalsTab
            pendingApprovals={pendingApprovals}
            approvalActions={approvalActions}
            approvingId={approvingId}
            artifacts={artifacts}
            downloadingArtifactId={downloadingArtifactId}
            copy={copy}
            onApprovalDecision={(id, decision) => { void handleApprovalDecision(id, decision); }}
            onDownload={(a) => {
              const record = artifacts.find((item) => item.artifact_id === a.artifact_id);
              if (record) void handleArtifactDownload(record);
            }}
            onShare={(a) => {
              const record = artifacts.find((item) => item.artifact_id === a.artifact_id);
              if (record) void handleArtifactShare(record);
            }}
          />
        )}
      </div>

      {/* ── Bottom tab bar ── */}
      <nav role="tablist" className="flex shrink-0 border-t border-rc-border-primary bg-rc-bg-surface/95 backdrop-blur pb-[env(safe-area-inset-bottom)]">
        <TabButton
          active={activeTab === 'sessions'}
          icon={<List size={20} />}
          label={copy.mobileTabSessions}
          onClick={() => setActiveTab('sessions')}
        />
        <TabButton
          active={activeTab === 'timeline'}
          icon={<MessageSquare size={20} />}
          label={copy.mobileTabTimeline}
          onClick={() => setActiveTab('timeline')}
        />
        <TabButton
          active={activeTab === 'approvals'}
          icon={<Shield size={20} />}
          label={copy.mobileTabApprovals}
          badge={hasPending ? pendingApprovals.length : undefined}
          onClick={() => setActiveTab('approvals')}
        />
      </nav>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Mobile Auth Screen
// ═══════════════════════════════════════════════════════════════════════════

function MobileAuthScreen({
  copy,
  authErrorMessage,
  authLoading,
  bootstrapEnabled,
  health,
  deviceName,
  manualAccessToken,
  bootstrapSecret,
  username,
  password,
  pairingOfferId,
  pairingSecret,
  onBootstrapClaim,
  onClearSavedToken,
  onManualTokenSave,
  onPairingAccept,
  onUserSignIn,
  setBootstrapSecret,
  setDeviceName,
  setManualAccessToken,
  setPairingOfferId,
  setPairingSecret,
  setUsername,
  setPassword,
}: {
  copy: ReturnType<typeof getRemoteCopy>;
  authErrorMessage: string | null;
  authLoading: boolean;
  bootstrapEnabled: boolean;
  health: RemoteControlPlaneHealth;
  deviceName: string;
  manualAccessToken: string;
  bootstrapSecret: string;
  username: string;
  password: string;
  pairingOfferId: string;
  pairingSecret: string;
  onBootstrapClaim: () => void;
  onClearSavedToken: () => void;
  onManualTokenSave: () => void;
  onPairingAccept: () => void;
  onUserSignIn: () => void;
  setBootstrapSecret: (v: string) => void;
  setDeviceName: (v: string) => void;
  setManualAccessToken: (v: string) => void;
  setPairingOfferId: (v: string) => void;
  setPairingSecret: (v: string) => void;
  setUsername: (v: string) => void;
  setPassword: (v: string) => void;
}) {
  const [showOptions, setShowOptions] = useState(false);

  return (
    <div className="flex h-dvh flex-col overflow-y-auto bg-rc-bg-base px-5 py-8">
      {/* Header */}
      <div className="mb-8 text-center">
        <div className="text-[11px] font-semibold uppercase tracking-[0.32em] text-rc-text-tertiary">
          {copy.authGateEyebrow}
        </div>
        <div className="mt-3 text-2xl font-bold text-rc-text-primary">{copy.authGateTitle}</div>
        <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.mobileAuthSubtitle}</div>
      </div>

      {/* Error */}
      {authErrorMessage && (
        <div role="alert" className="mb-4 flex items-start gap-3 rounded-2xl border border-[#f0d3c8] bg-[#fff2ed] px-4 py-3 text-sm text-[#8d3f30]">
          <AlertTriangle size={16} className="mt-0.5 shrink-0" />
          <div>{authErrorMessage}</div>
        </div>
      )}

      {/* Device name */}
      <label className="mb-4 block">
        <div className="mb-2 text-xs font-semibold uppercase tracking-[0.2em] text-rc-text-tertiary">
          {copy.deviceNameLabel}
        </div>
        <input
          value={deviceName}
          onChange={(e) => setDeviceName(e.target.value)}
          placeholder={copy.deviceNamePlaceholder}
          className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-[#a58a5e]"
        />
      </label>

      {/* Sign in (primary) */}
      <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-surface p-5">
        <div className="text-sm font-semibold text-rc-text-primary">{copy.multiUserTitle}</div>
        <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.multiUserDescription}</div>
        <div className="mt-4 grid gap-3">
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder={copy.usernamePlaceholder}
            autoComplete="username"
            className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-[#a58a5e]"
          />
          <input
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            type="password"
            placeholder={copy.passwordPlaceholder}
            autoComplete="current-password"
            className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-[#a58a5e]"
          />
        </div>
        <button
          type="button"
          onClick={onUserSignIn}
          disabled={authLoading || !username.trim() || !password.trim()}
          className="mt-4 flex h-12 w-full items-center justify-center rounded-2xl bg-[#1d6b45] text-sm font-semibold text-white active:bg-[#145033] disabled:opacity-50"
        >
          {authLoading ? <LoaderCircle size={18} className="animate-spin" /> : copy.signInAction}
        </button>
      </div>

      {/* Expandable other options */}
      <button
        type="button"
        onClick={() => setShowOptions(!showOptions)}
        className="mt-5 flex items-center justify-center gap-2 text-sm font-medium text-rc-text-tertiary active:text-rc-text-secondary"
      >
        {showOptions ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
        {showOptions ? copy.mobileCollapseOptions : copy.mobileExpandOptions}
      </button>

      {showOptions && (
        <div className="mt-3 space-y-4">
          {bootstrapEnabled && (
            <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-surface p-5">
              <div className="text-sm font-semibold text-rc-text-primary">{copy.bootstrapTitle}</div>
              <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.bootstrapDescription}</div>
              <input
                type="password"
                value={bootstrapSecret}
                onChange={(e) => setBootstrapSecret(e.target.value)}
                placeholder={copy.bootstrapSecretLabel}
                className="mt-3 w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-[#a58a5e]"
              />
              <button
                type="button"
                onClick={onBootstrapClaim}
                disabled={authLoading}
                className="mt-3 flex h-12 w-full items-center justify-center rounded-2xl bg-[#1d6b45] text-sm font-semibold text-white active:bg-[#145033] disabled:opacity-50"
              >
                {authLoading ? <LoaderCircle size={18} className="animate-spin" /> : copy.claimOwnerDevice}
              </button>
            </div>
          )}

          <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-surface p-5">
            <div className="text-sm font-semibold text-rc-text-primary">{copy.acceptPairingTitle}</div>
            <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.acceptPairingDescription}</div>
            <div className="mt-3 grid gap-3">
              <input
                value={pairingOfferId}
                onChange={(e) => setPairingOfferId(e.target.value)}
                placeholder={copy.offerIdPlaceholder}
                className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-[#a58a5e]"
              />
              <input
                value={pairingSecret}
                onChange={(e) => setPairingSecret(e.target.value)}
                type="password"
                placeholder={copy.pairingSecretPlaceholder}
                className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-[#a58a5e]"
              />
            </div>
            <button
              type="button"
              onClick={onPairingAccept}
              disabled={authLoading}
              className="mt-3 flex h-12 w-full items-center justify-center rounded-2xl bg-[#174e8c] text-sm font-semibold text-white active:bg-[#123b6b] disabled:opacity-50"
            >
              {authLoading ? <LoaderCircle size={18} className="animate-spin" /> : copy.acceptPairingAction}
            </button>
          </div>

          <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-surface p-5">
            <div className="text-sm font-semibold text-rc-text-primary">{copy.existingTokenTitle}</div>
            <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.existingTokenDescription}</div>
            <textarea
              value={manualAccessToken}
              onChange={(e) => setManualAccessToken(e.target.value)}
              rows={2}
              placeholder="rcdt_..."
              className="mt-3 w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-[#a58a5e]"
            />
            <div className="mt-3 flex gap-3">
              <button
                type="button"
                onClick={onManualTokenSave}
                className="flex-1 rounded-2xl border border-rc-border-primary bg-rc-bg-surface py-3 text-sm font-medium text-rc-text-secondary active:bg-rc-bg-hover"
              >
                {copy.saveToken}
              </button>
              <button
                type="button"
                onClick={onClearSavedToken}
                className="flex-1 rounded-2xl border border-rc-border-primary py-3 text-sm font-medium text-rc-text-tertiary active:bg-rc-bg-active"
              >
                {copy.clearSavedToken}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Server info */}
      <div className="mt-6 rounded-2xl border border-dashed border-rc-border-primary bg-rc-bg-surface/60 px-4 py-3 text-xs leading-5 text-rc-text-tertiary">
        {health.service} · {copy.availableRunnersLabel}: {health.available_runner_count} · {copy.activeSessionsLabel}: {health.session_count}
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tab: Sessions
// ═══════════════════════════════════════════════════════════════════════════

function MobileSessionsTab({
  sessions,
  sessionsLoading,
  activeSessionId,
  copy,
  locale,
  onSelectSession,
  onRefresh,
}: {
  sessions: RemoteSessionRecord[];
  sessionsLoading: boolean;
  activeSessionId: string | null;
  copy: ReturnType<typeof getRemoteCopy>;
  locale: ReturnType<typeof resolveRemoteLocale>;
  onSelectSession: (id: string) => void;
  onRefresh: () => void;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center justify-between border-b border-rc-border-primary px-4 py-3">
        <span className="text-sm font-semibold text-rc-text-primary">{copy.refreshSessions}</span>
        <button
          type="button"
          onClick={onRefresh}
          className="rounded-xl border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-xs font-medium text-rc-text-secondary active:bg-rc-bg-hover"
        >
          {copy.refreshSessions}
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {sessionsLoading ? (
          <div className="flex items-center justify-center py-12">
            <LoaderCircle size={20} className="animate-spin text-rc-text-tertiary" />
          </div>
        ) : sessions.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <div className="text-lg font-semibold text-rc-text-primary">{copy.noSessionsTitle}</div>
            <div className="mt-3 text-sm leading-6 text-rc-text-tertiary">{copy.noSessionsDescription}</div>
          </div>
        ) : (
          <div className="space-y-2">
            {sessions.map((session) => {
              const selected = session.session_id === activeSessionId;
              return (
                <button
                  key={session.session_id}
                  type="button"
                  onClick={() => onSelectSession(session.session_id)}
                  className={`w-full rounded-2xl border px-4 py-4 text-left transition-colors active:scale-[0.98] ${
                    selected
                      ? 'border-[#b8cbbf] bg-[#edf7ef] shadow-sm'
                      : 'border-rc-border-primary bg-rc-bg-surface active:bg-rc-bg-hover'
                  }`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 text-sm font-semibold text-rc-text-primary truncate">
                      {resolveRemoteSessionTitle(session)}
                    </div>
                    <StatePill state={session.state} copy={copy} />
                  </div>
                  <div className="mt-2 flex items-center gap-2 text-xs text-rc-text-tertiary">
                    <span>{formatRemoteRelativeTime(session.updated_at, locale, copy)}</span>
                    {session.metadata.agent_type && (
                      <>
                        <span>·</span>
                        <span className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[10px] uppercase text-rc-text-secondary">
                          {session.metadata.agent_type}
                        </span>
                      </>
                    )}
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tab: Timeline + floating prompt bar
// ═══════════════════════════════════════════════════════════════════════════

function MobileTimelineTab({
  activeSession,
  events,
  eventsLoading,
  composer,
  sending,
  interrupting,
  controlStatus,
  copy,
  locale,
  onComposerChange,
  onSend,
  onInterrupt,
}: {
  activeSession: RemoteSessionRecord | null;
  events: RemoteTimelineEvent[];
  eventsLoading: boolean;
  composer: string;
  sending: boolean;
  interrupting: boolean;
  controlStatus: ReturnType<typeof describeSessionControl>;
  copy: ReturnType<typeof getRemoteCopy>;
  locale: ReturnType<typeof resolveRemoteLocale>;
  onComposerChange: (v: string) => void;
  onSend: () => void;
  onInterrupt: () => void;
}) {
  if (!activeSession) {
    return (
      <div className="flex h-full items-center justify-center px-6 text-center">
        <div>
          <div className="text-lg font-semibold text-rc-text-primary">{copy.pickSessionTitle}</div>
          <div className="mt-3 text-sm leading-6 text-rc-text-tertiary">{copy.pickSessionDescription}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Timeline */}
      <div className="flex-1 min-h-0 bg-rc-bg-secondary">
        {eventsLoading ? (
          <div className="flex items-center justify-center py-16">
            <LoaderCircle size={20} className="animate-spin text-rc-text-tertiary" />
          </div>
        ) : events.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center px-6">
            <div className="text-lg font-semibold text-rc-text-primary">{copy.timelineEmptyTitle}</div>
            <div className="mt-3 text-sm leading-6 text-rc-text-tertiary">{copy.timelineEmptyDescription}</div>
          </div>
        ) : (
          <Virtuoso
            data={events}
            followOutput="smooth"
            className="h-full"
            itemContent={(_index, event) => (
              <div className="px-4 py-2">
                <TimelineCard copy={copy} locale={locale} event={event} />
              </div>
            )}
          />
        )}
      </div>

      {/* Floating prompt bar */}
      <div className="shrink-0 border-t border-rc-border-primary bg-rc-bg-secondary px-3 py-3 pb-[calc(0.75rem+env(safe-area-inset-bottom))]">
        {controlStatus.notice && (
          <div className="mb-2 rounded-xl bg-[#fff7e8] px-3 py-2 text-xs leading-5 text-[#845612]">
            {controlStatus.notice}
          </div>
        )}
        <div className="flex items-end gap-2">
          <textarea
            aria-label={copy.followUpPlaceholder}
            value={composer}
            onChange={(e) => {
              onComposerChange(e.target.value);
              // Auto-grow: reset height then expand to scroll height, clamped to max.
              e.target.style.height = 'auto';
              e.target.style.height = `${Math.min(e.target.scrollHeight, 120)}px`;
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); onSend(); }
            }}
            rows={1}
            disabled={!controlStatus.canSendPrompt}
            placeholder={copy.followUpPlaceholder}
            className="min-h-[44px] max-h-[120px] flex-1 resize-none rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none placeholder:text-rc-text-tertiary focus:border-[#a58a5e] disabled:opacity-50"
          />
          {controlStatus.canInterrupt && (
            <button
              type="button"
              onClick={onInterrupt}
              disabled={interrupting}
              className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-rc-border-primary bg-rc-bg-hover text-rc-text-secondary active:bg-[#f3ebdf] disabled:opacity-50"
            >
              {interrupting ? <LoaderCircle size={16} className="animate-spin" /> : <Square size={14} />}
            </button>
          )}
          <button
            type="button"
            onClick={onSend}
            disabled={sending || composer.trim().length === 0 || !controlStatus.canSendPrompt}
            className="flex h-11 shrink-0 items-center justify-center rounded-2xl bg-[#17181a] px-4 text-sm font-medium text-white shadow-md active:bg-[#282a2d] disabled:bg-[#cbbfac] disabled:text-white/70"
          >
            {sending ? <LoaderCircle size={16} className="animate-spin" /> : copy.send}
          </button>
        </div>
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tab: Approvals + Artifacts
// ═══════════════════════════════════════════════════════════════════════════

function MobileApprovalsTab({
  pendingApprovals,
  approvalActions,
  approvingId,
  artifacts,
  downloadingArtifactId,
  copy,
  onApprovalDecision,
  onDownload,
  onShare,
}: {
  pendingApprovals: RemoteApprovalRecord[];
  approvalActions: Array<{ decision: RemoteApprovalDecision; label: string; className: string }>;
  approvingId: string | null;
  artifacts: RemoteArtifactRecord[];
  downloadingArtifactId: string | null;
  copy: ReturnType<typeof getRemoteCopy>;
  onApprovalDecision: (id: string, decision: RemoteApprovalDecision) => void;
  onDownload: (artifact: RemoteArtifactRecord) => void;
  onShare: (artifact: RemoteArtifactRecord) => void;
}) {
  return (
    <div className="h-full overflow-y-auto px-4 py-4">
      {/* Approvals section */}
      <div className="mb-6">
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
          <Shield size={16} />
          {copy.pendingApprovals}
          {pendingApprovals.length > 0 && (
            <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-[#fbf3df] px-1.5 text-[11px] font-bold text-[#7c5d12]">
              {pendingApprovals.length}
            </span>
          )}
        </div>

        {pendingApprovals.length === 0 ? (
          <div className="mt-4 rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-6 text-center text-sm text-rc-text-tertiary">
            {copy.noPendingApprovals}
          </div>
        ) : (
          <div className="mt-3 space-y-3">
            {pendingApprovals.map((approval) => (
              <div key={approval.approval_id} className="rounded-2xl border border-[#ead9b7] bg-rc-bg-surface p-4">
                <div className="text-sm font-semibold text-rc-text-primary">{approval.title}</div>
                {approval.description && (
                  <div className="mt-1 text-xs leading-5 text-rc-text-tertiary">{approval.description}</div>
                )}
                <div className="mt-3 flex gap-2">
                  {approvalActions.map((action) => (
                    <button
                      key={action.decision}
                      type="button"
                      onClick={() => onApprovalDecision(approval.approval_id, action.decision)}
                      disabled={approvingId !== null}
                      className={`flex-1 rounded-xl py-3 text-sm font-semibold disabled:opacity-50 ${action.className}`}
                    >
                      {approvingId === approval.approval_id ? (
                        <LoaderCircle size={14} className="mx-auto animate-spin" />
                      ) : action.label}
                    </button>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Artifacts section */}
      <div>
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
          <FileOutput size={16} />
          {copy.artifacts}
        </div>

        {artifacts.length === 0 ? (
          <div className="mt-4 rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-6 text-center text-sm text-rc-text-tertiary">
            {copy.noArtifacts}
          </div>
        ) : (
          <div className="mt-3 space-y-2">
            {artifacts.map((artifact) => (
              <div key={artifact.artifact_id} className="flex items-center justify-between rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-rc-text-primary">{artifact.file_name}</div>
                  <div className="mt-0.5 text-xs text-rc-text-tertiary">{formatBytes(artifact.size_bytes)}</div>
                </div>
                <div className="flex shrink-0 gap-2">
                  <button
                    type="button"
                    onClick={() => onDownload(artifact)}
                    disabled={downloadingArtifactId === artifact.artifact_id}
                    className="rounded-xl border border-rc-border-primary bg-rc-bg-hover px-3 py-2 text-xs font-medium text-rc-text-secondary active:bg-rc-bg-surface disabled:opacity-50"
                  >
                    {downloadingArtifactId === artifact.artifact_id ? <LoaderCircle size={12} className="animate-spin" /> : copy.renderResponse}
                  </button>
                  <button
                    type="button"
                    onClick={() => onShare(artifact)}
                    className="rounded-xl border border-rc-border-primary bg-rc-bg-hover px-3 py-2 text-xs font-medium text-rc-text-secondary active:bg-rc-bg-surface"
                  >
                    {copy.shareArtifact}
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Shared UI primitives
// ═══════════════════════════════════════════════════════════════════════════

function TabButton({
  active,
  icon,
  label,
  badge,
  onClick,
}: {
  active: boolean;
  icon: React.ReactNode;
  label: string;
  badge?: number;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={`relative flex flex-1 flex-col items-center gap-1 py-2 text-[11px] font-medium transition-colors ${
        active ? 'text-[#1d6b45]' : 'text-rc-text-tertiary active:text-rc-text-secondary'
      }`}
    >
      {icon}
      <span>{label}</span>
      {badge != null && badge > 0 && (
        <span className="absolute right-1/4 top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-red-500 px-1 text-[10px] font-bold text-white">
          {badge}
        </span>
      )}
    </button>
  );
}

function ConnectionPill({ state, copy }: { state: RemoteConnectionState; copy: ReturnType<typeof getRemoteCopy> }) {
  const className = state === 'open'
    ? 'text-[#236342]'
    : state === 'error'
    ? 'text-[#9b3b32]'
    : 'text-[#7c5d12]';

  return (
    <span className={`inline-flex items-center gap-1 text-xs font-medium ${className}`}>
      {state === 'open' ? <Wifi size={12} /> : state === 'error' ? <AlertTriangle size={12} /> : <WifiOff size={12} />}
      {copy.connectionLabels[state]}
    </span>
  );
}

function StatePill({ state, copy }: { state: import('./types').RemoteSessionState; copy: ReturnType<typeof getRemoteCopy> }) {
  const className = state === 'running'
    ? 'bg-[#edf7ef] text-[#236342]'
    : state === 'waiting_approval'
    ? 'bg-[#fbf3df] text-[#7c5d12]'
    : state === 'failed'
    ? 'bg-[#fff3f1] text-[#9b3b32]'
    : 'bg-[#f6f1eb] text-rc-text-secondary';

  return (
    <span className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium ${className}`}>
      {copy.sessionStateLabels[state]}
    </span>
  );
}
