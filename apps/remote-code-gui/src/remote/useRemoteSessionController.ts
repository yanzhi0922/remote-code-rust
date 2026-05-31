import {
  startTransition,
  useDeferredValue,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  clearRemoteAccessToken,
  clearRemoteActiveSessionId,
  clearRemotePairingContext,
  deriveUserKey,
  hydrateRemoteAuthTokensFromSecureStore,
  persistRemoteAccessToken,
  persistRemoteActiveSessionId,
  persistRemoteRefreshToken,
  resolveRemoteAccessToken,
  resolveRemoteActiveSessionId,
  resolveRemoteBaseUrl,
  resolveRemotePairingContext,
  stripRemoteSensitiveQueryParams,
} from '../lib/runtime';
import { downloadRemoteArtifact } from '../lib/fileDownload';
import { shareFile } from '../lib/mobile/fileDownload';
import { initAppLifecycle } from '../lib/mobile/appLifecycle';
import { initDeepLinks, parsePairingUrl } from '../lib/mobile/deepLink';
import {
  initPushNotifications,
  registerPushTokenWithControlPlane,
  showLocalNotification,
} from '../lib/mobile/pushNotifications';
import { appendRemoteTimelineEvent } from '../session/normalize/fromRemote';
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
import type { TransportConfig } from './connection-manager';
import { extractErrorMessage } from './utils';
import {
  formatRemoteRelativeTime,
  getRemoteCopy,
  resolveRemoteLocale,
  type RemoteConnectionState,
} from './i18n';
import { loadRemoteSessionBundle } from './transport';
import {
  isDirectRunnerEnabled,
  resolveRemoteRunnerBaseUrl,
  resolveRemoteTransportStrategy,
} from './transportMode';
import { useConnection } from './useConnection';
import type {
  RemoteApprovalDecision,
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteSessionRecord,
  RemoteTimelineEvent,
} from './types';

export interface RemoteSessionControlStatus {
  canSendPrompt: boolean;
  canInterrupt: boolean;
  notice: string | null;
}

interface UseRemoteSessionControllerOptions {
  defaultDeviceName: string;
}

