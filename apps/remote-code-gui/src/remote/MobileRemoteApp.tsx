/**
 * MobileRemoteApp — 移动端专用远程控制 UI。
 *
 * 与桌面端 RemoteApp 的核心区别：
 * - 底部 Tab 导航（会话 / 时间线 / 审批 / 连接 / 设置）替代侧边栏 + 侧面板
 * - 全屏视图，大触摸目标（最小 44px）
 * - 简化认证界面（单列、可折叠次要选项）
 * - 浮动 prompt 输入栏（类似聊天 app）
 *
 * 职责：发送提示词、审批/拒绝、查看时间线 —— 不执行任何本地代码。
 */

import {
  AlertTriangle,
  Bell,
  ChevronDown,
  ChevronUp,
  FileOutput,
  Fingerprint,
  Globe,
  Info,
  List,
  LoaderCircle,
  MessageSquare,
  Monitor,
  Moon,
  Settings,
  Shield,
  Square,
  Sun,
  Vibrate,
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
  clearRemoteBaseUrl,
  clearRemotePairingContext,
  deriveUserKey,
  hydrateRemoteAuthTokensFromSecureStore,
  persistRemoteAccessToken,
  persistRemoteActiveSessionId,
  persistRemoteBaseUrl,
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
import { hapticSuccess, hapticWarning, hapticError } from '../lib/mobile/haptics';
import { getNetworkStatus, onNetworkChange, describeConnectionType, initNetworkMonitoring } from '../lib/mobile/network';
import { secureStoreGet, secureStoreSet } from '../lib/mobile/secureStorage';
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
import { useTheme } from '../components/design/ThemeProvider';
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

type MobileTab = 'sessions' | 'timeline' | 'approvals' | 'connect' | 'settings';

const APPROVAL_DECISIONS: Array<{
  decision: RemoteApprovalDecision;
  className: string;
}> = [
  { decision: 'approved', className: 'bg-rc-accent-success text-white active:opacity-80' },
  { decision: 'denied', className: 'bg-rc-accent-error text-white active:opacity-80' },
  { decision: 'cancelled', className: 'bg-rc-bg-active text-rc-text-secondary active:opacity-80' },
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
    transportStrategy,
    transportMetrics,
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

  const { mode: themeMode, setMode: setThemeMode, isDark } = useTheme();

  // Destroy the ConnectionManager singleton on unmount so transports and
  // listeners are cleaned up when the remote app is navigated away from.
  useEffect(() => {
    return () => destroyConnectionManager();
  }, []);

  const [activeTab, setActiveTab] = useState<MobileTab>('sessions');
  const [bioEnabled, setBioEnabled] = useState(false);
  const [hapticEnabled, setHapticEnabledState] = useState(true);
  const [networkOnline, setNetworkOnline] = useState(true);
  const [networkType, setNetworkType] = useState('unknown');

  // Load biometric + haptic state on mount.
  useEffect(() => {
    void getBiometricEnabled().then(setBioEnabled);
    void secureStoreGet('haptic_enabled').then((v) => setHapticEnabledState(v !== 'false'));
  }, []);

  // Network status monitoring.
  useEffect(() => {
    initNetworkMonitoring();
    const s = getNetworkStatus();
    setNetworkOnline(s.connected);
    setNetworkType(s.connectionType);
    const unsub = onNetworkChange((connected, connectionType) => {
      setNetworkOnline(connected);
      setNetworkType(connectionType);
    });
    return unsub;
  }, []);

  // Haptic guard helper.
  const haptic = useCallback((type: 'success' | 'warning' | 'error') => {
    if (!hapticEnabled) return;
    if (type === 'success') hapticSuccess();
    else if (type === 'warning') hapticWarning();
    else hapticError();
  }, [hapticEnabled]);

  const handleTabSwitch = useCallback((tab: MobileTab) => {
    setActiveTab(tab);
    haptic('success');
  }, [haptic]);

  const handleToggleBiometric = useCallback(async () => {
    const next = !bioEnabled;
    const ok = await setBiometricEnabled(next);
    if (ok) {
      setBioEnabled(next);
      haptic(next ? 'success' : 'warning');
    }
  }, [bioEnabled, haptic]);

  const handleToggleHaptic = useCallback(async () => {
    const next = !hapticEnabled;
    await secureStoreSet('haptic_enabled', String(next));
    setHapticEnabledState(next);
    if (next) hapticSuccess();
  }, [hapticEnabled]);

  const approvalActions = useMemo(
    () => APPROVAL_DECISIONS.map((item) => ({ ...item, label: copy.approvalDecisionLabels[item.decision] })),
    [copy],
  );
  const handleSelectSession = useCallback((id: string) => {
    setActiveSessionId(id);
    setActiveTab('timeline');
  }, [setActiveSessionId]);
  const hasPending = pendingApprovals.length > 0;
  const connected = Boolean(baseUrl && health);

  // ═══════════════════════════════════════════════════════════════════════
  // Render
  // ═══════════════════════════════════════════════════════════════════════
  // Auth gate — when connected but not yet authenticated
  if (connected && health && showAuthGate) {
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
        <div role="alert" className="shrink-0 border-b border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-2 text-sm text-rc-accent-error">
          {errorMessage}
        </div>
      )}
      {statusMessage && (
        <div role="status" className="shrink-0 border-b border-rc-accent-success-border bg-rc-accent-success-bg px-4 py-2 text-sm text-rc-accent-success">
          {statusMessage}
        </div>
      )}

      {/* ── Compact header ── */}
      <header className="flex shrink-0 items-center gap-3 border-b border-rc-border-primary bg-rc-bg-surface/90 px-4 py-3 backdrop-blur">
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold">
            {!connected ? 'Remote Code' : activeSession ? resolveRemoteSessionTitle(activeSession) : copy.selectRemoteSession}
          </div>
          <div className="mt-0.5 flex items-center gap-2 text-xs text-rc-text-tertiary">
            <ConnectionPill state={connectionState} copy={copy} />
            {activeSession && <StatePill state={activeSession.state} copy={copy} />}
          </div>
        </div>
      </header>

      {/* ── Tab content (full screen) ── */}
      <div className="min-h-0 flex-1 overflow-hidden">
        {activeTab === 'connect' && (
          <MobileConnectTab
            copy={copy}
            connected={connected}
            health={health}
            connectionState={connectionState}
            onClearSavedToken={handleClearSavedToken}
          />
        )}
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
            onSend={() => { haptic('success'); void handleSendPrompt(); }}
            onInterrupt={() => { haptic('warning'); void handleInterrupt(); }}
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
            onApprovalDecision={(id, decision) => {
              haptic(decision === 'approved' ? 'success' : 'warning');
              void handleApprovalDecision(id, decision);
            }}
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
        {activeTab === 'settings' && (
          <MobileSettingsTab
            copy={copy}
            themeMode={themeMode}
            onThemeChange={(mode) => { setThemeMode(mode); haptic('success'); }}
            locale={locale}
            bioEnabled={bioEnabled}
            onToggleBiometric={handleToggleBiometric}
            hapticEnabled={hapticEnabled}
            onToggleHaptic={handleToggleHaptic}
            networkOnline={networkOnline}
            networkType={networkType}
            transportStrategy={transportStrategy}
            transportMetrics={transportMetrics}
            connected={connected}
            health={health}
            deviceName={deviceName}
            onSignOut={handleClearSavedToken}
          />
        )}
      </div>

      {/* ── Bottom tab bar (all 5 tabs always visible) ── */}
      <nav role="tablist" className="flex shrink-0 border-t border-rc-border-primary bg-rc-bg-surface/95 backdrop-blur pb-[env(safe-area-inset-bottom)]">
        <TabButton
          active={activeTab === 'sessions'}
          icon={<List size={20} />}
          label={copy.mobileTabSessions}
          onClick={() => handleTabSwitch('sessions')}
        />
        <TabButton
          active={activeTab === 'timeline'}
          icon={<MessageSquare size={20} />}
          label={copy.mobileTabTimeline}
          onClick={() => handleTabSwitch('timeline')}
        />
        <TabButton
          active={activeTab === 'approvals'}
          icon={<Shield size={20} />}
          label={copy.mobileTabApprovals}
          badge={hasPending ? pendingApprovals.length : undefined}
          onClick={() => handleTabSwitch('approvals')}
        />
        <TabButton
          active={activeTab === 'connect'}
          icon={connected ? <Wifi size={20} /> : <WifiOff size={20} />}
          label={connected ? copy.mobileTabConnected : copy.mobileTabConnect}
          onClick={() => handleTabSwitch('connect')}
        />
        <TabButton
          active={activeTab === 'settings'}
          icon={<Settings size={20} />}
          label={copy.mobileTabSettings}
          onClick={() => handleTabSwitch('settings')}
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
        <div role="alert" className="mb-4 flex items-start gap-3 rounded-2xl border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-3 text-sm text-rc-accent-error">
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
          className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-rc-accent-primary"
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
            className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-rc-accent-primary"
          />
          <input
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            type="password"
            placeholder={copy.passwordPlaceholder}
            autoComplete="current-password"
            className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-rc-accent-primary"
          />
        </div>
        <button
          type="button"
          onClick={onUserSignIn}
          disabled={authLoading || !username.trim() || !password.trim()}
          className="mt-4 flex h-12 w-full items-center justify-center rounded-2xl bg-rc-accent-success text-sm font-semibold text-white active:opacity-80 disabled:opacity-50"
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
                className="mt-3 w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-rc-accent-primary"
              />
              <button
                type="button"
                onClick={onBootstrapClaim}
                disabled={authLoading}
                className="mt-3 flex h-12 w-full items-center justify-center rounded-2xl bg-rc-accent-success text-sm font-semibold text-white active:opacity-80 disabled:opacity-50"
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
                className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-rc-accent-primary"
              />
              <input
                value={pairingSecret}
                onChange={(e) => setPairingSecret(e.target.value)}
                type="password"
                placeholder={copy.pairingSecretPlaceholder}
                className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-rc-accent-primary"
              />
            </div>
            <button
              type="button"
              onClick={onPairingAccept}
              disabled={authLoading}
              className="mt-3 flex h-12 w-full items-center justify-center rounded-2xl bg-rc-accent-primary text-sm font-semibold text-white active:opacity-80 disabled:opacity-50"
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
              className="mt-3 w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-rc-accent-primary"
            />
            <div className="mt-3 flex gap-3">
              <button
                type="button"
                onClick={onManualTokenSave}
                className="flex-1 rounded-2xl border border-rc-border-primary bg-rc-bg-surface py-3 text-sm font-medium text-rc-text-secondary active:opacity-80"
              >
                {copy.saveToken}
              </button>
              <button
                type="button"
                onClick={onClearSavedToken}
                className="flex-1 rounded-2xl border border-rc-border-primary py-3 text-sm font-medium text-rc-text-tertiary active:opacity-80"
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
        <span className="text-sm font-semibold text-rc-text-primary">{copy.mobileTabSessions}</span>
        <button
          type="button"
          onClick={onRefresh}
          className="rounded-xl border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-xs font-medium text-rc-text-secondary active:opacity-80"
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
            <List size={48} className="mb-4 text-rc-text-tertiary/30" />
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
                      ? 'border-rc-accent-success-border bg-rc-accent-success-bg shadow-xs'
                      : 'border-rc-border-primary bg-rc-bg-surface active:opacity-80'
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
                        <span className="rounded bg-rc-bg-tertiary px-1.5 py-0.5 font-mono text-[10px] uppercase text-rc-text-secondary">
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
          <MessageSquare size={48} className="mx-auto mb-4 text-rc-text-tertiary/30" />
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
            <MessageSquare size={48} className="mb-4 text-rc-text-tertiary/30" />
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
          <div className="mb-2 rounded-xl bg-rc-accent-warning-bg px-3 py-2 text-xs leading-5 text-rc-accent-warning">
            {controlStatus.notice}
          </div>
        )}
        <div className="flex items-end gap-2">
          <textarea
            aria-label={copy.followUpPlaceholder}
            value={composer}
            onChange={(e) => {
              onComposerChange(e.target.value);
              e.target.style.height = 'auto';
              e.target.style.height = `${Math.min(e.target.scrollHeight, 120)}px`;
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); onSend(); }
            }}
            rows={1}
            disabled={!controlStatus.canSendPrompt}
            placeholder={copy.followUpPlaceholder}
            className="min-h-[44px] max-h-[120px] flex-1 resize-none rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none placeholder:text-rc-text-tertiary focus:border-rc-accent-primary disabled:opacity-50"
          />
          {controlStatus.canInterrupt && (
            <button
              type="button"
              onClick={onInterrupt}
              disabled={interrupting}
              className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-rc-border-primary bg-rc-bg-hover text-rc-text-secondary active:opacity-80 disabled:opacity-50"
            >
              {interrupting ? <LoaderCircle size={16} className="animate-spin" /> : <Square size={14} />}
            </button>
          )}
          <button
            type="button"
            onClick={onSend}
            disabled={sending || composer.trim().length === 0 || !controlStatus.canSendPrompt}
            className="flex h-11 shrink-0 items-center justify-center rounded-2xl bg-rc-accent-primary px-4 text-sm font-medium text-white shadow-md active:opacity-80 disabled:bg-rc-bg-tertiary disabled:text-rc-text-tertiary"
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
            <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-rc-accent-warning-bg px-1.5 text-[11px] font-bold text-rc-accent-warning">
              {pendingApprovals.length}
            </span>
          )}
        </div>

        {pendingApprovals.length === 0 ? (
          <div className="mt-4 flex flex-col items-center rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-8 text-center">
            <Shield size={36} className="mb-3 text-rc-text-tertiary/30" />
            <div className="text-sm text-rc-text-tertiary">{copy.noPendingApprovals}</div>
          </div>
        ) : (
          <div className="mt-3 space-y-3">
            {pendingApprovals.map((approval) => (
              <div key={approval.approval_id} className="rounded-2xl border border-rc-accent-warning-border bg-rc-bg-surface p-4">
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
          <div className="mt-4 flex flex-col items-center rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-8 text-center">
            <FileOutput size={36} className="mb-3 text-rc-text-tertiary/30" />
            <div className="text-sm text-rc-text-tertiary">{copy.noArtifacts}</div>
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
                    className="rounded-xl border border-rc-border-primary bg-rc-bg-hover px-3 py-2 text-xs font-medium text-rc-text-secondary active:opacity-80 disabled:opacity-50"
                  >
                    {downloadingArtifactId === artifact.artifact_id ? <LoaderCircle size={12} className="animate-spin" /> : copy.renderResponse}
                  </button>
                  <button
                    type="button"
                    onClick={() => onShare(artifact)}
                    className="rounded-xl border border-rc-border-primary bg-rc-bg-hover px-3 py-2 text-xs font-medium text-rc-text-secondary active:opacity-80"
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
// Tab: Connect / Server Configuration
// ═══════════════════════════════════════════════════════════════════════════

function MobileConnectTab({
  copy,
  connected,
  health,
  connectionState,
  onClearSavedToken,
}: {
  copy: ReturnType<typeof getRemoteCopy>;
  connected: boolean;
  health: Awaited<ReturnType<typeof getControlPlaneHealth>> | null;
  connectionState: RemoteConnectionState;
  onClearSavedToken: () => void;
}) {
  const [inputUrl, setInputUrl] = useState('');
  const [saveError, setSaveError] = useState<string | null>(null);

  const handleConnect = useCallback(() => {
    const trimmed = inputUrl.trim();
    if (!trimmed) return;
    try {
      new URL(trimmed);
      persistRemoteBaseUrl(trimmed);
      window.location.reload();
    } catch {
      setSaveError('Invalid URL — please include https://');
    }
  }, [inputUrl]);

  const handleDisconnect = useCallback(() => {
    clearRemoteBaseUrl();
    onClearSavedToken();
    window.location.reload();
  }, [onClearSavedToken]);

  return (
    <div className="h-full overflow-y-auto px-5 py-6">
      {/* Connection status */}
      <div className="mb-6 rounded-3xl border border-rc-border-primary bg-rc-bg-surface p-5 text-center">
        {connected ? (
          <>
            <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-rc-accent-success-bg">
              <Wifi size={24} className="text-rc-accent-success" />
            </div>
            <div className="mt-3 text-sm font-semibold text-rc-text-primary">{copy.connectedTitle}</div>
            <div className="mt-1 text-xs text-rc-text-tertiary break-all">{health?.service}</div>
            <div className="mt-2 flex items-center justify-center gap-2 text-xs text-rc-text-tertiary">
              <ConnectionPill state={connectionState} copy={copy} />
            </div>
            {health && (
              <div className="mt-3 text-xs text-rc-text-tertiary">
                {copy.availableRunnersLabel}: {health.available_runner_count} · {copy.activeSessionsLabel}: {health.session_count}
              </div>
            )}
            <button
              type="button"
              onClick={handleDisconnect}
              className="mt-4 rounded-2xl border border-rc-accent-error-border bg-rc-accent-error-bg px-6 py-2.5 text-sm font-medium text-rc-accent-error active:opacity-80"
            >
              {copy.disconnectAction}
            </button>
          </>
        ) : (
          <>
            <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-rc-bg-secondary">
              <WifiOff size={24} className="text-rc-text-tertiary" />
            </div>
            <div className="mt-3 text-sm font-semibold text-rc-text-primary">{copy.notConnectedTitle}</div>
            <div className="mt-1 text-xs text-rc-text-tertiary">{copy.notConnectedDescription}</div>
          </>
        )}
      </div>

      {/* Server URL input */}
      {!connected && (
        <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-surface p-5">
          <div className="text-sm font-semibold text-rc-text-primary">{copy.enterServerUrlTitle}</div>
          <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.enterServerUrlDescription}</div>
          <input
            value={inputUrl}
            onChange={(e) => { setInputUrl(e.target.value); setSaveError(null); }}
            placeholder="https://your-server.example.com"
            autoCapitalize="none"
            autoCorrect="off"
            className="mt-3 w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3.5 text-sm text-rc-text-primary outline-none focus:border-rc-accent-primary"
          />
          {saveError && (
            <div className="mt-2 text-xs text-rc-accent-error">{saveError}</div>
          )}
          <button
            type="button"
            onClick={handleConnect}
            disabled={!inputUrl.trim()}
            className="mt-3 flex h-12 w-full items-center justify-center rounded-2xl bg-rc-accent-success text-sm font-semibold text-white active:opacity-80 disabled:opacity-50"
          >
            {copy.connectAction}
          </button>
          <button
            type="button"
            className="mt-3 flex w-full items-center justify-center gap-2 rounded-2xl border border-rc-border-primary bg-rc-bg-hover py-3 text-sm font-medium text-rc-text-secondary active:opacity-80"
          >
            {copy.scanQrAction}
          </button>
        </div>
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tab: Settings
// ═══════════════════════════════════════════════════════════════════════════

function MobileSettingsTab({
  copy,
  themeMode,
  onThemeChange,
  locale,
  bioEnabled,
  onToggleBiometric,
  hapticEnabled,
  onToggleHaptic,
  networkOnline,
  networkType,
  transportStrategy,
  transportMetrics,
  connected,
  health,
  deviceName,
  onSignOut,
}: {
  copy: ReturnType<typeof getRemoteCopy>;
  themeMode: import('../components/design/ThemeProvider').ThemeMode;
  onThemeChange: (mode: import('../components/design/ThemeProvider').ThemeMode) => void;
  locale: ReturnType<typeof resolveRemoteLocale>;
  bioEnabled: boolean;
  onToggleBiometric: () => void;
  hapticEnabled: boolean;
  onToggleHaptic: () => void;
  networkOnline: boolean;
  networkType: string;
  transportStrategy: string | null;
  transportMetrics: { latencyMs: number | null; eventsReceived: number; commandsSent: number } | null;
  connected: boolean;
  health: RemoteControlPlaneHealth | null;
  deviceName: string;
  onSignOut: () => void;
}) {
  const handleLocaleSwitch = useCallback(() => {
    const current = window.location.search;
    const nextLocale = locale === 'en' ? 'zh' : 'en';
    const url = new URL(window.location.href);
    url.searchParams.set('lang', nextLocale);
    window.location.replace(url.toString());
  }, [locale]);

  return (
    <div className="h-full overflow-y-auto px-4 py-4">
      {/* ── Appearance ── */}
      <SettingsSection title={copy.settingsAppearance}>
        <SettingsRow icon={<Sun size={16} />} label={copy.settingsTheme}>
          <div className="flex rounded-lg border border-rc-border-primary bg-rc-bg-base p-0.5">
            {([
              { mode: 'light' as const, icon: <Sun size={14} />, label: copy.settingsThemeLight },
              { mode: 'dark' as const, icon: <Moon size={14} />, label: copy.settingsThemeDark },
              { mode: 'system' as const, icon: <Monitor size={14} />, label: copy.settingsThemeSystem },
            ]).map((opt) => (
              <button
                key={opt.mode}
                type="button"
                onClick={() => onThemeChange(opt.mode)}
                className={`flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs font-medium transition-colors ${
                  themeMode === opt.mode
                    ? 'bg-rc-accent-primary text-white'
                    : 'text-rc-text-tertiary active:text-rc-text-secondary'
                }`}
              >
                {opt.icon}
                <span>{opt.label}</span>
              </button>
            ))}
          </div>
        </SettingsRow>

        <div className="border-t border-rc-border-secondary" />

        <SettingsRow icon={<Globe size={16} />} label={copy.settingsLanguage}>
          <button
            type="button"
            onClick={handleLocaleSwitch}
            className="rounded-lg border border-rc-border-primary bg-rc-bg-base px-3 py-1.5 text-xs font-medium text-rc-text-secondary active:opacity-80"
          >
            {locale === 'en' ? copy.settingsLanguageEn : copy.settingsLanguageZh}
          </button>
        </SettingsRow>
      </SettingsSection>

      {/* ── Security ── */}
      <SettingsSection title={copy.settingsSecurity}>
        <SettingsRow icon={<Fingerprint size={16} />} label={copy.settingsBiometric}>
          <ToggleSwitch checked={bioEnabled} onChange={onToggleBiometric} />
        </SettingsRow>

        <div className="border-t border-rc-border-secondary" />

        <SettingsRow icon={<Vibrate size={16} />} label={copy.settingsHaptic}>
          <ToggleSwitch checked={hapticEnabled} onChange={onToggleHaptic} />
        </SettingsRow>
      </SettingsSection>

      {/* ── Network & Transport ── */}
      <SettingsSection title={copy.settingsNetwork}>
        <SettingsRow
          icon={<Wifi size={16} className={networkOnline ? 'text-rc-accent-success' : 'text-rc-accent-error'} />}
          label={networkOnline ? copy.settingsNetworkOnline : copy.settingsNetworkOffline}
        >
          <span className="text-xs text-rc-text-tertiary">{describeConnectionType(networkType)}</span>
        </SettingsRow>

        {connected && transportStrategy && (
          <>
            <div className="border-t border-rc-border-secondary" />
            <SettingsRow icon={<Settings size={16} />} label={copy.settingsTransportStrategy}>
              <span className="text-xs font-medium text-rc-text-secondary">{transportStrategy.replace('_', ' ')}</span>
            </SettingsRow>
            {transportMetrics?.latencyMs != null && (
              <>
                <div className="border-t border-rc-border-secondary" />
                <SettingsRow icon={<Info size={16} />} label={copy.settingsLatency}>
                  <span className="text-xs font-medium text-rc-text-secondary">{transportMetrics.latencyMs} ms</span>
                </SettingsRow>
              </>
            )}
          </>
        )}
      </SettingsSection>

      {/* ── About ── */}
      <SettingsSection title={copy.settingsAbout}>
        <SettingsRow icon={<Info size={16} />} label={copy.settingsAppVersion}>
          <span className="text-xs text-rc-text-tertiary">1.0.0</span>
        </SettingsRow>

        <div className="border-t border-rc-border-secondary" />

        <SettingsRow icon={<Bell size={16} />} label={copy.settingsDeviceName}>
          <span className="text-xs text-rc-text-tertiary">{deviceName}</span>
        </SettingsRow>

        {health && (
          <>
            <div className="border-t border-rc-border-secondary" />
            <SettingsRow icon={<Wifi size={16} />} label="Server">
              <span className="max-w-[160px] truncate text-xs text-rc-text-tertiary">{health.service}</span>
            </SettingsRow>
          </>
        )}
      </SettingsSection>

      {/* ── Account ── */}
      {connected && (
        <SettingsSection title="Account">
          <div className="px-4 py-3">
            <button
              type="button"
              onClick={onSignOut}
              className="w-full rounded-xl border border-rc-accent-error-border bg-rc-accent-error-bg py-3 text-sm font-semibold text-rc-accent-error active:opacity-80"
            >
              {copy.signOutAction}
            </button>
          </div>
        </SettingsSection>
      )}
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
  icon: ReactNode;
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
      className={`relative flex flex-1 flex-col items-center gap-0.5 py-2 text-[10px] font-medium transition-colors ${
        active ? 'text-rc-accent-primary' : 'text-rc-text-tertiary active:text-rc-text-secondary'
      }`}
    >
      {icon}
      <span className="truncate max-w-[56px]">{label}</span>
      {badge != null && badge > 0 && (
        <span className="absolute right-1/4 top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-rc-accent-error px-1 text-[10px] font-bold text-white">
          {badge}
        </span>
      )}
    </button>
  );
}

function ToggleSwitch({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={onChange}
      className={`relative h-7 w-12 rounded-full transition-colors ${
        checked ? 'bg-rc-accent-success' : 'bg-rc-bg-active'
      }`}
    >
      <span className={`absolute top-0.5 h-6 w-6 rounded-full bg-white shadow-sm transition-transform ${
        checked ? 'translate-x-5' : 'translate-x-0.5'
      }`} />
    </button>
  );
}

function SettingsSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-4">
      <div className="mb-2 px-1 text-xs font-semibold uppercase tracking-wider text-rc-text-tertiary">
        {title}
      </div>
      <div className="rounded-2xl border border-rc-border-primary bg-rc-bg-surface overflow-hidden">
        {children}
      </div>
    </div>
  );
}

function SettingsRow({ icon, label, children }: { icon?: ReactNode; label: string; children: React.ReactNode }) {
  return (
    <div className="flex min-h-[44px] items-center justify-between px-4 py-3">
      <div className="flex items-center gap-3 text-sm text-rc-text-primary">
        {icon && <span className="text-rc-text-tertiary">{icon}</span>}
        {label}
      </div>
      {children}
    </div>
  );
}

function ConnectionPill({ state, copy }: { state: RemoteConnectionState; copy: ReturnType<typeof getRemoteCopy> }) {
  const className = state === 'open'
    ? 'text-rc-accent-success'
    : state === 'error'
    ? 'text-rc-accent-error'
    : 'text-rc-accent-warning';

  return (
    <span className={`inline-flex items-center gap-1 text-xs font-medium ${className}`}>
      {state === 'open' ? <Wifi size={12} /> : state === 'error' ? <AlertTriangle size={12} /> : <WifiOff size={12} />}
      {copy.connectionLabels[state]}
    </span>
  );
}

function StatePill({ state, copy }: { state: import('./types').RemoteSessionState; copy: ReturnType<typeof getRemoteCopy> }) {
  const className = state === 'running'
    ? 'bg-rc-accent-success-bg text-rc-accent-success'
    : state === 'waiting_approval'
    ? 'bg-rc-accent-warning-bg text-rc-accent-warning'
    : state === 'failed'
    ? 'bg-rc-accent-error-bg text-rc-accent-error'
    : 'bg-rc-bg-tertiary text-rc-text-secondary';

  return (
    <span className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium ${className}`}>
      {copy.sessionStateLabels[state]}
    </span>
  );
}
