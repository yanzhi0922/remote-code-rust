import * as Dialog from '@radix-ui/react-dialog';
import {
  AlertTriangle,
  Bot,
  Database,
  FileOutput,
  Layers,
  GitBranch,
  LoaderCircle,
  MessageSquareText,
  Shield,
  Square,
  Wifi,
  WifiOff,
  X,
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
import { ApprovalPanel } from '../components/shared/ApprovalPanel';
import { ArtifactPanel } from '../components/shared/ArtifactPanel';
import { formatBytes } from '../components/shared/formatBytes';
import { TimelineEventCard } from '../components/shared/TimelineEventCard';
import { TimelineMessageCard } from '../components/shared/TimelineMessageCard';
import {
  acceptPairingOffer,
  buildArtifactDownloadUrl,
  bootstrapControlPlane,
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
import {
  appendRemoteTimelineEvent,
} from '../session/normalize/fromRemote';
import { RemoteAuthGate } from './RemoteAuthGate';
import { RemoteShell, EmptyCard } from './RemoteShell';
import { loadRemoteSessionBundle } from './transport';
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

type ConnectionState = RemoteConnectionState;

const LazyMarkdownRenderer = lazy(() => import('../components/chat/MarkdownRenderer'));

const APPROVAL_DECISIONS: Array<{
  decision: RemoteApprovalDecision;
  className: string;
}> = [
  {
    decision: 'approved',
    className: 'bg-[#1d6b45] text-white hover:bg-[#145033]',
  },
  {
    decision: 'denied',
    className: 'bg-[#a13a30] text-white hover:bg-[#7e2b24]',
  },
  {
    decision: 'cancelled',
    className: 'bg-[#efe7db] text-slate-700 hover:bg-[#e6dccd]',
  },
];

// ═══════════════════════════════════════════════════════════════════════════
// RemoteApp — thin orchestrator
// ═══════════════════════════════════════════════════════════════════════════

export default function RemoteApp() {
  const baseUrl = resolveRemoteBaseUrl();
  const locale = useMemo(() => resolveRemoteLocale(), []);
  const copy = useMemo(() => getRemoteCopy(locale), [locale]);
  const initialPairingContext = resolveRemotePairingContext();
  const [accessToken, setAccessToken] = useState<string | null>(() => resolveRemoteAccessToken());
  const [health, setHealth] = useState<RemoteControlPlaneHealth | null>(null);
  const [authLoading, setAuthLoading] = useState(false);
  const [authErrorMessage, setAuthErrorMessage] = useState<string | null>(null);
  const [manualAccessToken, setManualAccessToken] = useState('');
  const [bootstrapSecret, setBootstrapSecret] = useState('');
  const [deviceName, setDeviceName] = useState('Mobile Browser');
  const [pairingOfferId, setPairingOfferId] = useState(initialPairingContext.offerId ?? '');
  const [pairingSecret, setPairingSecret] = useState(initialPairingContext.pairingSecret ?? '');
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
  const [composer, setComposer] = useState('');
  const [sending, setSending] = useState(false);
  const [interrupting, setInterrupting] = useState(false);
  const [approvingId, setApprovingId] = useState<string | null>(null);
  const [downloadingArtifactId, setDownloadingArtifactId] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const activeSessionIdRef = useRef<string | null>(null);
  const latestSequenceRef = useRef(0);
  const connectedSessionRef = useRef<string | null>(null);
  const sessionRefreshTimerRef = useRef<number | null>(null);
  const statusTimerRef = useRef<number | null>(null);

  const activeSession = useMemo(
    () => sessions.find((session) => session.session_id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );
  const selectedSessionId = activeSession?.session_id ?? null;
  useEffect(() => {
    activeSessionIdRef.current = selectedSessionId;
  });
  const activeSessionControlStatus = useMemo(
    () => describeSessionControl(activeSession, locale, copy),
    [activeSession, copy, locale],
  );
  const pendingApprovals = useMemo(
    () => approvals.filter((approval) => approval.state === 'pending'),
    [approvals],
  );
  const approvalActions = useMemo(
    () =>
      APPROVAL_DECISIONS.map((item) => ({
        ...item,
        label: copy.approvalDecisionLabels[item.decision],
      })),
    [copy],
  );
  const authRequired = health?.auth_required ?? false;
  const showAuthGate = Boolean(baseUrl) && ((authRequired && !accessToken) || authErrorMessage);

  // ── Utility callbacks ──────────────────────────────────────────────────

  const showStatusMessage = useEffectEvent((message: string) => {
    setStatusMessage(message);
    if (statusTimerRef.current !== null) {
      window.clearTimeout(statusTimerRef.current);
    }
    statusTimerRef.current = window.setTimeout(() => {
      setStatusMessage(null);
      statusTimerRef.current = null;
    }, 3000);
  });

  const reportAsyncError = useEffectEvent((error: unknown) => {
    const message = extractErrorMessage(error);
    if (message.includes('HTTP 401')) {
      setAuthErrorMessage(message);
    } else {
      setErrorMessage(message);
    }
  });

  const scheduleSessionsRefresh = useEffectEvent(() => {
    if (sessionRefreshTimerRef.current !== null) {
      return;
    }
    sessionRefreshTimerRef.current = window.setTimeout(() => {
      sessionRefreshTimerRef.current = null;
      void refreshSessions().catch(reportAsyncError);
    }, 350);
  });

  // ── Locale ─────────────────────────────────────────────────────────────

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  // ── Active session persistence ─────────────────────────────────────────

  useEffect(() => {
    if (selectedSessionId) {
      persistRemoteActiveSessionId(baseUrl, selectedSessionId);
      return;
    }
    clearRemoteActiveSessionId(baseUrl);
  }, [baseUrl, selectedSessionId]);

  // ── Health check ───────────────────────────────────────────────────────

  useEffect(() => {
    if (!baseUrl) {
      return;
    }
    let cancelled = false;
    void getControlPlaneHealth(baseUrl)
      .then((response) => {
        if (!cancelled) {
          setHealth(response);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          reportAsyncError(error);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [baseUrl, accessToken]);

  // ── Auth handlers ──────────────────────────────────────────────────────

  const completeAuthentication = useEffectEvent((token: string, message: string, refreshToken?: string) => {
    persistRemoteAccessToken(token);
    if (refreshToken) {
      persistRemoteRefreshToken(refreshToken);
    }
    void clearRemotePairingContext();
    stripRemoteSensitiveQueryParams();
    setBootstrapSecret('');
    setPairingOfferId('');
    setPairingSecret('');
    setManualAccessToken('');
    setAccessToken(token);
    setAuthErrorMessage(null);
    setErrorMessage(null);
    showStatusMessage(message);
  });

  const handleBootstrapClaim = useEffectEvent(async () => {
    if (!baseUrl || authLoading) {
      return;
    }
    setAuthLoading(true);
    try {
      const response = await bootstrapControlPlane(baseUrl, bootstrapSecret, deviceName);
      completeAuthentication(response.access_token, copy.statusBootstrapClaimSucceeded, response.refresh_token);
      const nextHealth = await getControlPlaneHealth(baseUrl);
      setHealth(nextHealth);
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setAuthLoading(false);
    }
  });

  const handlePairingAccept = useEffectEvent(async () => {
    if (!baseUrl || authLoading) {
      return;
    }
    setAuthLoading(true);
    try {
      const response = await acceptPairingOffer(
        baseUrl,
        pairingOfferId,
        pairingSecret,
        deviceName,
      );
      completeAuthentication(response.access_token, copy.statusPairingSucceeded, response.refresh_token);
      const nextHealth = await getControlPlaneHealth(baseUrl);
      setHealth(nextHealth);
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setAuthLoading(false);
    }
  });

  const handleManualTokenSave = useEffectEvent(() => {
    if (!manualAccessToken.trim()) {
      return;
    }
    persistRemoteAccessToken(manualAccessToken);
    void clearRemotePairingContext();
    stripRemoteSensitiveQueryParams();
    setBootstrapSecret('');
    setPairingOfferId('');
    setPairingSecret('');
    setManualAccessToken('');
    setAccessToken(manualAccessToken.trim());
    setAuthErrorMessage(null);
    showStatusMessage(copy.statusSavedAccessToken);
  });

  const handleClearSavedToken = useEffectEvent(() => {
    clearRemoteAccessToken();
    clearRemoteActiveSessionId(baseUrl);
    void clearRemotePairingContext();
    stripRemoteSensitiveQueryParams();
    setBootstrapSecret('');
    setPairingOfferId('');
    setPairingSecret('');
    setAccessToken(null);
    setManualAccessToken('');
    setAuthErrorMessage(null);
    showStatusMessage(copy.statusClearedAccessToken);
  });

  // ── Data fetching ──────────────────────────────────────────────────────

  const refreshSessions = useEffectEvent(async () => {
    if (!baseUrl || !health || (authRequired && !accessToken)) {
      return;
    }
    setSessionsLoading((current) => (sessions.length === 0 ? true : current));
    try {
      const response = await listSessions(baseUrl);
      const nextSessions = [...response.items].sort(
        (left, right) =>
          new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime(),
      );
      setSessions(nextSessions);
      setActiveSessionId((current) => {
        if (current && nextSessions.some((session) => session.session_id === current)) {
          return current;
        }
        const stored = resolveRemoteActiveSessionId(baseUrl);
        if (stored && nextSessions.some((session) => session.session_id === stored)) {
          return stored;
        }
        return nextSessions[0]?.session_id ?? null;
      });
      setErrorMessage(null);
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setSessionsLoading(false);
    }
  });

  const refreshApprovals = useEffectEvent(async (sessionId: string) => {
    if (!baseUrl || !health || (authRequired && !accessToken)) {
      return;
    }
    const response = await listSessionApprovals(baseUrl, sessionId);
    if (activeSessionIdRef.current !== sessionId) {
      return;
    }
    setApprovals(
      [...response.items].sort(
        (left, right) =>
          new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime(),
      ),
    );
  });

  const refreshArtifacts = useEffectEvent(async (sessionId: string) => {
    if (!baseUrl || !health || (authRequired && !accessToken)) {
      return;
    }
    const response = await listSessionArtifacts(baseUrl, sessionId);
    if (activeSessionIdRef.current !== sessionId) {
      return;
    }
    setArtifacts(
      [...response.items].sort(
        (left, right) =>
          new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
      ),
    );
  });

  const refreshSessionBundle = useEffectEvent(async (sessionId: string) => {
    if (!baseUrl || !health || (authRequired && !accessToken)) {
      return;
    }

    const bundle = await loadRemoteSessionBundle(baseUrl, sessionId);

    if (activeSessionIdRef.current !== sessionId) {
      return;
    }

    latestSequenceRef.current = bundle.latestSequence;

    startTransition(() => {
      setEvents(bundle.events);
    });
    setApprovals(bundle.approvals);
    setArtifacts(bundle.artifacts);
  });

  const handleLiveEvent = useEffectEvent((sessionId: string, event: RemoteTimelineEvent) => {
    if (activeSessionIdRef.current !== sessionId) {
      return;
    }

    latestSequenceRef.current = Math.max(latestSequenceRef.current, event.sequence);
    startTransition(() => {
      setEvents((current) => appendRemoteTimelineEvent(current, event));
    });

    if (event.detail.kind === 'approval_requested' || event.detail.kind === 'approval_resolved') {
      void refreshApprovals(sessionId).catch(reportAsyncError);
    }
    if (event.detail.kind === 'artifact_created' || event.detail.kind === 'artifact_manifest') {
      void refreshArtifacts(sessionId).catch(reportAsyncError);
    }
    if (
      event.detail.kind === 'approval_requested' ||
      event.detail.kind === 'approval_resolved' ||
      event.detail.kind === 'daemon_presence_changed'
    ) {
      scheduleSessionsRefresh();
    }
    if (
      event.detail.kind === 'session_state_changed' &&
      event.detail.previous_state !== event.detail.state
    ) {
      scheduleSessionsRefresh();
    }
  });

  // ── Unified transport via ConnectionManager ──────────────────────────────

  const handleTransportEvent = useEffectEvent((event: RemoteTimelineEvent) => {
    const sessionId = connectedSessionRef.current;
    if (sessionId) {
      handleLiveEvent(sessionId, event);
    }
  });

  const {
    connectionState: transportConnectionState,
    connect: transportConnect,
    disconnect: transportDisconnect,
    latestSequence: transportSequence,
  } = useConnection(handleTransportEvent);

  const connectionState: ConnectionState =
    transportConnectionState === 'probing' ? 'connecting' : transportConnectionState;

  useEffect(() => {
    latestSequenceRef.current = Math.max(latestSequenceRef.current, transportSequence);
  }, [transportSequence]);

  // ── Periodic + visibility refresh ──────────────────────────────────────

  useEffect(() => {
    void refreshSessions();
    const intervalId = window.setInterval(() => {
      void refreshSessions();
    }, 15_000);
    return () => {
      window.clearInterval(intervalId);
    };
  }, [accessToken, baseUrl, health]);

  useEffect(() => {
    const onVisibilityChange = () => {
      if (document.visibilityState !== 'visible') {
        return;
      }
      void refreshSessions().catch(reportAsyncError);
      if (activeSessionIdRef.current) {
        void refreshSessionBundle(activeSessionIdRef.current).catch(reportAsyncError);
      }
    };

    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, []);

  useEffect(() => {
    const onOnline = () => {
      void refreshSessions().catch(reportAsyncError);
      if (activeSessionIdRef.current) {
        void refreshSessionBundle(activeSessionIdRef.current).catch(reportAsyncError);
      }
    };

    window.addEventListener('online', onOnline);
    return () => {
      window.removeEventListener('online', onOnline);
    };
  }, []);

  useEffect(() => {
    return () => {
      if (sessionRefreshTimerRef.current !== null) {
        window.clearTimeout(sessionRefreshTimerRef.current);
      }
      if (statusTimerRef.current !== null) {
        window.clearTimeout(statusTimerRef.current);
      }
    };
  }, []);

  // ── Transport subscription ─────────────────────────────────────────────

  useEffect(() => {
    if (!baseUrl || !selectedSessionId || !health || (authRequired && !accessToken)) {
      setEvents([]);
      setApprovals([]);
      setArtifacts([]);
      connectedSessionRef.current = null;
      transportDisconnect();
      latestSequenceRef.current = 0;
      return;
    }

    connectedSessionRef.current = selectedSessionId;

    let cancelled = false;

    const bootstrap = async () => {
      setEventsLoading(true);
      try {
        await refreshSessionBundle(selectedSessionId);
        if (!cancelled) {
          const config: TransportConfig = {
            strategy: 'hybrid',
            baseUrl,
            runnerBaseUrl: activeSession?.owner_runner_public_base_url ?? null,
            sessionId: selectedSessionId,
            authToken: accessToken,
          };
          await transportConnect(config, latestSequenceRef.current);
          setErrorMessage(null);
        }
      } catch (error) {
        if (!cancelled) {
          reportAsyncError(error);
        }
      } finally {
        if (!cancelled) {
          setEventsLoading(false);
        }
      }
    };

    void bootstrap();

    return () => {
      cancelled = true;
      connectedSessionRef.current = null;
      transportDisconnect();
    };
  }, [accessToken, authRequired, baseUrl, health, selectedSessionId]);

  // ── Action handlers ────────────────────────────────────────────────────

  const handleSendPrompt = async () => {
    if (
      !baseUrl ||
      !selectedSessionId ||
      (authRequired && !accessToken) ||
      !composer.trim() ||
      sending ||
      !activeSessionControlStatus.canSendPrompt
    ) {
      return;
    }

    setSending(true);
    try {
      await sendPrompt(baseUrl, selectedSessionId, composer.trim(), activeSession?.owner_runner_public_base_url);
      setComposer('');
      showStatusMessage(copy.statusPromptForwarded);
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setSending(false);
    }
  };

  const handleInterrupt = async () => {
    if (
      !baseUrl ||
      !selectedSessionId ||
      (authRequired && !accessToken) ||
      interrupting ||
      !activeSessionControlStatus.canInterrupt
    ) {
      return;
    }

    setInterrupting(true);
    try {
      await interruptSession(baseUrl, selectedSessionId, activeSession?.owner_runner_public_base_url);
      showStatusMessage(copy.statusInterruptForwarded);
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setInterrupting(false);
    }
  };

  const handleApprovalDecision = async (
    approvalId: string,
    decision: RemoteApprovalDecision,
  ) => {
    if (!baseUrl || (authRequired && !accessToken) || approvingId) {
      return;
    }

    setApprovingId(approvalId);
    try {
      await respondToApproval(baseUrl, approvalId, decision, undefined, activeSession?.owner_runner_public_base_url);
      showStatusMessage(copy.statusApprovalDecision(copy.approvalDecisionLabels[decision]));
      if (selectedSessionId) {
        await refreshApprovals(selectedSessionId);
      }
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setApprovingId(null);
    }
  };

  const handleArtifactDownload = async (artifact: RemoteArtifactRecord) => {
    if (!baseUrl || (authRequired && !accessToken) || downloadingArtifactId) {
      return;
    }

    setDownloadingArtifactId(artifact.artifact_id);
    try {
      await downloadRemoteArtifact({
        url: buildArtifactDownloadUrl(baseUrl, artifact.artifact_id),
        fileName: artifact.file_name,
        token: accessToken,
      });
      showStatusMessage(copy.statusArtifactDownloaded(artifact.file_name));
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setDownloadingArtifactId(null);
    }
  };

  // ── Render: early exits ────────────────────────────────────────────────

  if (!baseUrl) {
    return (
      <div className="min-h-screen bg-[radial-gradient(circle_at_top_left,#fbf6ec,transparent_28%),linear-gradient(180deg,#f4efe4_0%,#efe8db_100%)] text-slate-900">
        <div className="flex min-h-screen items-center justify-center px-6">
          <EmptyCard
            title={copy.remoteModeNotConfiguredTitle}
            description={copy.remoteModeNotConfiguredDescription}
          />
        </div>
      </div>
    );
  }

  if (!health) {
    return (
      <div className="min-h-screen bg-[radial-gradient(circle_at_top_left,#fbf6ec,transparent_28%),linear-gradient(180deg,#f4efe4_0%,#efe8db_100%)] text-slate-900">
        <div className="flex min-h-screen items-center justify-center px-6">
          <div role="status" className="flex items-center gap-3 rounded-2xl border border-[#e2d8c8] bg-white px-5 py-4 text-sm text-slate-600 shadow-[0_18px_45px_rgba(52,45,34,0.08)]">
            <LoaderCircle size={16} className="animate-spin" />
            {copy.contactingControlPlane}
          </div>
        </div>
      </div>
    );
  }

  if (showAuthGate) {
    return (
      <div className="min-h-screen bg-[radial-gradient(circle_at_top_left,#fbf6ec,transparent_28%),linear-gradient(180deg,#f4efe4_0%,#efe8db_100%)] text-slate-900">
        <RemoteAuthGate
          authErrorMessage={authErrorMessage}
          authLoading={authLoading}
          bootstrapEnabled={!health.owner_claimed && health.bootstrap_secret_configured}
          copy={copy}
          deviceName={deviceName}
          health={health}
          manualAccessToken={manualAccessToken}
          onBootstrapClaim={() => {
            void handleBootstrapClaim();
          }}
          onClearSavedToken={() => {
            handleClearSavedToken();
          }}
          onManualTokenSave={handleManualTokenSave}
          onPairingAccept={() => {
            void handlePairingAccept();
          }}
          pairingOfferId={pairingOfferId}
          pairingSecret={pairingSecret}
          setBootstrapSecret={setBootstrapSecret}
          setDeviceName={setDeviceName}
          setManualAccessToken={setManualAccessToken}
          setPairingOfferId={setPairingOfferId}
          setPairingSecret={setPairingSecret}
        />
      </div>
    );
  }

  // ── Render: main shell ─────────────────────────────────────────────────

  return (
    <RemoteShell
      sessions={sessions}
      sessionsLoading={sessionsLoading}
      activeSessionId={selectedSessionId}
      activeSession={activeSession}
      connectionState={connectionState}
      sidebarOpen={sidebarOpen}
      errorMessage={errorMessage}
      statusMessage={statusMessage}
      baseUrl={baseUrl}
      copy={copy}
      locale={locale}
      onToggleSidebar={setSidebarOpen}
      onSelectSession={(id) => {
        setActiveSessionId(id);
        setSidebarOpen(false);
      }}
      onRefreshSessions={() => {
        void refreshSessions();
      }}
    >
      <main className="grid min-h-0 flex-1 gap-0 lg:grid-cols-[minmax(0,1fr)_340px]">
        {/* ── Timeline + Composer ── */}
        <section className="flex min-h-0 flex-col border-b border-[#e5ddcf] bg-[#f7f2e8] lg:border-b-0 lg:border-r">
          {activeSession ? (
            <div className="flex min-h-0 flex-1 flex-col">
              <div className="flex-1 min-h-0">
                {eventsLoading ? (
                  <div className="flex min-h-[280px] items-center justify-center px-4 py-5 sm:px-6">
                    <div role="status" className="flex items-center gap-3 rounded-2xl bg-white px-4 py-3 text-sm text-slate-500 shadow-[0_12px_28px_rgba(34,32,28,0.06)]">
                      <LoaderCircle size={16} className="animate-spin" />
                      {copy.loadingSessionTimeline}
                    </div>
                  </div>
                ) : deferredEvents.length === 0 ? (
                  <div className="flex min-h-[280px] items-center justify-center px-4 py-5 sm:px-6">
                    <EmptyCard
                      title={copy.timelineEmptyTitle}
                      description={copy.timelineEmptyDescription}
                    />
                  </div>
                ) : (
                  <Virtuoso
                    data={deferredEvents}
                    followOutput="smooth"
                    className="h-full"
                    itemContent={(index, event) => (
                      <div className="px-4 py-2.5 sm:px-6 first:pt-5 last:pb-5">
                        <div className="mx-auto max-w-5xl">
                          <TimelineCard copy={copy} locale={locale} event={event} />
                        </div>
                      </div>
                    )}
                  />
                )}
              </div>

              <div className="border-t border-[#e5ddcf] bg-[#f5efe4] px-4 py-4 sm:px-6">
                <div className="mx-auto max-w-5xl rounded-[28px] border border-[#ded4c4] bg-white shadow-[0_18px_44px_rgba(34,32,28,0.09)]">
                  <div className="flex items-center justify-between gap-3 border-b border-[#efe6d9] px-4 py-3">
                    <div className="inline-flex items-center gap-2 text-sm text-slate-600">
                      <MessageSquareText size={15} />
                      {copy.followUpControl}
                    </div>
                    <button
                      type="button"
                      onClick={() => {
                        void handleInterrupt();
                      }}
                      disabled={interrupting || !activeSessionControlStatus.canInterrupt}
                      className="inline-flex items-center gap-2 rounded-full border border-[#dccfc0] bg-[#faf6ef] px-3 py-1.5 text-sm text-slate-700 transition-colors hover:bg-[#f3ebdf] disabled:cursor-not-allowed disabled:opacity-60"
                    >
                      {interrupting ? (
                        <LoaderCircle size={14} className="animate-spin" />
                      ) : (
                        <Square size={14} />
                      )}
                      {interrupting ? copy.interrupting : copy.interrupt}
                    </button>
                  </div>
                  {activeSessionControlStatus.notice && (
                    <div className="border-b border-[#f0dfbf] bg-[#fff7e8] px-4 py-3 text-sm leading-6 text-[#845612]">
                      {activeSessionControlStatus.notice}
                    </div>
                  )}
                  <div className="flex items-end gap-3 px-4 py-4">
                    <textarea
                      aria-label={copy.followUpPlaceholder}
                      value={composer}
                      onChange={(event) => setComposer(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' && !event.shiftKey) {
                          event.preventDefault();
                          void handleSendPrompt();
                        }
                      }}
                      rows={1}
                      disabled={!activeSessionControlStatus.canSendPrompt}
                      placeholder={copy.followUpPlaceholder}
                      className="min-h-[88px] flex-1 resize-none bg-transparent text-[15px] leading-6 text-slate-800 outline-none placeholder:text-slate-400 disabled:cursor-not-allowed disabled:text-slate-400"
                    />
                    <button
                      type="button"
                      onClick={() => {
                        void handleSendPrompt();
                      }}
                      disabled={
                        sending ||
                        composer.trim().length === 0 ||
                        !activeSessionControlStatus.canSendPrompt
                      }
                      className="inline-flex h-14 items-center justify-center rounded-2xl bg-[#17181a] px-5 text-sm font-medium text-white shadow-[0_10px_22px_rgba(23,24,26,0.2)] transition-colors hover:bg-[#282a2d] disabled:cursor-not-allowed disabled:bg-[#cbbfac] disabled:text-white/70"
                    >
                      {sending ? <LoaderCircle size={18} className="animate-spin" /> : copy.send}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div className="flex min-h-[420px] items-center justify-center px-6">
              <EmptyCard
                title={copy.pickSessionTitle}
                description={copy.pickSessionDescription}
              />
            </div>
          )}
        </section>

        {/* ── Desktop: Approvals + Artifacts side panel (always rendered; mobile also has floating FABs) ── */}
        <aside className="border-t border-[#e5ddcf] bg-[#f2ebdf] lg:border-t-0">
          <div className="grid gap-4 px-4 py-5 sm:px-6 lg:sticky lg:top-0">
            <ApprovalPanel
              title={copy.pendingApprovals}
              icon={<Shield size={16} />}
              emptyText={copy.noPendingApprovals}
              items={pendingApprovals}
              actions={approvalActions}
              approvingId={approvingId}
              loadingText={copy.loading}
              onDecision={(approvalId, decision) => {
                void handleApprovalDecision(approvalId, decision as RemoteApprovalDecision);
              }}
            />

            <ArtifactPanel
              title={copy.artifacts}
              icon={<FileOutput size={16} />}
              emptyText={copy.noArtifacts}
              items={artifacts}
              downloadingId={downloadingArtifactId}
              onDownload={(artifact) => {
                const record = artifacts.find((item) => item.artifact_id === artifact.artifact_id);
                if (!record) {
                  return;
                }
                void handleArtifactDownload(record);
              }}
            />
          </div>
        </aside>

        {/* ── Mobile: Floating action buttons + bottom sheets ── */}
        <div className="fixed bottom-6 right-4 z-40 flex flex-col gap-3 lg:hidden">
          {/* Approval bottom sheet */}
          <Dialog.Root>
            <Dialog.Trigger asChild>
              <button
                type="button"
                aria-label={copy.pendingApprovals}
                className="relative inline-flex h-14 w-14 items-center justify-center rounded-2xl bg-[#17181a] text-white shadow-[0_10px_22px_rgba(23,24,26,0.25)] transition-transform active:scale-95"
              >
                <Shield size={20} />
                {pendingApprovals.length > 0 && (
                  <span className="absolute -right-1 -top-1 flex h-5 min-w-5 items-center justify-center rounded-full bg-red-500 px-1 text-[11px] font-bold text-white">
                    {pendingApprovals.length}
                  </span>
                )}
              </button>
            </Dialog.Trigger>
            <Dialog.Portal>
              <Dialog.Overlay className="fixed inset-0 z-50 bg-slate-950/40 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
              <Dialog.Content className="fixed inset-x-0 bottom-0 z-50 max-h-[80vh] rounded-t-[28px] border-t border-[#e0d6c6] bg-white px-5 py-5 shadow-[0_-12px_40px_rgba(34,32,28,0.12)] focus:outline-none data-[state=open]:animate-in data-[state=open]:slide-in-from-bottom data-[state=closed]:animate-out data-[state=closed]:slide-out-to-bottom">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <Dialog.Title className="text-lg font-semibold text-slate-900">
                      {copy.pendingApprovals}
                    </Dialog.Title>
                    <Dialog.Description className="sr-only">
                      {copy.noPendingApprovals}
                    </Dialog.Description>
                    {pendingApprovals.length > 0 && (
                      <span className="inline-flex h-6 min-w-6 items-center justify-center rounded-full bg-[#fbf3df] px-2 text-xs font-semibold text-[#7c5d12]">
                        {pendingApprovals.length}
                      </span>
                    )}
                  </div>
                  <Dialog.Close asChild>
                    <button
                      type="button"
                      aria-label="Close"
                      className="inline-flex h-9 w-9 items-center justify-center rounded-2xl border border-[#e5ddd4] bg-[#faf6ef] text-slate-700 transition-colors hover:bg-white"
                    >
                      <X size={16} />
                    </button>
                  </Dialog.Close>
                </div>
                <div className="mt-4 max-h-[calc(80vh-80px)] overflow-y-auto">
                  <ApprovalPanel
                    title={copy.pendingApprovals}
                    icon={<Shield size={16} />}
                    emptyText={copy.noPendingApprovals}
                    items={pendingApprovals}
                    actions={approvalActions}
                    approvingId={approvingId}
                    loadingText={copy.loading}
                    hideTitle
                    onDecision={(approvalId, decision) => {
                      void handleApprovalDecision(approvalId, decision as RemoteApprovalDecision);
                    }}
                  />
                </div>
                <div className="absolute left-1/2 top-2 h-1 w-10 -translate-x-1/2 rounded-full bg-slate-300" />
              </Dialog.Content>
            </Dialog.Portal>
          </Dialog.Root>

          {/* Artifact bottom sheet */}
          <Dialog.Root>
            <Dialog.Trigger asChild>
              <button
                type="button"
                aria-label={copy.artifacts}
                className="relative inline-flex h-14 w-14 items-center justify-center rounded-2xl bg-[#17181a] text-white shadow-[0_10px_22px_rgba(23,24,26,0.25)] transition-transform active:scale-95"
              >
                <FileOutput size={20} />
                {artifacts.length > 0 && (
                  <span className="absolute -right-1 -top-1 flex h-5 min-w-5 items-center justify-center rounded-full bg-amber-500 px-1 text-[11px] font-bold text-white">
                    {artifacts.length}
                  </span>
                )}
              </button>
            </Dialog.Trigger>
            <Dialog.Portal>
              <Dialog.Overlay className="fixed inset-0 z-50 bg-slate-950/40 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
              <Dialog.Content className="fixed inset-x-0 bottom-0 z-50 max-h-[80vh] rounded-t-[28px] border-t border-[#e0d6c6] bg-white px-5 py-5 shadow-[0_-12px_40px_rgba(34,32,28,0.12)] focus:outline-none data-[state=open]:animate-in data-[state=open]:slide-in-from-bottom data-[state=closed]:animate-out data-[state=closed]:slide-out-to-bottom">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <Dialog.Title className="text-lg font-semibold text-slate-900">
                      {copy.artifacts}
                    </Dialog.Title>
                    <Dialog.Description className="sr-only">
                      {copy.noArtifacts}
                    </Dialog.Description>
                    {artifacts.length > 0 && (
                      <span className="inline-flex h-6 min-w-6 items-center justify-center rounded-full bg-[#fbf3df] px-2 text-xs font-semibold text-[#7c5d12]">
                        {artifacts.length}
                      </span>
                    )}
                  </div>
                  <Dialog.Close asChild>
                    <button
                      type="button"
                      aria-label="Close"
                      className="inline-flex h-9 w-9 items-center justify-center rounded-2xl border border-[#e5ddd4] bg-[#faf6ef] text-slate-700 transition-colors hover:bg-white"
                    >
                      <X size={16} />
                    </button>
                  </Dialog.Close>
                </div>
                <div className="mt-4 max-h-[calc(80vh-80px)] overflow-y-auto">
                  <ArtifactPanel
                    title={copy.artifacts}
                    icon={<FileOutput size={16} />}
                    emptyText={copy.noArtifacts}
                    items={artifacts}
                    downloadingId={downloadingArtifactId}
                    onDownload={(artifact) => {
                      const record = artifacts.find((item) => item.artifact_id === artifact.artifact_id);
                      if (!record) {
                        return;
                      }
                      void handleArtifactDownload(record);
                    }}
                    hideTitle
                  />
                </div>
                <div className="absolute left-1/2 top-2 h-1 w-10 -translate-x-1/2 rounded-full bg-slate-300" />
              </Dialog.Content>
            </Dialog.Portal>
          </Dialog.Root>
        </div>
      </main>
    </RemoteShell>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Timeline rendering
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
          <div className="whitespace-pre-wrap break-words text-[15px] leading-7 text-slate-800">
            {detail.text}
          </div>
        )}
      </TimelineMessageCard>
    );
  }

  if (detail.kind === 'message_delta') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.streaming}
        accent="text-amber-700"
        icon={<LoaderCircle size={16} className="animate-spin" />}
        timestampLabel={ts}
      >
        <div className="whitespace-pre-wrap break-words text-sm leading-6 text-slate-700">
          {detail.delta}
        </div>
      </TimelineEventCard>
    );
  }

  if (
    detail.kind === 'tool_started' ||
    detail.kind === 'tool_finished' ||
    detail.kind === 'tool_progress'
  ) {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.tool}
        accent={detail.kind === 'tool_finished' && detail.is_error ? 'text-rose-700' : 'text-emerald-700'}
        icon={<Bot size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-2 text-sm text-slate-700">
          <div className="font-medium text-slate-900">{toolLabel(detail)}</div>
          <div className="rounded-2xl bg-[#f7f2e8] px-3 py-2 text-sm leading-6 text-slate-600">
            {toolSummary(detail, copy)}
          </div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'approval_requested' || detail.kind === 'approval_resolved') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.approval}
        accent="text-[#7f4f19]"
        icon={<Shield size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-2 text-sm leading-6 text-slate-700">
          <div className="font-medium text-slate-900">{approvalSummary(detail, copy)}</div>
          {'responder' in detail && detail.responder && (
            <div className="text-xs uppercase tracking-[0.18em] text-slate-400">
              {copy.responderLabel}: {detail.responder}
            </div>
          )}
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'artifact_created' || detail.kind === 'artifact_manifest') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.artifact}
        accent="text-sky-700"
        icon={<FileOutput size={16} />}
        timestampLabel={ts}
      >
        <div className="text-sm leading-6 text-slate-700">{artifactSummary(detail, copy)}</div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'runtime_error') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.runtime}
        accent="text-rose-700"
        icon={<AlertTriangle size={16} />}
        timestampLabel={ts}
      >
        <div className="text-sm leading-6 text-rose-700">{detail.message}</div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'daemon_presence_changed') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.daemon}
        accent="text-slate-700"
        icon={detail.state === 'online' ? <Wifi size={16} /> : <WifiOff size={16} />}
        timestampLabel={ts}
      >
        <div className="text-sm text-slate-700">{copy.daemonNow(copy.daemonStates[detail.state])}</div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'subtask_started' || detail.kind === 'subtask_progress' || detail.kind === 'subtask_completed') {
    const stageLabel =
      detail.kind === 'subtask_started' ? 'started' :
      detail.kind === 'subtask_completed' ? 'completed' : 'progress';
    const desc = detail.kind === 'subtask_started' ? detail.description : detail.summary;
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.subtask}
        accent={stageLabel === 'completed' ? 'text-emerald-700' : 'text-violet-700'}
        icon={<GitBranch size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-1 text-sm text-slate-700">
          <div className="font-medium text-slate-900">{desc}</div>
          <div className="text-xs text-slate-400">
            {detail.task_id} · {stageLabel}
            {'turns_used' in detail && detail.turns_used != null ? ` · ${detail.turns_used} turns` : ''}
          </div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'batch_progress') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.batch}
        accent="text-blue-700"
        icon={<Layers size={16} />}
        timestampLabel={ts}
      >
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
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.context}
        accent={isOverflow ? 'text-amber-700' : 'text-slate-700'}
        icon={<Database size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-1 text-sm text-slate-700">
          <div className="font-medium text-slate-900">{isOverflow ? 'Context overflow' : 'Context usage'}: {pct}%</div>
          <div className="text-xs text-slate-400">{detail.estimated_tokens.toLocaleString()} / {detail.max_input_tokens.toLocaleString()} tokens</div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'context_compacted') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.context}
        accent="text-slate-700"
        icon={<Database size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-1 text-sm text-slate-700">
          <div className="font-medium text-slate-900">Context compacted</div>
          <div className="text-xs text-slate-400">{detail.entries_removed} entries removed · ratio {detail.usage_ratio.toFixed(2)}</div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'session_created' || detail.kind === 'session_state_changed') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.session}
        accent="text-slate-700"
        icon={<MessageSquareText size={16} />}
        timestampLabel={ts}
      >
        <div className="text-sm text-slate-700">{sessionEventSummary(detail, copy)}</div>
      </TimelineEventCard>
    );
  }

  return (
    <TimelineEventCard
      eyebrow={copy.eventEyebrows.runner}
      accent="text-slate-700"
      icon={<Bot size={16} />}
      timestampLabel={ts}
    >
      <div className="text-sm text-slate-700">{runnerEventSummary(detail, copy)}</div>
    </TimelineEventCard>
  );
}

