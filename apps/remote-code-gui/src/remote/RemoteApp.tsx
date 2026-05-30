import * as Dialog from '@radix-ui/react-dialog';
import {
  FileOutput,
  LoaderCircle,
  MessageSquareText,
  Shield,
  Square,
  X,
} from 'lucide-react';
import {
  useMemo,
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
import { ApprovalPanel } from '../components/shared/ApprovalPanel';
import { ArtifactPanel } from '../components/shared/ArtifactPanel';
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
import { RemoteAuthGate } from './RemoteAuthGate';
import { RemoteShell, EmptyCard } from './RemoteShell';
import { isDirectRunnerEnabled, resolveRemoteRunnerBaseUrl, resolveRemoteTransportStrategy } from './transportMode';
import { useRemoteSessionController } from './useRemoteSessionController';
import { extractErrorMessage } from './utils';
import { TimelineCard, describeSessionControl } from './RemoteTimelineShared';
import type { TransportConfig } from './connection-manager';
import type {
  RemoteApprovalDecision,
  RemoteApprovalRecord,
  RemoteArtifactRecord,
  RemoteControlPlaneHealth,
  RemoteSessionRecord,
} from './types';


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
    className: 'bg-rc-bg-active text-rc-text-secondary hover:bg-rc-bg-active',
  },
];

// ═══════════════════════════════════════════════════════════════════════════
// RemoteApp — thin orchestrator
// ═══════════════════════════════════════════════════════════════════════════

export default function RemoteApp() {
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
  } = useRemoteSessionController({ defaultDeviceName: 'Mobile Browser' });

  const [sidebarOpen, setSidebarOpen] = useState(false);
  const approvalActions = useMemo(
    () =>
      APPROVAL_DECISIONS.map((item) => ({
        ...item,
        label: copy.approvalDecisionLabels[item.decision],
      })),
    [copy],
  );
  // ── Render: early exits ────────────────────────────────────────────────

  if (!baseUrl) {
    return (
      <div className="min-h-screen bg-rc-bg-base text-rc-text-primary">
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
      <div className="min-h-screen bg-rc-bg-base text-rc-text-primary">
        <div className="flex min-h-screen items-center justify-center px-6">
          <div role="status" className="flex items-center gap-3 rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-5 py-4 text-sm text-rc-text-secondary shadow-[0_18px_45px_rgba(52,45,34,0.08)]">
            <LoaderCircle size={16} className="animate-spin" />
            {copy.contactingControlPlane}
          </div>
        </div>
      </div>
    );
  }

  if (showAuthGate) {
    return (
      <div className="min-h-screen bg-rc-bg-base text-rc-text-primary">
        <RemoteAuthGate
          authErrorMessage={authErrorMessage}
          authLoading={authLoading}
          bootstrapEnabled={!health.owner_claimed && health.bootstrap_secret_configured}
          copy={copy}
          deviceName={deviceName}
          health={health}
          manualAccessToken={manualAccessToken}
          username={signInUsername}
          password={signInPassword}
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
          onUserSignIn={() => {
            void handleUserSignIn();
          }}
          pairingOfferId={pairingOfferId}
          pairingSecret={pairingSecret}
          setBootstrapSecret={setBootstrapSecret}
          setDeviceName={setDeviceName}
          setManualAccessToken={setManualAccessToken}
          setPairingOfferId={setPairingOfferId}
          setPairingSecret={setPairingSecret}
          setUsername={setSignInUsername}
          setPassword={setSignInPassword}
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
      onSignOut={handleClearSavedToken}
      transportStrategy={transportStrategy}
      transportLatencyMs={transportMetrics?.latencyMs ?? null}
    >
      <main className="grid min-h-0 flex-1 gap-0 lg:grid-cols-[minmax(0,1fr)_340px]">
        {/* ── Timeline + Composer ── */}
        <section className="flex min-h-0 flex-col border-b border-rc-border-primary bg-rc-bg-secondary lg:border-b-0 lg:border-r">
          {activeSession ? (
            <div className="flex min-h-0 flex-1 flex-col">
              <div className="flex-1 min-h-0">
                {eventsLoading ? (
                  <div className="flex min-h-[280px] items-center justify-center px-4 py-5 sm:px-6">
                    <div role="status" className="flex items-center gap-3 rounded-2xl bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-tertiary shadow-[0_12px_28px_rgba(34,32,28,0.06)]">
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

              <div className="border-t border-rc-border-primary bg-rc-bg-secondary px-4 py-4 sm:px-6">
                <div className="mx-auto max-w-5xl rounded-3xl border border-rc-border-primary bg-rc-bg-surface shadow-[0_18px_44px_rgba(34,32,28,0.09)]">
                  <div className="flex items-center justify-between gap-3 border-b border-rc-border-primary px-4 py-3">
                    <div className="inline-flex items-center gap-2 text-sm text-rc-text-secondary">
                      <MessageSquareText size={15} />
                      {copy.followUpControl}
                    </div>
                    <button
                      type="button"
                      onClick={() => {
                        void handleInterrupt();
                      }}
                      disabled={interrupting || !activeSessionControlStatus.canInterrupt}
                      className="inline-flex items-center gap-2 rounded-full border border-rc-border-primary bg-rc-bg-hover px-3 py-1.5 text-sm text-rc-text-secondary transition-colors hover:bg-[#f3ebdf] disabled:cursor-not-allowed disabled:opacity-60"
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
                      className="min-h-[88px] flex-1 resize-none bg-transparent text-[15px] leading-6 text-rc-text-primary outline-none placeholder:text-rc-text-tertiary disabled:cursor-not-allowed disabled:text-rc-text-tertiary"
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
        <aside className="border-t border-rc-border-primary bg-rc-bg-secondary lg:border-t-0">
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
              onShare={(artifact) => {
                const record = artifacts.find((item) => item.artifact_id === artifact.artifact_id);
                if (!record) {
                  return;
                }
                void handleArtifactShare(record);
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
              <Dialog.Content className="fixed inset-x-0 bottom-0 z-50 max-h-[80vh] rounded-t-[28px] border-t border-rc-border-primary bg-rc-bg-surface px-5 py-5 shadow-[0_-12px_40px_rgba(34,32,28,0.12)] focus:outline-none data-[state=open]:animate-in data-[state=open]:slide-in-from-bottom data-[state=closed]:animate-out data-[state=closed]:slide-out-to-bottom">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <Dialog.Title className="text-lg font-semibold text-rc-text-primary">
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
                      className="inline-flex h-9 w-9 items-center justify-center rounded-2xl border border-rc-border-primary bg-rc-bg-hover text-rc-text-secondary transition-colors hover:bg-rc-bg-surface"
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
              <Dialog.Content className="fixed inset-x-0 bottom-0 z-50 max-h-[80vh] rounded-t-[28px] border-t border-rc-border-primary bg-rc-bg-surface px-5 py-5 shadow-[0_-12px_40px_rgba(34,32,28,0.12)] focus:outline-none data-[state=open]:animate-in data-[state=open]:slide-in-from-bottom data-[state=closed]:animate-out data-[state=closed]:slide-out-to-bottom">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <Dialog.Title className="text-lg font-semibold text-rc-text-primary">
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
                      className="inline-flex h-9 w-9 items-center justify-center rounded-2xl border border-rc-border-primary bg-rc-bg-hover text-rc-text-secondary transition-colors hover:bg-rc-bg-surface"
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
                    onShare={(artifact) => {
                      const record = artifacts.find((item) => item.artifact_id === artifact.artifact_id);
                      if (!record) {
                        return;
                      }
                      void handleArtifactShare(record);
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
