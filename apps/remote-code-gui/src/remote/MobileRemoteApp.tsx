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
  Bot,
  ChevronDown,
  ChevronUp,
  Database,
  FileOutput,
  GitBranch,
  Layers,
  List,
  LoaderCircle,
  MessageSquare,
  MessageSquareText,
  Shield,
  Square,
  Wifi,
  WifiOff,
} from 'lucide-react';
import {
  Suspense,
  lazy,
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
import { TimelineEventCard } from '../components/shared/TimelineEventCard';
import { TimelineMessageCard } from '../components/shared/TimelineMessageCard';
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
import { resolveRemoteRunnerBaseUrl, resolveRemoteTransportStrategy } from './transportMode';
import { useConnection } from './useConnection';
import type { TransportConfig } from './connection-manager';
import type {
  RemoteApprovalDecision,
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteControlPlaneHealth,
  RemoteSessionRecord,
  RemoteTimelineEvent,
  RemoteTimelineEventDetail,
} from './types';

const LazyMarkdownRenderer = lazy(() => import('../components/chat/MarkdownRenderer'));

type MobileTab = 'sessions' | 'timeline' | 'approvals';

const APPROVAL_DECISIONS: Array<{
  decision: RemoteApprovalDecision;
  className: string;
}> = [
  { decision: 'approved', className: 'bg-[#1d6b45] text-white active:bg-[#145033]' },
  { decision: 'denied', className: 'bg-[#a13a30] text-white active:bg-[#7e2b24]' },
  { decision: 'cancelled', className: 'bg-[#efe7db] text-slate-700 active:bg-[#e6dccd]' },
];

// ═══════════════════════════════════════════════════════════════════════════
// MobileRemoteApp
// ═══════════════════════════════════════════════════════════════════════════

export default function MobileRemoteApp() {
  const baseUrl = resolveRemoteBaseUrl();
  const locale = useMemo(() => resolveRemoteLocale(), []);
  const copy = useMemo(() => getRemoteCopy(locale), [locale]);
  const initialPairingContext = resolveRemotePairingContext();

  // ── Auth state ────────────────────────────────────────────────────────
  const [accessToken, setAccessToken] = useState<string | null>(() => resolveRemoteAccessToken());
  const [health, setHealth] = useState<RemoteControlPlaneHealth | null>(null);
  const [authLoading, setAuthLoading] = useState(false);
  const [authErrorMessage, setAuthErrorMessage] = useState<string | null>(null);
  const [manualAccessToken, setManualAccessToken] = useState('');
  const [bootstrapSecret, setBootstrapSecret] = useState('');
  const [signInUsername, setSignInUsername] = useState('');
  const [signInPassword, setSignInPassword] = useState('');
  const [deviceName, setDeviceName] = useState('Mobile');
  const [pairingOfferId, setPairingOfferId] = useState(initialPairingContext.offerId ?? '');
  const [pairingSecret, setPairingSecret] = useState(initialPairingContext.pairingSecret ?? '');

  // ── Data state ────────────────────────────────────────────────────────
  const [sessions, setSessions] = useState<RemoteSessionRecord[]>([]);
  const [sessionsLoading, setSessionsLoading] = useState(true);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(() =>
    resolveRemoteActiveSessionId(baseUrl),
  );
  const [events, setEvents] = useState<RemoteTimelineEvent[]>([]);
  const deferredEvents = useDeferredValue(events);
  const [eventsLoading, setEventsLoading] = useState(false);
  const [approvals, setApprovals] = useState<RemoteApprovalRecord[]>([]);
  const [artifacts, setArtifacts] = useState<RemoteArtifactRecord[]>([]);

  // ── UI state ──────────────────────────────────────────────────────────
  const [composer, setComposer] = useState('');
  const [sending, setSending] = useState(false);
  const [interrupting, setInterrupting] = useState(false);
  const [approvingId, setApprovingId] = useState<string | null>(null);
  const [downloadingArtifactId, setDownloadingArtifactId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<MobileTab>('sessions');

  // ── Refs ──────────────────────────────────────────────────────────────
  const activeSessionIdRef = useRef<string | null>(null);
  const latestSequenceRef = useRef(0);
  const connectedSessionRef = useRef<string | null>(null);
  const sessionRefreshTimerRef = useRef<number | null>(null);
  const statusTimerRef = useRef<number | null>(null);

  // ── Computed ──────────────────────────────────────────────────────────
  const activeSession = useMemo(
    () => sessions.find((s) => s.session_id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );
  const selectedSessionId = activeSession?.session_id ?? null;
  useEffect(() => { activeSessionIdRef.current = selectedSessionId; });

  const activeSessionControlStatus = useMemo(
    () => describeSessionControl(activeSession, locale, copy),
    [activeSession, copy, locale],
  );
  const pendingApprovals = useMemo(
    () => approvals.filter((a) => a.state === 'pending'),
    [approvals],
  );
  const approvalActions = useMemo(
    () => APPROVAL_DECISIONS.map((item) => ({ ...item, label: copy.approvalDecisionLabels[item.decision] })),
    [copy],
  );
  const authRequired = health?.auth_required ?? false;
  const showAuthGate = Boolean(baseUrl) && ((authRequired && !accessToken) || authErrorMessage);

  // ── Utility callbacks ─────────────────────────────────────────────────
  const showStatusMessage = useEffectEvent((message: string) => {
    setStatusMessage(message);
    if (statusTimerRef.current !== null) window.clearTimeout(statusTimerRef.current);
    statusTimerRef.current = window.setTimeout(() => { setStatusMessage(null); statusTimerRef.current = null; }, 3000);
  });

  const reportAsyncError = useEffectEvent((error: unknown) => {
    const message = extractErrorMessage(error);
    if (message.includes('HTTP 401')) setAuthErrorMessage(message);
    else setErrorMessage(message);
  });

  const scheduleSessionsRefresh = useEffectEvent(() => {
    if (sessionRefreshTimerRef.current !== null) return;
    sessionRefreshTimerRef.current = window.setTimeout(() => {
      sessionRefreshTimerRef.current = null;
      void refreshSessions().catch(reportAsyncError);
    }, 350);
  });

  // ── Locale ────────────────────────────────────────────────────────────
  useEffect(() => { document.documentElement.lang = locale; }, [locale]);

  // ── Active session persistence ────────────────────────────────────────
  useEffect(() => {
    if (accessToken || !baseUrl) return;
    let cancelled = false;
    void hydrateRemoteAuthTokensFromSecureStore().then((token) => {
      if (!cancelled && token) setAccessToken(token);
    });
    return () => { cancelled = true; };
  }, [accessToken, baseUrl]);

  useEffect(() => {
    if (selectedSessionId) { persistRemoteActiveSessionId(baseUrl, selectedSessionId); return; }
    clearRemoteActiveSessionId(baseUrl);
  }, [baseUrl, selectedSessionId]);

  // ── Health check ──────────────────────────────────────────────────────
  useEffect(() => {
    if (!baseUrl) return;
    let cancelled = false;
    void getControlPlaneHealth(baseUrl)
      .then((r) => { if (!cancelled) setHealth(r); })
      .catch((e) => { if (!cancelled) reportAsyncError(e); });
    return () => { cancelled = true; };
  }, [baseUrl, accessToken]);

  // ── Auth handlers ─────────────────────────────────────────────────────

  const completeAuthentication = useEffectEvent((token: string, message: string, refreshToken?: string) => {
    persistRemoteAccessToken(token);
    if (refreshToken) persistRemoteRefreshToken(refreshToken);
    void clearRemotePairingContext();
    stripRemoteSensitiveQueryParams();
    setBootstrapSecret(''); setPairingOfferId(''); setPairingSecret('');
    setManualAccessToken(''); setSignInUsername(''); setSignInPassword('');
    setAccessToken(token); setAuthErrorMessage(null); setErrorMessage(null);
    showStatusMessage(message);
  });

  const handleBootstrapClaim = useEffectEvent(async () => {
    if (!baseUrl || authLoading) return;
    setAuthLoading(true);
    try {
      const r = await bootstrapControlPlane(baseUrl, bootstrapSecret, deviceName);
      completeAuthentication(r.access_token, copy.statusBootstrapClaimSucceeded, r.refresh_token);
      setHealth(await getControlPlaneHealth(baseUrl));
    } catch (e) { reportAsyncError(e); }
    finally { setAuthLoading(false); }
  });

  const handlePairingAccept = useEffectEvent(async () => {
    if (!baseUrl || authLoading) return;
    setAuthLoading(true);
    try {
      const r = await acceptPairingOffer(baseUrl, pairingOfferId, pairingSecret, deviceName);
      completeAuthentication(r.access_token, copy.statusPairingSucceeded, r.refresh_token);
      setHealth(await getControlPlaneHealth(baseUrl));
    } catch (e) { reportAsyncError(e); }
    finally { setAuthLoading(false); }
  });

  const handleManualTokenSave = useEffectEvent(() => {
    if (!manualAccessToken.trim()) return;
    persistRemoteAccessToken(manualAccessToken);
    void clearRemotePairingContext(); stripRemoteSensitiveQueryParams();
    setBootstrapSecret(''); setPairingOfferId(''); setPairingSecret('');
    setManualAccessToken(''); setSignInUsername(''); setSignInPassword('');
    setAccessToken(manualAccessToken.trim()); setAuthErrorMessage(null);
    showStatusMessage(copy.statusSavedAccessToken);
  });

  const handleClearSavedToken = useEffectEvent(() => {
    clearRemoteAccessToken(); clearRemoteActiveSessionId(baseUrl);
    void clearRemotePairingContext(); stripRemoteSensitiveQueryParams();
    setBootstrapSecret(''); setPairingOfferId(''); setPairingSecret('');
    setAccessToken(null); setManualAccessToken(''); setSignInUsername('');
    setSignInPassword(''); setAuthErrorMessage(null);
    showStatusMessage(copy.statusClearedAccessToken);
  });

  const handleUserSignIn = useEffectEvent(async () => {
    if (!baseUrl || authLoading || !signInUsername.trim() || !signInPassword.trim()) return;
    setAuthLoading(true);
    try {
      const userKey = await deriveUserKey(signInUsername.trim(), signInPassword.trim());
      completeAuthentication(userKey, copy.statusSignInSucceeded);
      setHealth(await getControlPlaneHealth(baseUrl));
    } catch (e) { reportAsyncError(e); }
    finally { setAuthLoading(false); }
  });

  // ── Mobile integrations ───────────────────────────────────────────────

  useEffect(() => {
    let cancelled = false;
    void initDeepLinks((url) => {
      if (cancelled) return;
      const pairing = parsePairingUrl(url);
      if (pairing) {
        setPairingOfferId(pairing.offerId); setPairingSecret(pairing.secret);
        showStatusMessage(copy.deepLinkPairingReceived);
      }
    });
    return () => { cancelled = true; };
  }, [copy]);

  useEffect(() => {
    if (!accessToken || !baseUrl) return;
    let cancelled = false;
    void (async () => {
      await initPushNotifications({
        onApproval: (approvalId, sessionId) => {
          if (cancelled) return;
          if (sessionId === activeSessionIdRef.current) void refreshApprovals(sessionId).catch(reportAsyncError);
          void showLocalNotification(copy.pushNotificationApprovalTitle, copy.pushNotificationApprovalBody(approvalId));
          scheduleSessionsRefresh();
        },
        onSessionUpdate: (sessionId) => {
          if (cancelled) return;
          scheduleSessionsRefresh();
          if (sessionId === activeSessionIdRef.current) void refreshSessionBundle(sessionId).catch(reportAsyncError);
        },
      });
      if (!cancelled) {
        const registered = await registerPushTokenWithControlPlane(baseUrl, accessToken);
        if (!cancelled && !registered) showStatusMessage(copy.mobileNotificationsUnavailable);
      }
    })();
    return () => { cancelled = true; };
  }, [accessToken, baseUrl, copy]);

  useEffect(() => {
    void initAppLifecycle({
      onResume: () => {
        void refreshSessions().catch(reportAsyncError);
        if (activeSessionIdRef.current) void refreshSessionBundle(activeSessionIdRef.current).catch(reportAsyncError);
      },
    });
  }, []);

  // ── Data fetching ─────────────────────────────────────────────────────

  const refreshSessions = useEffectEvent(async () => {
    if (!baseUrl || !health || (authRequired && !accessToken)) return;
    setSessionsLoading((cur) => (sessions.length === 0 ? true : cur));
    try {
      const r = await listSessions(baseUrl);
      const next = [...r.items].sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
      setSessions(next);
      setActiveSessionId((cur) => {
        if (cur && next.some((s) => s.session_id === cur)) return cur;
        const stored = resolveRemoteActiveSessionId(baseUrl);
        if (stored && next.some((s) => s.session_id === stored)) return stored;
        return next[0]?.session_id ?? null;
      });
      setErrorMessage(null);
    } catch (e) { reportAsyncError(e); }
    finally { setSessionsLoading(false); }
  });

  const refreshApprovals = useEffectEvent(async (sessionId: string) => {
    if (!baseUrl || !health || (authRequired && !accessToken)) return;
    const r = await listSessionApprovals(baseUrl, sessionId);
    if (activeSessionIdRef.current !== sessionId) return;
    setApprovals([...r.items].sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()));
  });

  const refreshArtifacts = useEffectEvent(async (sessionId: string) => {
    if (!baseUrl || !health || (authRequired && !accessToken)) return;
    const r = await listSessionArtifacts(baseUrl, sessionId);
    if (activeSessionIdRef.current !== sessionId) return;
    setArtifacts([...r.items].sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()));
  });

  const refreshSessionBundle = useEffectEvent(async (sessionId: string) => {
    if (!baseUrl || !health || (authRequired && !accessToken)) return;
    const bundle = await loadRemoteSessionBundle(baseUrl, sessionId);
    if (activeSessionIdRef.current !== sessionId) return;
    latestSequenceRef.current = bundle.latestSequence;
    startTransition(() => { setEvents(bundle.events); });
    setApprovals(bundle.approvals);
    setArtifacts(bundle.artifacts);
  });

  // ── Transport ─────────────────────────────────────────────────────────

  const handleTransportEvent = useEffectEvent((event: RemoteTimelineEvent) => {
    const sessionId = connectedSessionRef.current;
    if (!sessionId) return;

    latestSequenceRef.current = Math.max(latestSequenceRef.current, event.sequence);
    startTransition(() => { setEvents((cur) => appendRemoteTimelineEvent(cur, event)); });

    if (event.detail.kind === 'approval_requested' || event.detail.kind === 'approval_resolved') {
      void refreshApprovals(sessionId).catch(reportAsyncError);
    }
    if (event.detail.kind === 'artifact_created' || event.detail.kind === 'artifact_manifest') {
      void refreshArtifacts(sessionId).catch(reportAsyncError);
    }
    if (event.detail.kind === 'approval_requested' || event.detail.kind === 'approval_resolved' ||
        event.detail.kind === 'daemon_presence_changed') {
      scheduleSessionsRefresh();
    }
    if (event.detail.kind === 'session_state_changed' && event.detail.previous_state !== event.detail.state) {
      scheduleSessionsRefresh();
    }
  });

  const {
    connectionState: transportConnectionState,
    strategy: transportStrategy,
    metrics: transportMetrics,
    connect: transportConnect,
    disconnect: transportDisconnect,
    latestSequence: transportSequence,
  } = useConnection(handleTransportEvent);

  const connectionState: RemoteConnectionState =
    transportConnectionState === 'probing' ? 'connecting' : transportConnectionState;

  useEffect(() => {
    latestSequenceRef.current = Math.max(latestSequenceRef.current, transportSequence);
  }, [transportSequence]);

  // ── Periodic refresh ──────────────────────────────────────────────────

  useEffect(() => {
    void refreshSessions();
    const id = window.setInterval(() => { void refreshSessions(); }, 15_000);
    return () => { window.clearInterval(id); };
  }, [accessToken, baseUrl, health]);

  useEffect(() => {
    const onVis = () => {
      if (document.visibilityState !== 'visible') return;
      void refreshSessions().catch(reportAsyncError);
      if (activeSessionIdRef.current) void refreshSessionBundle(activeSessionIdRef.current).catch(reportAsyncError);
    };
    document.addEventListener('visibilitychange', onVis);
    return () => { document.removeEventListener('visibilitychange', onVis); };
  }, []);

  useEffect(() => {
    const onOnline = () => {
      void refreshSessions().catch(reportAsyncError);
      if (activeSessionIdRef.current) void refreshSessionBundle(activeSessionIdRef.current).catch(reportAsyncError);
    };
    window.addEventListener('online', onOnline);
    return () => { window.removeEventListener('online', onOnline); };
  }, []);

  useEffect(() => {
    return () => {
      if (sessionRefreshTimerRef.current !== null) window.clearTimeout(sessionRefreshTimerRef.current);
      if (statusTimerRef.current !== null) window.clearTimeout(statusTimerRef.current);
    };
  }, []);

  // ── Transport subscription ────────────────────────────────────────────

  useEffect(() => {
    if (!baseUrl || !selectedSessionId || !health || (authRequired && !accessToken)) {
      setEvents([]); setApprovals([]); setArtifacts([]);
      connectedSessionRef.current = null;
      transportDisconnect(); latestSequenceRef.current = 0;
      return;
    }
    connectedSessionRef.current = selectedSessionId;
    let cancelled = false;

    const bootstrap = async () => {
      setEventsLoading(true);
      try {
        await refreshSessionBundle(selectedSessionId);
        if (!cancelled) {
          const runnerBaseUrl = resolveRemoteRunnerBaseUrl(activeSession);
          const config: TransportConfig = {
            strategy: resolveRemoteTransportStrategy(activeSession),
            baseUrl,
            runnerBaseUrl,
            sessionId: selectedSessionId,
            authToken: accessToken,
          };
          await transportConnect(config, latestSequenceRef.current);
          setErrorMessage(null);
        }
      } catch (e) { if (!cancelled) reportAsyncError(e); }
      finally { if (!cancelled) setEventsLoading(false); }
    };
    void bootstrap();
    return () => { cancelled = true; connectedSessionRef.current = null; transportDisconnect(); };
  }, [accessToken, activeSession, authRequired, baseUrl, health, selectedSessionId]);

  // ── Action handlers ───────────────────────────────────────────────────

  const handleSendPrompt = async () => {
    if (!baseUrl || !selectedSessionId || (authRequired && !accessToken) ||
        !composer.trim() || sending || !activeSessionControlStatus.canSendPrompt) return;
    setSending(true);
    try {
      await sendPrompt(baseUrl, selectedSessionId, composer.trim(), resolveRemoteRunnerBaseUrl(activeSession) ?? undefined);
      setComposer('');
      showStatusMessage(copy.statusPromptForwarded);
    } catch (e) { reportAsyncError(e); }
    finally { setSending(false); }
  };

  const handleInterrupt = async () => {
    if (!baseUrl || !selectedSessionId || (authRequired && !accessToken) ||
        interrupting || !activeSessionControlStatus.canInterrupt) return;
    setInterrupting(true);
    try {
      await interruptSession(baseUrl, selectedSessionId, resolveRemoteRunnerBaseUrl(activeSession) ?? undefined);
      showStatusMessage(copy.statusInterruptForwarded);
    } catch (e) { reportAsyncError(e); }
    finally { setInterrupting(false); }
  };

  const handleApprovalDecision = async (approvalId: string, decision: RemoteApprovalDecision) => {
    if (!baseUrl || (authRequired && !accessToken) || approvingId) return;
    setApprovingId(approvalId);
    try {
      await respondToApproval(baseUrl, approvalId, decision, undefined, resolveRemoteRunnerBaseUrl(activeSession) ?? undefined);
      showStatusMessage(copy.statusApprovalDecision(copy.approvalDecisionLabels[decision]));
      if (selectedSessionId) await refreshApprovals(selectedSessionId);
    } catch (e) { reportAsyncError(e); }
    finally { setApprovingId(null); }
  };

  const handleArtifactDownload = async (artifact: RemoteArtifactRecord) => {
    if (!baseUrl || (authRequired && !accessToken) || downloadingArtifactId) return null;
    setDownloadingArtifactId(artifact.artifact_id);
    try {
      const filePath = await downloadRemoteArtifact({
        url: buildArtifactDownloadUrl(baseUrl, artifact.artifact_id),
        fileName: artifact.file_name,
        token: accessToken,
      });
      showStatusMessage(copy.statusArtifactDownloaded(artifact.file_name));
      return filePath;
    } catch (e) { reportAsyncError(e); return null; }
    finally { setDownloadingArtifactId(null); }
  };

  const handleArtifactShare = async (artifact: RemoteArtifactRecord) => {
    if (!baseUrl || (authRequired && !accessToken)) return;
    try {
      const filePath = await handleArtifactDownload(artifact);
      if (filePath) await shareFile(filePath, artifact.file_name);
    } catch (e) { reportAsyncError(e); }
  };

  // ── Select session → auto-switch to timeline ──────────────────────────
  const handleSelectSession = useCallback((id: string) => {
    setActiveSessionId(id);
    setActiveTab('timeline');
  }, []);

  // ═══════════════════════════════════════════════════════════════════════
  // Render
  // ═══════════════════════════════════════════════════════════════════════

  // Early exits
  if (!baseUrl) {
    return (
      <div className="flex h-dvh items-center justify-center bg-gradient-to-b from-[#f4efe4] to-[#efe8db] px-6">
        <div className="max-w-sm rounded-3xl border border-[#e1d7c8] bg-white p-8 text-center shadow-lg">
          <div className="text-lg font-semibold text-slate-900">{copy.remoteModeNotConfiguredTitle}</div>
          <div className="mt-3 text-sm leading-6 text-slate-500">{copy.remoteModeNotConfiguredDescription}</div>
        </div>
      </div>
    );
  }

  if (!health) {
    return (
      <div className="flex h-dvh items-center justify-center bg-gradient-to-b from-[#f4efe4] to-[#efe8db]">
        <div className="flex items-center gap-3 rounded-2xl border border-[#e2d8c8] bg-white px-5 py-4 text-sm text-slate-600 shadow-lg">
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
  const hasPending = pendingApprovals.length > 0;

  return (
    <div className="flex h-dvh flex-col bg-gradient-to-b from-[#f4efe4] to-[#efe8db] text-slate-900">
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
      <header className="flex shrink-0 items-center justify-between border-b border-[#e5ddcf] bg-white/90 px-4 py-3 backdrop-blur">
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold">
            {activeSession ? resolveRemoteSessionTitle(activeSession) : copy.selectRemoteSession}
          </div>
          <div className="mt-0.5 flex items-center gap-2 text-xs text-slate-500">
            <ConnectionPill state={connectionState} copy={copy} />
            {activeSession && <StatePill state={activeSession.state} copy={copy} />}
          </div>
        </div>
        <button
          type="button"
          onClick={handleClearSavedToken}
          className="ml-3 shrink-0 rounded-xl border border-[#ddd4c5] bg-[#faf6ef] px-3 py-2 text-xs font-medium text-slate-600 active:bg-white"
        >
          {copy.signOutAction}
        </button>
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
      <nav className="flex shrink-0 border-t border-[#e5ddcf] bg-white/95 backdrop-blur pb-[env(safe-area-inset-bottom)]">
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
    <div className="flex h-dvh flex-col overflow-y-auto bg-gradient-to-b from-[#f4efe4] to-[#efe8db] px-5 py-8">
      {/* Header */}
      <div className="mb-8 text-center">
        <div className="text-[11px] font-semibold uppercase tracking-[0.32em] text-slate-400">
          {copy.authGateEyebrow}
        </div>
        <div className="mt-3 text-2xl font-bold text-slate-900">{copy.authGateTitle}</div>
        <div className="mt-2 text-sm leading-6 text-slate-500">{copy.mobileAuthSubtitle}</div>
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
        <div className="mb-2 text-xs font-semibold uppercase tracking-[0.2em] text-slate-400">
          {copy.deviceNameLabel}
        </div>
        <input
          value={deviceName}
          onChange={(e) => setDeviceName(e.target.value)}
          placeholder={copy.deviceNamePlaceholder}
          className="w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3.5 text-sm text-slate-800 outline-none focus:border-[#a58a5e]"
        />
      </label>

      {/* Sign in (primary) */}
      <div className="rounded-3xl border border-[#e7dccd] bg-white p-5">
        <div className="text-sm font-semibold text-slate-900">{copy.multiUserTitle}</div>
        <div className="mt-2 text-sm leading-6 text-slate-500">{copy.multiUserDescription}</div>
        <div className="mt-4 grid gap-3">
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder={copy.usernamePlaceholder}
            autoComplete="username"
            className="w-full rounded-2xl border border-[#dfd5c6] bg-[#fcfaf6] px-4 py-3.5 text-sm text-slate-800 outline-none focus:border-[#a58a5e]"
          />
          <input
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            type="password"
            placeholder={copy.passwordPlaceholder}
            autoComplete="current-password"
            className="w-full rounded-2xl border border-[#dfd5c6] bg-[#fcfaf6] px-4 py-3.5 text-sm text-slate-800 outline-none focus:border-[#a58a5e]"
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
        className="mt-5 flex items-center justify-center gap-2 text-sm font-medium text-slate-500 active:text-slate-700"
      >
        {showOptions ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
        {showOptions ? copy.mobileCollapseOptions : copy.mobileExpandOptions}
      </button>

      {showOptions && (
        <div className="mt-3 space-y-4">
          {bootstrapEnabled && (
            <div className="rounded-3xl border border-[#e7dccd] bg-white p-5">
              <div className="text-sm font-semibold text-slate-900">{copy.bootstrapTitle}</div>
              <div className="mt-2 text-sm leading-6 text-slate-500">{copy.bootstrapDescription}</div>
              <input
                type="password"
                onChange={(e) => setBootstrapSecret(e.target.value)}
                placeholder={copy.bootstrapSecretLabel}
                className="mt-3 w-full rounded-2xl border border-[#dfd5c6] bg-[#fcfaf6] px-4 py-3.5 text-sm text-slate-800 outline-none focus:border-[#a58a5e]"
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

          <div className="rounded-3xl border border-[#e7dccd] bg-white p-5">
            <div className="text-sm font-semibold text-slate-900">{copy.acceptPairingTitle}</div>
            <div className="mt-2 text-sm leading-6 text-slate-500">{copy.acceptPairingDescription}</div>
            <div className="mt-3 grid gap-3">
              <input
                value={pairingOfferId}
                onChange={(e) => setPairingOfferId(e.target.value)}
                placeholder={copy.offerIdPlaceholder}
                className="w-full rounded-2xl border border-[#dfd5c6] bg-[#fcfaf6] px-4 py-3.5 text-sm text-slate-800 outline-none focus:border-[#a58a5e]"
              />
              <input
                value={pairingSecret}
                onChange={(e) => setPairingSecret(e.target.value)}
                type="password"
                placeholder={copy.pairingSecretPlaceholder}
                className="w-full rounded-2xl border border-[#dfd5c6] bg-[#fcfaf6] px-4 py-3.5 text-sm text-slate-800 outline-none focus:border-[#a58a5e]"
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

          <div className="rounded-3xl border border-[#e7dccd] bg-white p-5">
            <div className="text-sm font-semibold text-slate-900">{copy.existingTokenTitle}</div>
            <div className="mt-2 text-sm leading-6 text-slate-500">{copy.existingTokenDescription}</div>
            <textarea
              value={manualAccessToken}
              onChange={(e) => setManualAccessToken(e.target.value)}
              rows={2}
              placeholder="rcdt_..."
              className="mt-3 w-full rounded-2xl border border-[#dfd5c6] bg-[#fcfaf6] px-4 py-3.5 text-sm text-slate-800 outline-none focus:border-[#a58a5e]"
            />
            <div className="mt-3 flex gap-3">
              <button
                type="button"
                onClick={onManualTokenSave}
                className="flex-1 rounded-2xl border border-[#cfbfaa] bg-white py-3 text-sm font-medium text-slate-700 active:bg-[#fffaf2]"
              >
                {copy.saveToken}
              </button>
              <button
                type="button"
                onClick={onClearSavedToken}
                className="flex-1 rounded-2xl border border-[#eadccb] py-3 text-sm font-medium text-slate-500 active:bg-[#f4ecdf]"
              >
                {copy.clearSavedToken}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Server info */}
      <div className="mt-6 rounded-2xl border border-dashed border-[#d4c4ac] bg-white/60 px-4 py-3 text-xs leading-5 text-slate-500">
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
      <div className="flex shrink-0 items-center justify-between border-b border-[#e5ddcf] px-4 py-3">
        <span className="text-sm font-semibold text-slate-900">{copy.refreshSessions}</span>
        <button
          type="button"
          onClick={onRefresh}
          className="rounded-xl border border-[#ddd4c5] bg-white px-3 py-2 text-xs font-medium text-slate-600 active:bg-[#faf6ef]"
        >
          {copy.refreshSessions}
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {sessionsLoading ? (
          <div className="flex items-center justify-center py-12">
            <LoaderCircle size={20} className="animate-spin text-slate-400" />
          </div>
        ) : sessions.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <div className="text-lg font-semibold text-slate-900">{copy.noSessionsTitle}</div>
            <div className="mt-3 text-sm leading-6 text-slate-500">{copy.noSessionsDescription}</div>
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
                      : 'border-[#e5ddcf] bg-white active:bg-[#faf6ef]'
                  }`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 text-sm font-semibold text-slate-900 truncate">
                      {resolveRemoteSessionTitle(session)}
                    </div>
                    <StatePill state={session.state} copy={copy} />
                  </div>
                  <div className="mt-2 flex items-center gap-2 text-xs text-slate-500">
                    <span>{formatRemoteRelativeTime(session.updated_at, locale, copy)}</span>
                    {session.metadata.agent_type && (
                      <>
                        <span>·</span>
                        <span className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[10px] uppercase text-slate-600">
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
          <div className="text-lg font-semibold text-slate-900">{copy.pickSessionTitle}</div>
          <div className="mt-3 text-sm leading-6 text-slate-500">{copy.pickSessionDescription}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Timeline */}
      <div className="flex-1 min-h-0 bg-[#f7f2e8]">
        {eventsLoading ? (
          <div className="flex items-center justify-center py-16">
            <LoaderCircle size={20} className="animate-spin text-slate-400" />
          </div>
        ) : events.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center px-6">
            <div className="text-lg font-semibold text-slate-900">{copy.timelineEmptyTitle}</div>
            <div className="mt-3 text-sm leading-6 text-slate-500">{copy.timelineEmptyDescription}</div>
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
      <div className="shrink-0 border-t border-[#e5ddcf] bg-[#f5efe4] px-3 py-3 pb-[calc(0.75rem+env(safe-area-inset-bottom))]">
        {controlStatus.notice && (
          <div className="mb-2 rounded-xl bg-[#fff7e8] px-3 py-2 text-xs leading-5 text-[#845612]">
            {controlStatus.notice}
          </div>
        )}
        <div className="flex items-end gap-2">
          <textarea
            aria-label={copy.followUpPlaceholder}
            value={composer}
            onChange={(e) => onComposerChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); onSend(); }
            }}
            rows={1}
            disabled={!controlStatus.canSendPrompt}
            placeholder={copy.followUpPlaceholder}
            className="min-h-[44px] max-h-[120px] flex-1 resize-none rounded-2xl border border-[#ded4c4] bg-white px-3 py-2.5 text-sm text-slate-800 outline-none placeholder:text-slate-400 focus:border-[#a58a5e] disabled:opacity-50"
          />
          {controlStatus.canInterrupt && (
            <button
              type="button"
              onClick={onInterrupt}
              disabled={interrupting}
              className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-[#dccfc0] bg-[#faf6ef] text-slate-700 active:bg-[#f3ebdf] disabled:opacity-50"
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
        <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
          <Shield size={16} />
          {copy.pendingApprovals}
          {pendingApprovals.length > 0 && (
            <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-[#fbf3df] px-1.5 text-[11px] font-bold text-[#7c5d12]">
              {pendingApprovals.length}
            </span>
          )}
        </div>

        {pendingApprovals.length === 0 ? (
          <div className="mt-4 rounded-2xl border border-[#e5ddcf] bg-white px-4 py-6 text-center text-sm text-slate-500">
            {copy.noPendingApprovals}
          </div>
        ) : (
          <div className="mt-3 space-y-3">
            {pendingApprovals.map((approval) => (
              <div key={approval.approval_id} className="rounded-2xl border border-[#ead9b7] bg-white p-4">
                <div className="text-sm font-semibold text-slate-900">{approval.title}</div>
                {approval.description && (
                  <div className="mt-1 text-xs leading-5 text-slate-500">{approval.description}</div>
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
        <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
          <FileOutput size={16} />
          {copy.artifacts}
        </div>

        {artifacts.length === 0 ? (
          <div className="mt-4 rounded-2xl border border-[#e5ddcf] bg-white px-4 py-6 text-center text-sm text-slate-500">
            {copy.noArtifacts}
          </div>
        ) : (
          <div className="mt-3 space-y-2">
            {artifacts.map((artifact) => (
              <div key={artifact.artifact_id} className="flex items-center justify-between rounded-2xl border border-[#e5ddcf] bg-white px-4 py-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-slate-900">{artifact.file_name}</div>
                  <div className="mt-0.5 text-xs text-slate-500">{formatBytes(artifact.size_bytes)}</div>
                </div>
                <div className="flex shrink-0 gap-2">
                  <button
                    type="button"
                    onClick={() => onDownload(artifact)}
                    disabled={downloadingArtifactId === artifact.artifact_id}
                    className="rounded-xl border border-[#ddd4c5] bg-[#faf6ef] px-3 py-2 text-xs font-medium text-slate-700 active:bg-white disabled:opacity-50"
                  >
                    {downloadingArtifactId === artifact.artifact_id ? <LoaderCircle size={12} className="animate-spin" /> : copy.renderResponse}
                  </button>
                  <button
                    type="button"
                    onClick={() => onShare(artifact)}
                    className="rounded-xl border border-[#ddd4c5] bg-[#faf6ef] px-3 py-2 text-xs font-medium text-slate-700 active:bg-white"
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
      onClick={onClick}
      className={`relative flex flex-1 flex-col items-center gap-1 py-2 text-[11px] font-medium transition-colors ${
        active ? 'text-[#1d6b45]' : 'text-slate-400 active:text-slate-600'
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
    : 'bg-[#f6f1eb] text-slate-600';

  return (
    <span className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium ${className}`}>
      {copy.sessionStateLabels[state]}
    </span>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Timeline rendering (reused from RemoteApp)
// ═══════════════════════════════════════════════════════════════════════════

function TimelineCard({
  copy,
  event,
  locale,
}: {
  copy: ReturnType<typeof getRemoteCopy>;
  event: RemoteTimelineEvent;
  locale: ReturnType<typeof resolveRemoteLocale>;
}) {
  const { detail } = event;
  const ts = formatRemoteRelativeTime(event.recorded_at, locale, copy);

  if (detail.kind === 'message_committed') {
    return (
      <TimelineMessageCard role={detail.role} header={copy.messageHeaders[detail.role]}>
        {detail.role === 'assistant' ? (
          <Suspense fallback={<div className="space-y-2"><div className="h-4 w-3/4 animate-pulse rounded bg-slate-200" /><div className="h-4 w-1/2 animate-pulse rounded bg-slate-200" /></div>}>
            <LazyMarkdownRenderer content={detail.text} />
          </Suspense>
        ) : (
          <div className="whitespace-pre-wrap break-words text-[15px] leading-7 text-slate-800">{detail.text}</div>
        )}
      </TimelineMessageCard>
    );
  }

  if (detail.kind === 'message_delta') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.streaming} accent="text-amber-700" icon={<LoaderCircle size={16} className="animate-spin" />} timestampLabel={ts}>
        <div className="whitespace-pre-wrap break-words text-sm leading-6 text-slate-700">{detail.delta}</div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'tool_started' || detail.kind === 'tool_finished' || detail.kind === 'tool_progress') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.tool} accent={detail.kind === 'tool_finished' && detail.is_error ? 'text-rose-700' : 'text-emerald-700'} icon={<Bot size={16} />} timestampLabel={ts}>
        <div className="space-y-2 text-sm text-slate-700">
          <div className="font-medium text-slate-900">{toolLabel(detail)}</div>
          <div className="rounded-2xl bg-[#f7f2e8] px-3 py-2 text-sm leading-6 text-slate-600">{toolSummary(detail, copy)}</div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'approval_requested' || detail.kind === 'approval_resolved') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.approval} accent="text-[#7f4f19]" icon={<Shield size={16} />} timestampLabel={ts}>
        <div className="space-y-2 text-sm leading-6 text-slate-700">
          <div className="font-medium text-slate-900">{approvalSummary(detail, copy)}</div>
          {'responder' in detail && detail.responder && (
            <div className="text-xs uppercase tracking-[0.18em] text-slate-400">{copy.responderLabel}: {detail.responder}</div>
          )}
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'artifact_created' || detail.kind === 'artifact_manifest') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.artifact} accent="text-sky-700" icon={<FileOutput size={16} />} timestampLabel={ts}>
        <div className="text-sm leading-6 text-slate-700">{artifactSummary(detail, copy)}</div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'runtime_error') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.runtime} accent="text-rose-700" icon={<AlertTriangle size={16} />} timestampLabel={ts}>
        <div className="text-sm leading-6 text-rose-700">{detail.message}</div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'daemon_presence_changed') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.daemon} accent="text-slate-700" icon={detail.state === 'online' ? <Wifi size={16} /> : <WifiOff size={16} />} timestampLabel={ts}>
        <div className="text-sm text-slate-700">{copy.daemonNow(copy.daemonStates[detail.state])}</div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'subtask_started' || detail.kind === 'subtask_progress' || detail.kind === 'subtask_completed') {
    const stageLabel = detail.kind === 'subtask_started' ? 'started' : detail.kind === 'subtask_completed' ? 'completed' : 'progress';
    const desc = detail.kind === 'subtask_started' ? detail.description : detail.summary;
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.subtask} accent={stageLabel === 'completed' ? 'text-emerald-700' : 'text-violet-700'} icon={<GitBranch size={16} />} timestampLabel={ts}>
        <div className="space-y-1 text-sm text-slate-700">
          <div className="font-medium text-slate-900">{desc}</div>
          <div className="text-xs text-slate-400">{detail.task_id} · {stageLabel}{'turns_used' in detail && detail.turns_used != null ? ` · ${detail.turns_used} turns` : ''}</div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'batch_progress') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.batch} accent="text-blue-700" icon={<Layers size={16} />} timestampLabel={ts}>
        <div className="space-y-1 text-sm text-slate-700">
          <div className="font-medium text-slate-900">{detail.completed}/{detail.total} completed</div>
          {detail.running > 0 && <div className="text-xs text-slate-400">{detail.running} running</div>}
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'context_usage' || detail.kind === 'context_overflow') {
    const pct = Math.round(detail.ratio * 100);
    const isOverflow = detail.kind === 'context_overflow';
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.context} accent={isOverflow ? 'text-amber-700' : 'text-slate-700'} icon={<Database size={16} />} timestampLabel={ts}>
        <div className="space-y-1 text-sm text-slate-700">
          <div className="font-medium text-slate-900">{isOverflow ? 'Context overflow' : 'Context usage'}: {pct}%</div>
          <div className="text-xs text-slate-400">{detail.estimated_tokens.toLocaleString()} / {detail.max_input_tokens.toLocaleString()} tokens</div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'context_compacted') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.context} accent="text-slate-700" icon={<Database size={16} />} timestampLabel={ts}>
        <div className="space-y-1 text-sm text-slate-700">
          <div className="font-medium text-slate-900">Context compacted</div>
          <div className="text-xs text-slate-400">{detail.entries_removed} entries removed · ratio {detail.usage_ratio.toFixed(2)}</div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'session_created' || detail.kind === 'session_state_changed') {
    return (
      <TimelineEventCard eyebrow={copy.eventEyebrows.session} accent="text-slate-700" icon={<MessageSquareText size={16} />} timestampLabel={ts}>
        <div className="text-sm text-slate-700">{sessionEventSummary(detail, copy)}</div>
      </TimelineEventCard>
    );
  }

  return (
    <TimelineEventCard eyebrow={copy.eventEyebrows.runner} accent="text-slate-700" icon={<Bot size={16} />} timestampLabel={ts}>
      <div className="text-sm text-slate-700">{runnerEventSummary(detail, copy)}</div>
    </TimelineEventCard>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Pure helpers (same logic as RemoteApp)
// ═══════════════════════════════════════════════════════════════════════════

function describeSessionControl(
  session: RemoteSessionRecord | null,
  locale: ReturnType<typeof resolveRemoteLocale>,
  copy: ReturnType<typeof getRemoteCopy>,
): { canSendPrompt: boolean; canInterrupt: boolean; notice: string | null } {
  if (!session) return { canSendPrompt: false, canInterrupt: false, notice: null };
  if (!session.owner_runner_id) return { canSendPrompt: false, canInterrupt: false, notice: copy.controlUnavailableUnassigned };
  if (session.owner_runner_available === false) {
    return {
      canSendPrompt: false,
      canInterrupt: false,
      notice: copy.controlUnavailableRunnerOffline(
        session.owner_runner_id,
        session.owner_runner_last_seen_at ? formatRemoteRelativeTime(session.owner_runner_last_seen_at, locale, copy) : null,
      ),
    };
  }
  return { canSendPrompt: true, canInterrupt: true, notice: null };
}

function toolLabel(detail: Extract<RemoteTimelineEventDetail, { kind: 'tool_started' }> | Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }> | Extract<RemoteTimelineEventDetail, { kind: 'tool_finished' }>): string {
  if ('tool_name' in detail && detail.tool_name) return detail.tool_name;
  return detail.tool_call_id ?? 'tool';
}

function toolSummary(
  detail: Extract<RemoteTimelineEventDetail, { kind: 'tool_started' }> | Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }> | Extract<RemoteTimelineEventDetail, { kind: 'tool_finished' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'tool_started') return copy.toolStarted(detail.tool_call_id);
  if (detail.kind === 'tool_progress') {
    if (detail.delta) return detail.delta;
    if (detail.elapsed_time_seconds != null) return copy.toolElapsed(detail.elapsed_time_seconds);
    return copy.toolRunning;
  }
  if (detail.summary) return detail.summary;
  return detail.is_error ? copy.toolFailedWithoutSummary : copy.toolCompleted;
}

function approvalSummary(
  detail: Extract<RemoteTimelineEventDetail, { kind: 'approval_requested' }> | Extract<RemoteTimelineEventDetail, { kind: 'approval_resolved' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'approval_requested') return copy.approvalWaiting(detail.title);
  return copy.approvalResolved(detail.approval_id, copy.approvalStateLabels[detail.state]);
}

function artifactSummary(
  detail: Extract<RemoteTimelineEventDetail, { kind: 'artifact_created' }> | Extract<RemoteTimelineEventDetail, { kind: 'artifact_manifest' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'artifact_created') return copy.artifactCreated(detail.name, detail.file_name, formatBytes(detail.size_bytes));
  return copy.artifactManifest(detail.artifact_ids.length);
}

function sessionEventSummary(
  detail: Extract<RemoteTimelineEventDetail, { kind: 'session_created' }> | Extract<RemoteTimelineEventDetail, { kind: 'session_state_changed' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'session_created') return copy.sessionCreated(detail.workspace_id);
  return copy.sessionMoved(copy.sessionStateLabels[detail.previous_state], copy.sessionStateLabels[detail.state]);
}

function runnerEventSummary(
  detail: Extract<RemoteTimelineEventDetail, { kind: 'runner_registered' }> | Extract<RemoteTimelineEventDetail, { kind: 'runner_heartbeat' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'runner_registered') return copy.runnerRegistered(detail.workspace_ids.length, detail.lease_ttl_secs);
  return copy.runnerHeartbeat(detail.active_sessions, detail.queued_sessions);
}

function extractErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  return String(error);
}