// ═══════════════════════════════════════════════════════════════════════════
// Pure helpers
// ═══════════════════════════════════════════════════════════════════════════

function sessionCanAcceptCommands(session: RemoteSessionRecord | null): boolean {
  return Boolean(session?.owner_runner_id) && session?.owner_runner_available !== false;
}

function describeSessionControl(
  session: RemoteSessionRecord | null,
  locale: ReturnType<typeof resolveRemoteLocale>,
  copy: ReturnType<typeof getRemoteCopy>,
): {
  canSendPrompt: boolean;
  canInterrupt: boolean;
  notice: string | null;
} {
  if (!session) {
    return {
      canSendPrompt: false,
      canInterrupt: false,
      notice: null,
    };
  }

  if (!session.owner_runner_id) {
    return {
      canSendPrompt: false,
      canInterrupt: false,
      notice: copy.controlUnavailableUnassigned,
    };
  }

  if (session.owner_runner_available === false) {
    return {
      canSendPrompt: false,
      canInterrupt: false,
      notice: copy.controlUnavailableRunnerOffline(
        session.owner_runner_id,
        session.owner_runner_last_seen_at
          ? formatRemoteRelativeTime(session.owner_runner_last_seen_at, locale, copy)
          : null,
      ),
    };
  }

  const interactive = sessionCanAcceptCommands(session);
  return {
    canSendPrompt: interactive,
    canInterrupt: interactive,
    notice: null,
  };
}

