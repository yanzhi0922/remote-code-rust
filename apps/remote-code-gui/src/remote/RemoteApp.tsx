import {
  AlertTriangle,
  Bot,
  Download,
  FileOutput,
  LoaderCircle,
  Menu,
  MessageSquareText,
  RotateCcw,
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
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import {
  clearRemoteAccessToken,
  persistRemoteAccessToken,
  resolveRemoteAccessToken,
  resolveRemoteBaseUrl,
  resolveRemotePairingContext,
  stripRemoteSensitiveQueryParams,
} from '../lib/runtime';
import { cn, truncateMiddle } from '../lib/utils';
import {
  acceptPairingOffer,
  buildArtifactDownloadUrl,
  buildSessionEventsStreamUrl,
  bootstrapControlPlane,
  getControlPlaneHealth,
  interruptSession,
  listSessionApprovals,
  listSessionArtifacts,
  listSessionEvents,
  listSessions,
  respondToApproval,
  sendPrompt,
} from './api';
import type {
  RemoteApprovalDecision,
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteControlPlaneHealth,
  RemoteMessageRole,
  RemoteSessionRecord,
  RemoteSessionState,
  RemoteTimelineEvent,
  RemoteTimelineEventDetail,
} from './types';

type ConnectionState = 'idle' | 'connecting' | 'open' | 'reconnecting' | 'error';

const LazyMarkdownRenderer = lazy(() => import('../components/chat/MarkdownRenderer'));

const SESSION_STATE_LABELS: Record<RemoteSessionState, string> = {
  pending: 'Pending',
  assigned: 'Assigned',
  running: 'Running',
  waiting_approval: 'Waiting Approval',
  completed: 'Completed',
  failed: 'Failed',
  cancelled: 'Cancelled',
};

const APPROVAL_DECISIONS: Array<{
  decision: RemoteApprovalDecision;
  label: string;
  className: string;
}> = [
  {
    decision: 'approved',
    label: 'Approve',
    className: 'bg-[#1d6b45] text-white hover:bg-[#145033]',
  },
  {
    decision: 'denied',
    label: 'Deny',
    className: 'bg-[#a13a30] text-white hover:bg-[#7e2b24]',
  },
  {
    decision: 'cancelled',
    label: 'Cancel',
    className: 'bg-[#efe7db] text-slate-700 hover:bg-[#e6dccd]',
  },
];

export default function RemoteApp() {
  const baseUrl = resolveRemoteBaseUrl();
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
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [events, setEvents] = useState<RemoteTimelineEvent[]>([]);
  const deferredEvents = useDeferredValue(events);
  const [eventsLoading, setEventsLoading] = useState(false);
  const [approvals, setApprovals] = useState<RemoteApprovalRecord[]>([]);
  const [artifacts, setArtifacts] = useState<RemoteArtifactRecord[]>([]);
  const [composer, setComposer] = useState('');
  const [sending, setSending] = useState(false);
  const [interrupting, setInterrupting] = useState(false);
  const [approvingId, setApprovingId] = useState<string | null>(null);
  const [connectionState, setConnectionState] = useState<ConnectionState>('idle');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const activeSessionIdRef = useRef<string | null>(null);
  const latestSequenceRef = useRef(0);
  const statusTimerRef = useRef<number | null>(null);

  activeSessionIdRef.current = activeSessionId;

  const activeSession = useMemo(
    () => sessions.find((session) => session.session_id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );
  const pendingApprovals = useMemo(
    () => approvals.filter((approval) => approval.state === 'pending'),
    [approvals],
  );
  const authRequired = health?.auth_required ?? false;
  const showAuthGate = Boolean(baseUrl) && ((authRequired && !accessToken) || authErrorMessage);

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
          setAuthErrorMessage(extractErrorMessage(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [baseUrl, accessToken]);

  const completeAuthentication = useEffectEvent((token: string, message: string) => {
    persistRemoteAccessToken(token);
    stripRemoteSensitiveQueryParams();
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
      completeAuthentication(response.access_token, 'Bootstrap claim succeeded.');
      const nextHealth = await getControlPlaneHealth(baseUrl);
      setHealth(nextHealth);
    } catch (error) {
      setAuthErrorMessage(extractErrorMessage(error));
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
      completeAuthentication(response.access_token, 'Pairing succeeded.');
      const nextHealth = await getControlPlaneHealth(baseUrl);
      setHealth(nextHealth);
    } catch (error) {
      setAuthErrorMessage(extractErrorMessage(error));
    } finally {
      setAuthLoading(false);
    }
  });

  const handleManualTokenSave = useEffectEvent(() => {
    if (!manualAccessToken.trim()) {
      return;
    }
    persistRemoteAccessToken(manualAccessToken);
    stripRemoteSensitiveQueryParams();
    setAccessToken(manualAccessToken.trim());
    setAuthErrorMessage(null);
    showStatusMessage('Saved access token locally.');
  });

  const handleClearSavedToken = useEffectEvent(() => {
    clearRemoteAccessToken();
    stripRemoteSensitiveQueryParams();
    setAccessToken(null);
    setManualAccessToken('');
    setAuthErrorMessage(null);
    showStatusMessage('Cleared the saved access token.');
  });

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
        return nextSessions[0]?.session_id ?? null;
      });
      setErrorMessage(null);
    } catch (error) {
      const message = extractErrorMessage(error);
      if (message.includes('HTTP 401')) {
        setAuthErrorMessage(message);
      } else {
        setErrorMessage(message);
      }
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

    const [eventsResponse, approvalsResponse, artifactsResponse] = await Promise.all([
      listSessionEvents(baseUrl, sessionId),
      listSessionApprovals(baseUrl, sessionId),
      listSessionArtifacts(baseUrl, sessionId),
    ]);

    if (activeSessionIdRef.current !== sessionId) {
      return;
    }

    const hydratedEvents = hydrateTimeline(eventsResponse.items);
    const latestSequence =
      eventsResponse.latest_sequence ??
      hydratedEvents[hydratedEvents.length - 1]?.sequence ??
      0;
    latestSequenceRef.current = latestSequence;

    startTransition(() => {
      setEvents(hydratedEvents);
    });
    setApprovals(
      [...approvalsResponse.items].sort(
        (left, right) =>
          new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime(),
      ),
    );
    setArtifacts(
      [...artifactsResponse.items].sort(
        (left, right) =>
          new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
      ),
    );
  });

  const handleLiveEvent = useEffectEvent((sessionId: string, event: RemoteTimelineEvent) => {
    if (activeSessionIdRef.current !== sessionId) {
      return;
    }

    latestSequenceRef.current = Math.max(latestSequenceRef.current, event.sequence);
    startTransition(() => {
      setEvents((current) => appendTimelineEvent(current, event));
    });

    if (event.detail.kind === 'approval_requested' || event.detail.kind === 'approval_resolved') {
      void refreshApprovals(sessionId);
    }
    if (event.detail.kind === 'artifact_created' || event.detail.kind === 'artifact_manifest') {
      void refreshArtifacts(sessionId);
    }
    if (
      event.detail.kind === 'session_state_changed' ||
      event.detail.kind === 'approval_requested' ||
      event.detail.kind === 'approval_resolved' ||
      event.detail.kind === 'daemon_presence_changed'
    ) {
      void refreshSessions();
    }
  });

  useEffect(() => {
    void refreshSessions();
    const intervalId = window.setInterval(() => {
      void refreshSessions();
    }, 15_000);
    return () => {
      window.clearInterval(intervalId);
    };
  }, [accessToken, baseUrl, health, refreshSessions]);

  useEffect(() => {
    const onVisibilityChange = () => {
      if (document.visibilityState !== 'visible') {
        return;
      }
      void refreshSessions();
      if (activeSessionIdRef.current) {
        void refreshSessionBundle(activeSessionIdRef.current);
      }
    };

    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, [refreshSessionBundle, refreshSessions]);

  useEffect(() => {
    return () => {
      if (statusTimerRef.current !== null) {
        window.clearTimeout(statusTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!baseUrl || !activeSessionId || !health || (authRequired && !accessToken)) {
      setEvents([]);
      setApprovals([]);
      setArtifacts([]);
      setConnectionState('idle');
      latestSequenceRef.current = 0;
      return;
    }

    let cancelled = false;
    let reconnectTimer: number | null = null;
    let socket: WebSocket | null = null;

    const openSocket = (after: number) => {
      if (cancelled) {
        return;
      }

      setConnectionState(after > 0 ? 'reconnecting' : 'connecting');
      socket = new WebSocket(buildSessionEventsStreamUrl(baseUrl, activeSessionId, after));

      socket.onopen = () => {
        if (!cancelled) {
          setConnectionState('open');
        }
      };

      socket.onmessage = (message) => {
        if (typeof message.data !== 'string') {
          return;
        }

        try {
          const event = JSON.parse(message.data) as RemoteTimelineEvent;
          handleLiveEvent(activeSessionId, event);
        } catch {
          setConnectionState('error');
        }
      };

      socket.onerror = () => {
        if (!cancelled) {
          setConnectionState('error');
        }
      };

      socket.onclose = () => {
        if (cancelled) {
          return;
        }
        reconnectTimer = window.setTimeout(() => {
          openSocket(latestSequenceRef.current);
        }, 1_000);
      };
    };

    const bootstrap = async () => {
      setEventsLoading(true);
      try {
        await refreshSessionBundle(activeSessionId);
        if (!cancelled) {
          openSocket(latestSequenceRef.current);
          setErrorMessage(null);
        }
      } catch (error) {
        if (!cancelled) {
          setConnectionState('error');
          const message = extractErrorMessage(error);
          if (message.includes('HTTP 401')) {
            setAuthErrorMessage(message);
          } else {
            setErrorMessage(message);
          }
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
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
      }
      socket?.close();
    };
  }, [accessToken, activeSessionId, authRequired, baseUrl, handleLiveEvent, health, refreshSessionBundle]);

  const handleSendPrompt = async () => {
    if (!baseUrl || !activeSessionId || (authRequired && !accessToken) || !composer.trim() || sending) {
      return;
    }

    setSending(true);
    try {
      await sendPrompt(baseUrl, activeSessionId, composer.trim());
      setComposer('');
      showStatusMessage('Prompt forwarded to the local runner.');
    } catch (error) {
      const message = extractErrorMessage(error);
      if (message.includes('HTTP 401')) {
        setAuthErrorMessage(message);
      } else {
        setErrorMessage(message);
      }
    } finally {
      setSending(false);
    }
  };

  const handleInterrupt = async () => {
    if (!baseUrl || !activeSessionId || (authRequired && !accessToken) || interrupting) {
      return;
    }

    setInterrupting(true);
    try {
      await interruptSession(baseUrl, activeSessionId);
      showStatusMessage('Interrupt signal forwarded.');
    } catch (error) {
      const message = extractErrorMessage(error);
      if (message.includes('HTTP 401')) {
        setAuthErrorMessage(message);
      } else {
        setErrorMessage(message);
      }
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
      await respondToApproval(baseUrl, approvalId, decision);
      showStatusMessage(`Approval ${decision}.`);
      if (activeSessionId) {
        await refreshApprovals(activeSessionId);
      }
    } catch (error) {
      const message = extractErrorMessage(error);
      if (message.includes('HTTP 401')) {
        setAuthErrorMessage(message);
      } else {
        setErrorMessage(message);
      }
    } finally {
      setApprovingId(null);
    }
  };

  if (!baseUrl) {
    return (
      <RemoteFrame>
        <div className="flex min-h-screen items-center justify-center px-6">
          <EmptyCard
            title="Remote Mode Is Not Configured"
            description="Open this UI from your control-plane domain, or pass `?mode=remote&control_plane_url=https://your-domain`."
          />
        </div>
      </RemoteFrame>
    );
  }

  if (!health) {
    return (
      <RemoteFrame>
        <div className="flex min-h-screen items-center justify-center px-6">
          <div className="flex items-center gap-3 rounded-2xl border border-[#e2d8c8] bg-white px-5 py-4 text-sm text-slate-600 shadow-[0_18px_45px_rgba(52,45,34,0.08)]">
            <LoaderCircle size={16} className="animate-spin" />
            Contacting the control plane...
          </div>
        </div>
      </RemoteFrame>
    );
  }

  if (showAuthGate) {
    return (
      <RemoteFrame>
        <RemoteAuthGate
          authErrorMessage={authErrorMessage}
          authLoading={authLoading}
          bootstrapEnabled={!health.owner_claimed && health.bootstrap_secret_configured}
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
      </RemoteFrame>
    );
  }

  return (
    <RemoteFrame>
      {sidebarOpen && (
        <button
          aria-label="Close session sidebar"
          className="fixed inset-0 z-30 bg-slate-950/30 lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      <div className="mx-auto flex min-h-screen max-w-[1580px] flex-col lg:flex-row">
        <aside
          className={cn(
            'fixed inset-y-0 left-0 z-40 w-[320px] transform border-r border-[#e5ddcf] bg-[#f5efe4] transition-transform lg:static lg:z-0 lg:translate-x-0',
            sidebarOpen ? 'translate-x-0' : '-translate-x-full',
          )}
        >
          <div className="border-b border-[#e5ddcf] px-5 py-5">
            <div className="text-[11px] font-semibold uppercase tracking-[0.28em] text-slate-400">
              Remote Shell
            </div>
            <div className="mt-2 text-2xl font-semibold text-slate-900">remote-code</div>
            <div className="mt-3 text-sm leading-6 text-slate-500">
              Time line, approvals, artifact download, and follow-up control routed through your self-hosted control plane.
            </div>
            <button
              type="button"
              onClick={() => {
                void refreshSessions();
              }}
              className="mt-4 inline-flex items-center gap-2 rounded-full border border-[#ddd4c5] bg-white px-3 py-1.5 text-sm text-slate-700 transition-colors hover:bg-[#faf6ef]"
            >
              <RotateCcw size={14} />
              Refresh Sessions
            </button>
          </div>

          <div className="h-[calc(100vh-181px)] overflow-y-auto px-3 py-4">
            {sessionsLoading ? (
              <div className="flex items-center gap-2 rounded-2xl bg-white/80 px-4 py-3 text-sm text-slate-500">
                <LoaderCircle size={16} className="animate-spin" />
                Loading remote sessions...
              </div>
            ) : sessions.length === 0 ? (
              <EmptyCard
                title="No Sessions Yet"
                description="Start a local session on your runner, then refresh here to attach from the browser."
              />
            ) : (
              <div className="space-y-2">
                {sessions.map((session) => {
                  const selected = session.session_id === activeSessionId;
                  return (
                    <button
                      key={session.session_id}
                      type="button"
                      onClick={() => {
                        setActiveSessionId(session.session_id);
                        setSidebarOpen(false);
                      }}
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
                            {sessionTitle(session)}
                          </div>
                          <div className="mt-1 text-xs text-slate-500">
                            {truncateMiddle(session.workspace_id, 48)}
                          </div>
                        </div>
                        <StatePill state={session.state} />
                      </div>
                      <div className="mt-3 flex items-center gap-2 text-[11px] text-slate-500">
                        <span>{formatRelativeTime(session.updated_at)}</span>
                        <span>•</span>
                        <span>{session.owner_runner_id ?? 'unassigned runner'}</span>
                      </div>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </aside>

        <div className="flex min-h-screen min-w-0 flex-1 flex-col">
          <header className="border-b border-[#e5ddcf] bg-white/90 px-4 py-4 backdrop-blur sm:px-6">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3">
                <button
                  type="button"
                  className="inline-flex h-11 w-11 items-center justify-center rounded-2xl border border-[#ddd4c5] bg-[#faf6ef] text-slate-700 lg:hidden"
                  onClick={() => setSidebarOpen(true)}
                >
                  <Menu size={18} />
                </button>
                <div className="min-w-0">
                  <div className="truncate text-lg font-semibold text-slate-900">
                    {activeSession ? sessionTitle(activeSession) : 'Select a remote session'}
                  </div>
                  <div className="mt-1 flex flex-wrap items-center gap-2 text-sm text-slate-500">
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

              <div className="flex flex-wrap items-center gap-2">
                <ConnectionPill state={connectionState} />
                {activeSession && <StatePill state={activeSession.state} compact />}
              </div>
            </div>
          </header>

          {errorMessage && (
            <div className="border-b border-[#f1d2c9] bg-[#fff4f1] px-4 py-3 text-sm text-[#9b3b32] sm:px-6">
              {errorMessage}
            </div>
          )}

          {statusMessage && (
            <div className="border-b border-[#d9eadf] bg-[#edf7ef] px-4 py-3 text-sm text-[#226140] sm:px-6">
              {statusMessage}
            </div>
          )}

          <main className="grid min-h-0 flex-1 gap-0 lg:grid-cols-[minmax(0,1fr)_340px]">
            <section className="flex min-h-0 flex-col border-b border-[#e5ddcf] bg-[#f7f2e8] lg:border-b-0 lg:border-r">
              {activeSession ? (
                <div className="flex min-h-0 flex-1 flex-col">
                  <div className="flex-1 overflow-y-auto px-4 py-5 sm:px-6">
                    {eventsLoading ? (
                      <div className="flex min-h-[280px] items-center justify-center">
                        <div className="flex items-center gap-3 rounded-2xl bg-white px-4 py-3 text-sm text-slate-500 shadow-[0_12px_28px_rgba(34,32,28,0.06)]">
                          <LoaderCircle size={16} className="animate-spin" />
                          Loading session timeline...
                        </div>
                      </div>
                    ) : deferredEvents.length === 0 ? (
                      <div className="flex min-h-[280px] items-center justify-center">
                        <EmptyCard
                          title="Timeline Is Empty"
                          description="Once the local runner starts streaming events, message deltas, approvals, tools, and artifacts appear here."
                        />
                      </div>
                    ) : (
                      <div className="mx-auto flex w-full max-w-5xl flex-col gap-4">
                        {deferredEvents.map((event) => (
                          <TimelineCard key={event.sequence} event={event} />
                        ))}
                      </div>
                    )}
                  </div>

                  <div className="border-t border-[#e5ddcf] bg-[#f5efe4] px-4 py-4 sm:px-6">
                    <div className="mx-auto max-w-5xl rounded-[28px] border border-[#ded4c4] bg-white shadow-[0_18px_44px_rgba(34,32,28,0.09)]">
                      <div className="flex items-center justify-between gap-3 border-b border-[#efe6d9] px-4 py-3">
                        <div className="inline-flex items-center gap-2 text-sm text-slate-600">
                          <MessageSquareText size={15} />
                          Follow-up control for the current session
                        </div>
                        <button
                          type="button"
                          onClick={() => {
                            void handleInterrupt();
                          }}
                          disabled={interrupting}
                          className="inline-flex items-center gap-2 rounded-full border border-[#dccfc0] bg-[#faf6ef] px-3 py-1.5 text-sm text-slate-700 transition-colors hover:bg-[#f3ebdf] disabled:cursor-not-allowed disabled:opacity-60"
                        >
                          {interrupting ? (
                            <LoaderCircle size={14} className="animate-spin" />
                          ) : (
                            <Square size={14} />
                          )}
                          Interrupt
                        </button>
                      </div>
                      <div className="flex items-end gap-3 px-4 py-4">
                        <textarea
                          value={composer}
                          onChange={(event) => setComposer(event.target.value)}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter' && !event.shiftKey) {
                              event.preventDefault();
                              void handleSendPrompt();
                            }
                          }}
                          rows={1}
                          placeholder="Send a follow-up prompt to the local runner. Shift+Enter inserts a newline."
                          className="min-h-[88px] flex-1 resize-none bg-transparent text-[15px] leading-6 text-slate-800 outline-none placeholder:text-slate-400"
                        />
                        <button
                          type="button"
                          onClick={() => {
                            void handleSendPrompt();
                          }}
                          disabled={sending || composer.trim().length === 0}
                          className="inline-flex h-14 items-center justify-center rounded-2xl bg-[#17181a] px-5 text-sm font-medium text-white shadow-[0_10px_22px_rgba(23,24,26,0.2)] transition-colors hover:bg-[#282a2d] disabled:cursor-not-allowed disabled:bg-[#cbbfac] disabled:text-white/70"
                        >
                          {sending ? <LoaderCircle size={18} className="animate-spin" /> : 'Send'}
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="flex min-h-[420px] items-center justify-center px-6">
                  <EmptyCard
                    title="Pick A Session"
                    description="The browser shell stays read-only until you attach to a session on the left."
                  />
                </div>
              )}
            </section>

            <aside className="border-t border-[#e5ddcf] bg-[#f2ebdf] lg:border-t-0">
              <div className="grid gap-4 px-4 py-5 sm:px-6 lg:sticky lg:top-0">
                <section className="rounded-[24px] border border-[#e0d6c6] bg-white px-4 py-4 shadow-[0_12px_30px_rgba(34,32,28,0.06)]">
                  <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
                    <Shield size={16} />
                    Pending Approvals
                  </div>
                  <div className="mt-4 space-y-3">
                    {pendingApprovals.length === 0 ? (
                      <PanelHint>No pending approvals for the current session.</PanelHint>
                    ) : (
                      pendingApprovals.map((approval) => (
                        <div
                          key={approval.approval_id}
                          className="rounded-2xl border border-[#ebe2d5] bg-[#faf7f1] px-3 py-3"
                        >
                          <div className="text-sm font-medium text-slate-900">{approval.title}</div>
                          <div className="mt-1 text-sm leading-6 text-slate-600">
                            {approval.description}
                          </div>
                          {approval.metadata.blocked_path && (
                            <div className="mt-2 rounded-xl bg-white px-3 py-2 font-mono text-xs text-slate-500">
                              {truncateMiddle(approval.metadata.blocked_path, 56)}
                            </div>
                          )}
                          <div className="mt-3 flex flex-wrap gap-2">
                            {APPROVAL_DECISIONS.map((item) => (
                              <button
                                key={item.decision}
                                type="button"
                                onClick={() => {
                                  void handleApprovalDecision(approval.approval_id, item.decision);
                                }}
                                disabled={approvingId === approval.approval_id}
                                className={cn(
                                  'rounded-full px-3 py-1.5 text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-60',
                                  item.className,
                                )}
                              >
                                {approvingId === approval.approval_id ? (
                                  <span className="inline-flex items-center gap-2">
                                    <LoaderCircle size={14} className="animate-spin" />
                                    Working...
                                  </span>
                                ) : (
                                  item.label
                                )}
                              </button>
                            ))}
                          </div>
                        </div>
                      ))
                    )}
                  </div>
                </section>

                <section className="rounded-[24px] border border-[#e0d6c6] bg-white px-4 py-4 shadow-[0_12px_30px_rgba(34,32,28,0.06)]">
                  <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
                    <FileOutput size={16} />
                    Artifacts
                  </div>
                  <div className="mt-4 space-y-3">
                    {artifacts.length === 0 ? (
                      <PanelHint>No artifacts have been published yet.</PanelHint>
                    ) : (
                      artifacts.map((artifact) => (
                        <a
                          key={artifact.artifact_id}
                          href={buildArtifactDownloadUrl(baseUrl, artifact.artifact_id)}
                          className="flex items-start justify-between gap-3 rounded-2xl border border-[#ebe2d5] bg-[#faf7f1] px-3 py-3 transition-colors hover:bg-white"
                        >
                          <div className="min-w-0">
                            <div className="truncate text-sm font-medium text-slate-900">
                              {artifact.name}
                            </div>
                            <div className="mt-1 text-xs text-slate-500">
                              {artifact.file_name} • {formatBytes(artifact.size_bytes)}
                            </div>
                          </div>
                          <Download size={16} className="mt-0.5 shrink-0 text-slate-500" />
                        </a>
                      ))
                    )}
                  </div>
                </section>
              </div>
            </aside>
          </main>
        </div>
      </div>
    </RemoteFrame>
  );
}

function RemoteAuthGate({
  authErrorMessage,
  authLoading,
  bootstrapEnabled,
  deviceName,
  health,
  manualAccessToken,
  onBootstrapClaim,
  onClearSavedToken,
  onManualTokenSave,
  onPairingAccept,
  pairingOfferId,
  pairingSecret,
  setBootstrapSecret,
  setDeviceName,
  setManualAccessToken,
  setPairingOfferId,
  setPairingSecret,
}: {
  authErrorMessage: string | null;
  authLoading: boolean;
  bootstrapEnabled: boolean;
  deviceName: string;
  health: RemoteControlPlaneHealth;
  manualAccessToken: string;
  onBootstrapClaim: () => void;
  onClearSavedToken: () => void;
  onManualTokenSave: () => void;
  onPairingAccept: () => void;
  pairingOfferId: string;
  pairingSecret: string;
  setBootstrapSecret: (value: string) => void;
  setDeviceName: (value: string) => void;
  setManualAccessToken: (value: string) => void;
  setPairingOfferId: (value: string) => void;
  setPairingSecret: (value: string) => void;
}) {
  return (
    <div className="mx-auto flex min-h-screen max-w-5xl items-center px-6 py-10">
      <div className="grid w-full gap-6 lg:grid-cols-[1.1fr_0.9fr]">
        <section className="rounded-[36px] border border-[#ddd2c1] bg-white px-7 py-7 shadow-[0_30px_70px_rgba(52,45,34,0.1)]">
          <div className="text-[11px] font-semibold uppercase tracking-[0.32em] text-slate-400">
            Remote Access
          </div>
          <div className="mt-3 text-3xl font-semibold text-slate-900">Authenticate This Device</div>
          <div className="mt-4 max-w-2xl text-sm leading-7 text-slate-500">
            The control plane is live, but this browser is not trusted yet. Claim the owner device
            first, or accept a short-lived pairing offer generated from a device that is already
            trusted.
          </div>

          {authErrorMessage && (
            <div className="mt-5 flex items-start gap-3 rounded-3xl border border-[#f0d3c8] bg-[#fff2ed] px-4 py-4 text-sm leading-6 text-[#8d3f30]">
              <AlertTriangle size={18} className="mt-0.5 shrink-0" />
              <div>{authErrorMessage}</div>
            </div>
          )}

          <div className="mt-6 space-y-5">
            <label className="block">
              <div className="mb-2 text-xs font-semibold uppercase tracking-[0.2em] text-slate-400">
                Device Name
              </div>
              <input
                value={deviceName}
                onChange={(event) => setDeviceName(event.target.value)}
                placeholder="My iPhone"
                className="w-full rounded-2xl border border-[#dfd5c6] bg-[#fcfaf6] px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
              />
            </label>

            {bootstrapEnabled && (
              <div className="rounded-[28px] border border-[#e7dccd] bg-[#faf6ef] px-5 py-5">
                <div className="text-sm font-semibold text-slate-900">Bootstrap owner claim</div>
                <div className="mt-2 text-sm leading-6 text-slate-500">
                  Use the bootstrap secret from the server to mint the first trusted device token.
                </div>
                <label className="mt-4 block">
                  <div className="mb-2 text-xs font-semibold uppercase tracking-[0.2em] text-slate-400">
                    Bootstrap Secret
                  </div>
                  <input
                    type="password"
                    onChange={(event) => setBootstrapSecret(event.target.value)}
                    className="w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
                  />
                </label>
                <button
                  type="button"
                  onClick={onBootstrapClaim}
                  disabled={authLoading}
                  className="mt-4 inline-flex items-center gap-2 rounded-full bg-[#1d6b45] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[#145033] disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {authLoading ? <LoaderCircle size={15} className="animate-spin" /> : <Shield size={15} />}
                  Claim Owner Device
                </button>
              </div>
            )}

            <div className="rounded-[28px] border border-[#e7dccd] bg-[#faf6ef] px-5 py-5">
              <div className="text-sm font-semibold text-slate-900">Accept pairing offer</div>
              <div className="mt-2 text-sm leading-6 text-slate-500">
                Paste the offer id and pairing secret from a trusted device, or open the pairing
                URL directly on this phone.
              </div>
              <div className="mt-4 grid gap-3">
                <input
                  value={pairingOfferId}
                  onChange={(event) => setPairingOfferId(event.target.value)}
                  placeholder="Offer ID"
                  className="w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
                />
                <input
                  value={pairingSecret}
                  type="password"
                  onChange={(event) => setPairingSecret(event.target.value)}
                  placeholder="Pairing secret"
                  className="w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
                />
              </div>
              <button
                type="button"
                onClick={onPairingAccept}
                disabled={authLoading}
                className="mt-4 inline-flex items-center gap-2 rounded-full bg-[#174e8c] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[#123b6b] disabled:cursor-not-allowed disabled:opacity-60"
              >
                {authLoading ? <LoaderCircle size={15} className="animate-spin" /> : <Wifi size={15} />}
                Accept Pairing Offer
              </button>
            </div>

            <div className="rounded-[28px] border border-[#e7dccd] bg-[#faf6ef] px-5 py-5">
              <div className="text-sm font-semibold text-slate-900">Use an existing token</div>
              <div className="mt-2 text-sm leading-6 text-slate-500">
                If you already minted a device token from the CLI, paste it here and store it in
                this browser.
              </div>
              <textarea
                value={manualAccessToken}
                onChange={(event) => setManualAccessToken(event.target.value)}
                rows={3}
                placeholder="rcdt_..."
                className="mt-4 w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
              />
              <div className="mt-4 flex flex-wrap gap-3">
                <button
                  type="button"
                  onClick={onManualTokenSave}
                  className="inline-flex items-center gap-2 rounded-full border border-[#cfbfaa] bg-white px-4 py-2 text-sm font-medium text-slate-700 transition-colors hover:bg-[#fffaf2]"
                >
                  Save Token
                </button>
                <button
                  type="button"
                  onClick={onClearSavedToken}
                  className="inline-flex items-center gap-2 rounded-full border border-[#eadccb] bg-transparent px-4 py-2 text-sm font-medium text-slate-500 transition-colors hover:bg-[#f4ecdf]"
                >
                  Clear Saved Token
                </button>
              </div>
            </div>
          </div>
        </section>

        <aside className="rounded-[36px] border border-[#ddd2c1] bg-[#f8f2e7] px-6 py-6 shadow-[0_24px_60px_rgba(52,45,34,0.08)]">
          <div className="text-[11px] font-semibold uppercase tracking-[0.32em] text-slate-400">
            Control Plane
          </div>
          <div className="mt-3 text-xl font-semibold text-slate-900">{health.service}</div>
          <div className="mt-5 space-y-3 text-sm leading-6 text-slate-600">
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              Owner claimed: {health.owner_claimed ? 'yes' : 'no'}
            </div>
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              Trusted devices: {health.device_count}
            </div>
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              Available runners: {health.available_runner_count}
            </div>
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              Active sessions: {health.session_count}
            </div>
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              Bootstrap configured: {health.bootstrap_secret_configured ? 'yes' : 'no'}
            </div>
          </div>
          <div className="mt-6 rounded-3xl border border-dashed border-[#d4c4ac] bg-white/70 px-4 py-4 text-sm leading-6 text-slate-500">
            The browser only stores the device access token locally. Session content still stays
            on your local machine; the control plane only brokers access to the runner.
          </div>
        </aside>
      </div>
    </div>
  );
}

function TimelineCard({ event }: { event: RemoteTimelineEvent }) {
  const { detail } = event;

  if (detail.kind === 'message_committed') {
    return (
      <MessageCard
        role={detail.role}
        header={detail.role === 'assistant' ? 'Assistant' : detail.role === 'user' ? 'User' : 'System'}
      >
        {detail.role === 'assistant' ? (
          <Suspense fallback={<div className="text-sm text-slate-500">Rendering response...</div>}>
            <LazyMarkdownRenderer content={detail.text} />
          </Suspense>
        ) : (
          <div className="whitespace-pre-wrap break-words text-[15px] leading-7 text-slate-800">
            {detail.text}
          </div>
        )}
      </MessageCard>
    );
  }

  if (detail.kind === 'message_delta') {
    return (
      <EventCard
        eyebrow="Streaming"
        accent="text-amber-700"
        icon={<LoaderCircle size={16} className="animate-spin" />}
        timestamp={event.recorded_at}
      >
        <div className="whitespace-pre-wrap break-words text-sm leading-6 text-slate-700">
          {detail.delta}
        </div>
      </EventCard>
    );
  }

  if (
    detail.kind === 'tool_started' ||
    detail.kind === 'tool_finished' ||
    detail.kind === 'tool_progress'
  ) {
    return (
      <EventCard
        eyebrow="Tool"
        accent={detail.kind === 'tool_finished' && detail.is_error ? 'text-rose-700' : 'text-emerald-700'}
        icon={<Bot size={16} />}
        timestamp={event.recorded_at}
      >
        <div className="space-y-2 text-sm text-slate-700">
          <div className="font-medium text-slate-900">{toolLabel(detail)}</div>
          <div className="rounded-2xl bg-[#f7f2e8] px-3 py-2 text-sm leading-6 text-slate-600">
            {toolSummary(detail)}
          </div>
        </div>
      </EventCard>
    );
  }

  if (detail.kind === 'approval_requested' || detail.kind === 'approval_resolved') {
    return (
      <EventCard
        eyebrow="Approval"
        accent="text-[#7f4f19]"
        icon={<Shield size={16} />}
        timestamp={event.recorded_at}
      >
        <div className="space-y-2 text-sm leading-6 text-slate-700">
          <div className="font-medium text-slate-900">{approvalSummary(detail)}</div>
          {'responder' in detail && detail.responder && (
            <div className="text-xs uppercase tracking-[0.18em] text-slate-400">
              Responder: {detail.responder}
            </div>
          )}
        </div>
      </EventCard>
    );
  }

  if (detail.kind === 'artifact_created' || detail.kind === 'artifact_manifest') {
    return (
      <EventCard
        eyebrow="Artifact"
        accent="text-sky-700"
        icon={<FileOutput size={16} />}
        timestamp={event.recorded_at}
      >
        <div className="text-sm leading-6 text-slate-700">{artifactSummary(detail)}</div>
      </EventCard>
    );
  }

  if (detail.kind === 'runtime_error') {
    return (
      <EventCard
        eyebrow="Runtime"
        accent="text-rose-700"
        icon={<AlertTriangle size={16} />}
        timestamp={event.recorded_at}
      >
        <div className="text-sm leading-6 text-rose-700">{detail.message}</div>
      </EventCard>
    );
  }

  if (detail.kind === 'daemon_presence_changed') {
    return (
      <EventCard
        eyebrow="Daemon"
        accent="text-slate-700"
        icon={detail.state === 'online' ? <Wifi size={16} /> : <WifiOff size={16} />}
        timestamp={event.recorded_at}
      >
        <div className="text-sm text-slate-700">Daemon is now {detail.state.replace('_', ' ')}.</div>
      </EventCard>
    );
  }

  if (detail.kind === 'session_created' || detail.kind === 'session_state_changed') {
    return (
      <EventCard
        eyebrow="Session"
        accent="text-slate-700"
        icon={<MessageSquareText size={16} />}
        timestamp={event.recorded_at}
      >
        <div className="text-sm text-slate-700">{sessionEventSummary(detail)}</div>
      </EventCard>
    );
  }

  return (
    <EventCard
      eyebrow="Runner"
      accent="text-slate-700"
      icon={<Bot size={16} />}
      timestamp={event.recorded_at}
    >
      <div className="text-sm text-slate-700">{runnerEventSummary(detail)}</div>
    </EventCard>
  );
}

function RemoteFrame({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen bg-[radial-gradient(circle_at_top_left,#fbf6ec,transparent_28%),linear-gradient(180deg,#f4efe4_0%,#efe8db_100%)] text-slate-900">
      {children}
    </div>
  );
}

function EmptyCard({
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

function PanelHint({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-2xl bg-[#faf7f1] px-3 py-3 text-sm leading-6 text-slate-500">
      {children}
    </div>
  );
}

function MessageCard({
  role,
  header,
  children,
}: {
  role: RemoteMessageRole;
  header: string;
  children: ReactNode;
}) {
  const isUser = role === 'user';
  return (
    <div className={cn('flex', isUser ? 'justify-end' : 'justify-start')}>
      <div
        className={cn(
          'max-w-4xl rounded-[28px] px-5 py-4 shadow-[0_16px_34px_rgba(34,32,28,0.07)]',
          isUser ? 'bg-[#17181a] text-white' : 'border border-[#e5ddcf] bg-white',
        )}
      >
        <div
          className={cn(
            'mb-3 text-xs font-semibold uppercase tracking-[0.22em]',
            isUser ? 'text-white/60' : 'text-slate-400',
          )}
        >
          {header}
        </div>
        {children}
      </div>
    </div>
  );
}

function EventCard({
  eyebrow,
  accent,
  icon,
  timestamp,
  children,
}: {
  eyebrow: string;
  accent: string;
  icon: ReactNode;
  timestamp: string;
  children: ReactNode;
}) {
  return (
    <div className="rounded-[24px] border border-[#e5ddcf] bg-white px-5 py-4 shadow-[0_14px_32px_rgba(34,32,28,0.06)]">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div
          className={cn(
            'inline-flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.22em]',
            accent,
          )}
        >
          {icon}
          {eyebrow}
        </div>
        <div className="text-xs text-slate-400">{formatRelativeTime(timestamp)}</div>
      </div>
      {children}
    </div>
  );
}

function StatePill({
  state,
  compact = false,
}: {
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
      {SESSION_STATE_LABELS[state]}
    </div>
  );
}

function ConnectionPill({ state }: { state: ConnectionState }) {
  return (
    <div
      className={cn(
        'inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-sm',
        connectionClassName(state),
      )}
    >
      {state === 'open' ? <Wifi size={14} /> : state === 'error' ? <X size={14} /> : <WifiOff size={14} />}
      {connectionLabel(state)}
    </div>
  );
}

function hydrateTimeline(events: RemoteTimelineEvent[]): RemoteTimelineEvent[] {
  return [...events]
    .sort((left, right) => left.sequence - right.sequence)
    .reduce<RemoteTimelineEvent[]>((current, event) => appendTimelineEvent(current, event), []);
}

function appendTimelineEvent(
  current: RemoteTimelineEvent[],
  nextEvent: RemoteTimelineEvent,
): RemoteTimelineEvent[] {
  if (current.some((event) => event.sequence === nextEvent.sequence)) {
    return current;
  }

  if (nextEvent.detail.kind === 'message_committed') {
    const committedDetail = nextEvent.detail;
    const filtered = current.filter((event) => {
      if (event.detail.kind !== 'message_delta') {
        return true;
      }
      return !sameMessageStream(event.detail, committedDetail);
    });
    return [...filtered, nextEvent].sort((left, right) => left.sequence - right.sequence);
  }

  if (nextEvent.detail.kind === 'message_delta') {
    const deltaDetail = nextEvent.detail;
    const last = current[current.length - 1];
    if (last?.detail.kind === 'message_delta' && sameMessageStream(last.detail, deltaDetail)) {
      const merged: RemoteTimelineEvent = {
        ...nextEvent,
        detail: {
          ...deltaDetail,
          delta: `${last.detail.delta}${deltaDetail.delta}`,
        },
      };
      return [...current.slice(0, -1), merged];
    }
  }

  if (nextEvent.detail.kind === 'tool_progress') {
    const progressDetail = nextEvent.detail;
    const last = current[current.length - 1];
    if (last?.detail.kind === 'tool_progress' && sameToolProgress(last.detail, progressDetail)) {
      const mergedDelta = `${last.detail.delta ?? ''}${progressDetail.delta ?? ''}`.trim();
      const merged: RemoteTimelineEvent = {
        ...nextEvent,
        detail: {
          ...progressDetail,
          delta: mergedDelta || undefined,
          elapsed_time_seconds: progressDetail.elapsed_time_seconds ?? last.detail.elapsed_time_seconds,
        },
      };
      return [...current.slice(0, -1), merged];
    }
  }

  return [...current, nextEvent].sort((left, right) => left.sequence - right.sequence);
}

function sameMessageStream(
  left: Extract<RemoteTimelineEventDetail, { kind: 'message_delta' | 'message_committed' }>,
  right: Extract<RemoteTimelineEventDetail, { kind: 'message_delta' | 'message_committed' }>,
): boolean {
  if (left.message_id && right.message_id) {
    return left.message_id === right.message_id;
  }
  return left.role === right.role;
}

function sameToolProgress(
  left: Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }>,
  right: Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }>,
): boolean {
  const leftKey = left.tool_call_id ?? left.tool_name ?? '';
  const rightKey = right.tool_call_id ?? right.tool_name ?? '';
  return leftKey.length > 0 && leftKey === rightKey;
}

function sessionTitle(session: RemoteSessionRecord): string {
  const title = session.metadata.title?.trim();
  if (title) {
    return title;
  }
  return session.workspace_id;
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
): string {
  if (detail.kind === 'tool_started') {
    return `Started tool call ${detail.tool_call_id}.`;
  }
  if (detail.kind === 'tool_progress') {
    if (detail.delta) {
      return detail.delta;
    }
    if (detail.elapsed_time_seconds != null) {
      return `Elapsed ${detail.elapsed_time_seconds}s.`;
    }
    return 'Tool is still running.';
  }
  if (detail.summary) {
    return detail.summary;
  }
  return detail.is_error ? 'Tool failed without a summary.' : 'Tool completed.';
}

function approvalSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'approval_requested' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'approval_resolved' }>,
): string {
  if (detail.kind === 'approval_requested') {
    return `${detail.title} is waiting for a decision.`;
  }
  return `Approval ${detail.approval_id} is now ${detail.state}.`;
}

function artifactSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'artifact_created' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'artifact_manifest' }>,
): string {
  if (detail.kind === 'artifact_created') {
    return `${detail.name} (${detail.file_name}) published as ${formatBytes(detail.size_bytes)}.`;
  }
  return `${detail.artifact_ids.length} artifact reference(s) published to the session.`;
}

function sessionEventSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'session_created' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'session_state_changed' }>,
): string {
  if (detail.kind === 'session_created') {
    return `Session created for workspace ${detail.workspace_id}.`;
  }
  return `Session moved from ${SESSION_STATE_LABELS[detail.previous_state]} to ${SESSION_STATE_LABELS[detail.state]}.`;
}

function runnerEventSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'runner_registered' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'runner_heartbeat' }>,
): string {
  if (detail.kind === 'runner_registered') {
    return `Runner registered ${detail.workspace_ids.length} workspace(s) with a ${detail.lease_ttl_secs}s lease.`;
  }
  return `Runner heartbeat: ${detail.active_sessions} active, ${detail.queued_sessions} queued.`;
}

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

function connectionClassName(state: ConnectionState): string {
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

function connectionLabel(state: ConnectionState): string {
  switch (state) {
    case 'open':
      return 'Live';
    case 'connecting':
      return 'Connecting';
    case 'reconnecting':
      return 'Reconnecting';
    case 'error':
      return 'Stream Error';
    default:
      return 'Idle';
  }
}

function formatRelativeTime(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  const diffMinutes = Math.floor(diffMs / 60_000);
  if (diffMinutes < 1) {
    return 'just now';
  }
  if (diffMinutes < 60) {
    return `${diffMinutes}m ago`;
  }
  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) {
    return `${diffHours}h ago`;
  }
  return new Date(iso).toLocaleDateString();
}

function formatBytes(sizeBytes: number): string {
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`;
  }
  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`;
  }
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

function extractErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return String(error);
}
