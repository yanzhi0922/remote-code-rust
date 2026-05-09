import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { RemoteShell } from './RemoteShell';
import type { RemoteCopy } from './i18n';
import type { RemoteSessionRecord } from './types';

// Minimal copy subset used by RemoteShell
const COPY = {
  remoteShellEyebrow: 'Remote',
  remoteShellDescription: 'Control your runners',
  refreshSessions: 'Refresh',
  loadingRemoteSessions: 'Loading...',
  noSessionsTitle: 'No sessions',
  noSessionsDescription: 'Create one from your desktop',
  selectRemoteSession: 'Select session',
  runnerUnassigned: 'Unassigned',
  runnerOfflineLabel: 'offline',
  sessionStateLabels: {
    running: 'Running',
    waiting_approval: 'Waiting',
    completed: 'Done',
    failed: 'Failed',
    cancelled: 'Cancelled',
  } as Record<string, string>,
  connectionLabels: {
    idle: 'Idle',
    connecting: 'Connecting',
    open: 'Connected',
    reconnecting: 'Reconnecting',
    error: 'Error',
  } as Record<string, string>,
} as unknown as RemoteCopy;

const SESSION: RemoteSessionRecord = {
  session_id: 'session-1',
  workspace_id: 'workspace-main',
  owner_runner_id: 'runner-1',
  owner_runner_available: true,
  owner_runner_state: 'busy',
  owner_runner_last_seen_at: '2026-04-14T10:00:00Z',
  state: 'running',
  metadata: { title: 'Alpha Session' },
  created_at: '2026-04-14T09:00:00Z',
  updated_at: '2026-04-14T10:00:00Z',
};

describe('RemoteShell', () => {
  afterEach(() => {
    cleanup();
  });

  it('renders sessions in the sidebar and calls onSelectSession on click', () => {
    const onSelect = vi.fn();
    render(
      <RemoteShell
        sessions={[SESSION]}
        sessionsLoading={false}
        activeSessionId="session-1"
        activeSession={SESSION}
        connectionState="open"
        sidebarOpen={true}
        errorMessage={null}
        statusMessage={null}
        baseUrl="https://remote-code.test"
        copy={COPY}
        locale="en"
        onToggleSidebar={() => {}}
        onSelectSession={onSelect}
        onRefreshSessions={() => {}}
        onSignOut={() => {}}
        transportStrategy={null}
        transportLatencyMs={null}
      >
        <div data-testid="child">main content</div>
      </RemoteShell>,
    );

    // "Alpha Session" appears in both the sidebar card and the header title
    const sessionTitles = screen.getAllByText('Alpha Session');
    expect(sessionTitles.length).toBeGreaterThanOrEqual(2);
    expect(screen.getByTestId('child')).toBeInTheDocument();

    // Click the sidebar session card (first occurrence)
    fireEvent.click(sessionTitles[0]);
    expect(onSelect).toHaveBeenCalledWith('session-1');
  });

  it('shows loading state and empty state for sessions', () => {
    const { rerender } = render(
      <RemoteShell
        sessions={[]}
        sessionsLoading={true}
        activeSessionId={null}
        activeSession={null}
        connectionState="idle"
        sidebarOpen={false}
        errorMessage={null}
        statusMessage={null}
        baseUrl="https://remote-code.test"
        copy={COPY}
        locale="en"
        onToggleSidebar={() => {}}
        onSelectSession={() => {}}
        onRefreshSessions={() => {}}
        onSignOut={() => {}}
        transportStrategy={null}
        transportLatencyMs={null}
      >
        <div />
      </RemoteShell>,
    );

    expect(screen.getByText('Loading...')).toBeInTheDocument();

    rerender(
      <RemoteShell
        sessions={[]}
        sessionsLoading={false}
        activeSessionId={null}
        activeSession={null}
        connectionState="idle"
        sidebarOpen={false}
        errorMessage={null}
        statusMessage={null}
        baseUrl="https://remote-code.test"
        copy={COPY}
        locale="en"
        onToggleSidebar={() => {}}
        onSelectSession={() => {}}
        onRefreshSessions={() => {}}
        onSignOut={() => {}}
        transportStrategy={null}
        transportLatencyMs={null}
      >
        <div />
      </RemoteShell>,
    );

    expect(screen.getByText('No sessions')).toBeInTheDocument();
  });

  it('renders error and status banners when provided', () => {
    render(
      <RemoteShell
        sessions={[]}
        sessionsLoading={false}
        activeSessionId={null}
        activeSession={null}
        connectionState="open"
        sidebarOpen={false}
        errorMessage="Server unreachable"
        statusMessage="Session restored"
        baseUrl="https://remote-code.test"
        copy={COPY}
        locale="en"
        onToggleSidebar={() => {}}
        onSelectSession={() => {}}
        onRefreshSessions={() => {}}
        onSignOut={() => {}}
        transportStrategy={null}
        transportLatencyMs={null}
      >
        <div />
      </RemoteShell>,
    );

    expect(screen.getByText('Server unreachable')).toBeInTheDocument();
    expect(screen.getByText('Session restored')).toBeInTheDocument();
  });

  it('calls onRefreshSessions when the refresh button is clicked', () => {
    const onRefresh = vi.fn();
    render(
      <RemoteShell
        sessions={[]}
        sessionsLoading={false}
        activeSessionId={null}
        activeSession={null}
        connectionState="idle"
        sidebarOpen={false}
        errorMessage={null}
        statusMessage={null}
        baseUrl="https://remote-code.test"
        copy={COPY}
        locale="en"
        onToggleSidebar={() => {}}
        onSelectSession={() => {}}
        onRefreshSessions={onRefresh}
        onSignOut={vi.fn()}
        transportStrategy={null}
        transportLatencyMs={null}
      >
        <div />
      </RemoteShell>,
    );

    fireEvent.click(screen.getByText('Refresh'));
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });
});