function toolLabel(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_started' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_finished' }>,
): string {
  if ('tool_name' in detail && detail.tool_name) {
    return detail.tool_name;
  }
  return detail.tool_call_id ?? 'tool';
}

function toolSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_started' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_finished' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'tool_started') {
    return copy.toolStarted(detail.tool_call_id);
  }
  if (detail.kind === 'tool_progress') {
    if (detail.delta) {
      return detail.delta;
    }
    if (detail.elapsed_time_seconds != null) {
      return copy.toolElapsed(detail.elapsed_time_seconds);
    }
    return copy.toolRunning;
  }
  if (detail.summary) {
    return detail.summary;
  }
  return detail.is_error ? copy.toolFailedWithoutSummary : copy.toolCompleted;
}

function approvalSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'approval_requested' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'approval_resolved' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'approval_requested') {
    return copy.approvalWaiting(detail.title);
  }
  return copy.approvalResolved(detail.approval_id, copy.approvalStateLabels[detail.state]);
}

function artifactSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'artifact_created' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'artifact_manifest' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'artifact_created') {
    return copy.artifactCreated(detail.name, detail.file_name, formatBytes(detail.size_bytes));
  }
  return copy.artifactManifest(detail.artifact_ids.length);
}

function sessionEventSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'session_created' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'session_state_changed' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'session_created') {
    return copy.sessionCreated(detail.workspace_id);
  }
  return copy.sessionMoved(
    copy.sessionStateLabels[detail.previous_state],
    copy.sessionStateLabels[detail.state],
  );
}

function runnerEventSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'runner_registered' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'runner_heartbeat' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'runner_registered') {
    return copy.runnerRegistered(detail.workspace_ids.length, detail.lease_ttl_secs);
  }
  return copy.runnerHeartbeat(detail.active_sessions, detail.queued_sessions);
}

function extractErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return String(error);
}