export function useRemoteSessionController({
  defaultDeviceName,
}: UseRemoteSessionControllerOptions) {
  const baseUrl = resolveRemoteBaseUrl();
  const locale = useMemo(() => resolveRemoteLocale(), []);
  const copy = useMemo(() => getRemoteCopy(locale), [locale]);
  const initialPairingContext = resolveRemotePairingContext();

  const [accessToken, setAccessToken] = useState<string | null>(() => resolveRemoteAccessToken());
  const [health, setHealth] = useState<Awaited<ReturnType<typeof getControlPlaneHealth>> | null>(null);
  const [authLoading, setAuthLoading] = useState(false);
  const [authErrorMessage, setAuthErrorMessage] = useState<string | null>(null);
  const [manualAccessToken, setManualAccessToken] = useState('');
  const [bootstrapSecret, setBootstrapSecret] = useState('');
  const [signInUsername, setSignInUsername] = useState('');
  const [signInPassword, setSignInPassword] = useState('');
  const [deviceName, setDeviceName] = useState(defaultDeviceName);
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
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const activeSessionIdRef = useRef<string | null>(null);
  const latestSequenceRef = useRef(0);
  const connectedSessionRef = useRef<string | null>(null);
  const sessionRefreshTimerRef = useRef<number | null>(null);
  const statusTimerRef = useRef<number | null>(null);
  const refreshInProgressRef = useRef(false);

  // Clean up status timer on unmount to prevent stale state updates.
  useEffect(() => {
    return () => {
      if (statusTimerRef.current !== null) {
        window.clearTimeout(statusTimerRef.current);
      }
    };
  }, []);

  const activeSession = useMemo(
    () => sessions.find((session) => session.session_id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );
  const selectedSessionId = activeSession?.session_id ?? null;
  const activeSessionControlStatus = useMemo(
    () => describeRemoteSessionControl(activeSession, locale, copy),
    [activeSession, copy, locale],
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

  useEffect(() => {
    activeSessionIdRef.current = selectedSessionId;
  }, [selectedSessionId]);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    if (accessToken || !baseUrl) {
      return;
    }
    let cancelled = false;
    void hydrateRemoteAuthTokensFromSecureStore().then((token) => {
      if (!cancelled && token) {
        setAccessToken(token);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [accessToken, baseUrl]);

  useEffect(() => {
    if (selectedSessionId) {
      persistRemoteActiveSessionId(baseUrl, selectedSessionId);
      return;
    }
    clearRemoteActiveSessionId(baseUrl);
  }, [baseUrl, selectedSessionId]);

  useEffect(() => {
    if (!baseUrl) {
      return;
    }
    let cancelled = false;
    const HEALTH_TIMEOUT_MS = 8000;
    const timer = setTimeout(() => {
      if (!cancelled) {
        // Timeout: set a fallback health so the UI exits "Contacting..."
        // and shows the auth screen where the user can configure the URL.
        setHealth({
          ok: false,
          service: baseUrl,
          phase: 'unreachable',
          runner_count: 0,
          available_runner_count: 0,
          session_count: 0,
          artifact_count: 0,
          queued_runner_command_count: 0,
          auth_required: true,
          bootstrap_secret_configured: false,
          owner_claimed: false,
          device_count: 0,
        } as Awaited<ReturnType<typeof getControlPlaneHealth>>);
        reportAsyncError(new Error(`Control plane health check timed out after ${HEALTH_TIMEOUT_MS}ms`));
      }
    }, HEALTH_TIMEOUT_MS);
    void getControlPlaneHealth(baseUrl)
      .then((response) => {
        clearTimeout(timer);
        if (!cancelled) {
          setHealth(response);
        }
      })
      .catch((error) => {
        clearTimeout(timer);
        if (!cancelled) {
          reportAsyncError(error);
          // On network error, still show the auth screen rather than
          // hanging on "Contacting..." forever.
          setHealth({
            ok: false,
            service: baseUrl,
            phase: 'unreachable',
            runner_count: 0,
            available_runner_count: 0,
            session_count: 0,
            artifact_count: 0,
            queued_runner_command_count: 0,
            auth_required: true,
            bootstrap_secret_configured: false,
            owner_claimed: false,
            device_count: 0,
          } as Awaited<ReturnType<typeof getControlPlaneHealth>>);
        }
      });
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [baseUrl, accessToken]);

  useEffect(() => {
    let cancelled = false;
    let unlistenDeepLinks: (() => void) | undefined;
    void initDeepLinks((url) => {
      if (cancelled) return;
      const pairing = parsePairingUrl(url);
      if (pairing) {
        setPairingOfferId(pairing.offerId);
        setPairingSecret(pairing.secret);
        showStatusMessage(copy.deepLinkPairingReceived);
      }
    }).then((unlisten) => {
      unlistenDeepLinks = unlisten;
    });
    return () => {
      cancelled = true;
      unlistenDeepLinks?.();
    };
  }, [copy]);

  useEffect(() => {
    if (!accessToken || !baseUrl) return;

    let cancelled = false;
    void (async () => {
      await initPushNotifications({
        onApproval: (approvalId, sessionId) => {
          if (cancelled) return;
          if (sessionId === activeSessionIdRef.current) {
            void refreshApprovals(sessionId).catch(reportAsyncError);
          }
          void showLocalNotification(
            copy.pushNotificationApprovalTitle,
            copy.pushNotificationApprovalBody(approvalId),
          );
          scheduleSessionsRefresh();
        },
        onSessionUpdate: (sessionId) => {
          if (cancelled) return;
          scheduleSessionsRefresh();
          if (sessionId === activeSessionIdRef.current) {
            void refreshSessionBundle(sessionId).catch(reportAsyncError);
          }
        },
      });

      if (!cancelled) {
        const registered = await registerPushTokenWithControlPlane(baseUrl, accessToken);
        if (!cancelled && !registered) {
          showStatusMessage(copy.mobileNotificationsUnavailable);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [accessToken, baseUrl, copy]);

  useEffect(() => {
    const cleanup = initAppLifecycle({
      onResume: () => {
        void refreshSessions().catch(reportAsyncError);
        if (activeSessionIdRef.current) {
          void refreshSessionBundle(activeSessionIdRef.current).catch(reportAsyncError);
        }
      },
    });
    return cleanup;
  }, []);

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
    setSignInUsername('');
    setSignInPassword('');
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
      setHealth(await getControlPlaneHealth(baseUrl));
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
      const response = await acceptPairingOffer(baseUrl, pairingOfferId, pairingSecret, deviceName);
      completeAuthentication(response.access_token, copy.statusPairingSucceeded, response.refresh_token);
      setHealth(await getControlPlaneHealth(baseUrl));
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setAuthLoading(false);
    }
  });

  const handleManualTokenSave = useEffectEvent(() => {
    const normalized = manualAccessToken.trim();
    if (!normalized) {
      return;
    }
    persistRemoteAccessToken(normalized);
    void clearRemotePairingContext();
    stripRemoteSensitiveQueryParams();
    setBootstrapSecret('');
    setPairingOfferId('');
    setPairingSecret('');
    setManualAccessToken('');
    setSignInUsername('');
    setSignInPassword('');
    setAccessToken(normalized);
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
    setSignInUsername('');
    setSignInPassword('');
    setAuthErrorMessage(null);
    showStatusMessage(copy.statusClearedAccessToken);
  });

  const handleUserSignIn = useEffectEvent(async () => {
    if (!baseUrl || authLoading || !signInUsername.trim() || !signInPassword.trim()) {
      return;
    }
    setAuthLoading(true);
    try {
      const userKey = await deriveUserKey(signInUsername.trim(), signInPassword.trim());
      completeAuthentication(userKey, copy.statusSignInSucceeded);
      setHealth(await getControlPlaneHealth(baseUrl));
    } catch (error) {
      reportAsyncError(error);
    } finally {
      setAuthLoading(false);
    }
  });

  const refreshSessions = useEffectEvent(async () => {
    if (!baseUrl || !health || (authRequired && !accessToken)) {
      return;
    }
    if (refreshInProgressRef.current) return;
    refreshInProgressRef.current = true;
    setSessionsLoading((current) => (sessions.length === 0 ? true : current));
    try {
      const response = await listSessions(baseUrl);
      const nextSessions = [...response.items].sort(
        (left, right) => new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime(),
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
      refreshInProgressRef.current = false;
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
        (left, right) => new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime(),
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
        (left, right) => new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
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

  const handleTransportEvent = useEffectEvent((event: RemoteTimelineEvent) => {
    const sessionId = connectedSessionRef.current;
    if (!sessionId) {
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

  useEffect(() => {
    void refreshSessions().catch(reportAsyncError);
    const intervalId = window.setInterval(() => {
      void refreshSessions().catch(reportAsyncError);
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
          const runnerBaseUrl = resolveRemoteRunnerBaseUrl(activeSession);
          const config: TransportConfig = {
            strategy: resolveRemoteTransportStrategy(activeSession),
            baseUrl,
            runnerBaseUrl,
            allowDirectRunner: isDirectRunnerEnabled(),
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
  }, [accessToken, activeSession?.owner_runner_id, authRequired, baseUrl, health, selectedSessionId]);

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
      await sendPrompt(baseUrl, selectedSessionId, composer.trim(), resolveRemoteRunnerBaseUrl(activeSession) ?? undefined);
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
      await interruptSession(baseUrl, selectedSessionId, resolveRemoteRunnerBaseUrl(activeSession) ?? undefined);
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
      await respondToApproval(baseUrl, approvalId, decision, undefined, resolveRemoteRunnerBaseUrl(activeSession) ?? undefined);
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
      return null;
    }

    setDownloadingArtifactId(artifact.artifact_id);
    try {
      const filePath = await downloadRemoteArtifact({
        url: buildArtifactDownloadUrl(baseUrl, artifact.artifact_id),
        fileName: artifact.file_name,
        token: accessToken,
      });
      showStatusMessage(copy.statusArtifactDownloaded(artifact.file_name));
      return filePath;
    } catch (error) {
      reportAsyncError(error);
      return null;
    } finally {
      setDownloadingArtifactId(null);
    }
  };

  const handleArtifactShare = async (artifact: RemoteArtifactRecord) => {
    if (!baseUrl || (authRequired && !accessToken)) return;
    try {
      const filePath = await handleArtifactDownload(artifact);
      if (filePath) {
        await shareFile(filePath, artifact.file_name);
      }
    } catch (error) {
      reportAsyncError(error);
    }
  };

  return {
    baseUrl,
    locale,
    copy,
    accessToken,
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
    events,
    deferredEvents,
    eventsLoading,
    approvals,
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
    authRequired,
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
  };
}

export function describeRemoteSessionControl(
  session: RemoteSessionRecord | null,
  locale: ReturnType<typeof resolveRemoteLocale>,
  copy: ReturnType<typeof getRemoteCopy>,
): RemoteSessionControlStatus {
  if (!session) {
    return { canSendPrompt: false, canInterrupt: false, notice: null };
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
  return {
    canSendPrompt: true,
    canInterrupt: true,
    notice: null,
  };
}
