import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import MobileRemoteApp from './MobileRemoteApp';

const mockApi = vi.hoisted(() => ({
  acceptPairingOffer: vi.fn(),
  bootstrapControlPlane: vi.fn(),
  buildArtifactDownloadUrl: vi.fn(() => 'https://example.test/artifact-1'),
  buildSessionEventsStreamUrl: vi.fn(() => 'wss://example.test/events'),
  createStreamTicket: vi.fn(() => Promise.resolve({ stream_ticket: 'rcst_test', expires_in_secs: 45 })),
  getControlPlaneHealth: vi.fn(),
  interruptSession: vi.fn(),
  listSessionApprovals: vi.fn(),
  listSessionArtifacts: vi.fn(),
  listSessionEvents: vi.fn(),
  listSessions: vi.fn(),
  respondToApproval: vi.fn(),
  sendPrompt: vi.fn(),
}));

const mockFileDownload = vi.hoisted(() => ({
  downloadRemoteArtifact: vi.fn(),
}));

const mockMobileFileDownload = vi.hoisted(() => ({
  shareFile: vi.fn(),
}));

const mockPush = vi.hoisted(() => ({
  initPushNotifications: vi.fn(() => Promise.resolve()),
  registerPushTokenWithControlPlane: vi.fn(() => Promise.resolve(true)),
  showLocalNotification: vi.fn(() => Promise.resolve()),
}));

const mockDeepLink = vi.hoisted(() => ({
  initDeepLinks: vi.fn(() => Promise.resolve()),
  parsePairingUrl: vi.fn(() => null),
}));

const mockLifecycle = vi.hoisted(() => ({
  initAppLifecycle: vi.fn(() => () => {}),
}));

const mockOfflineQueue = vi.hoisted(() => ({
  drainCommands: vi.fn(() => Promise.resolve([])),
  enqueueCommand: vi.fn(),
}));

const mockBiometric = vi.hoisted(() => ({
  getBiometricEnabled: vi.fn(() => Promise.resolve(false)),
  setBiometricEnabled: vi.fn(() => Promise.resolve(true)),
}));

const mockRuntime = vi.hoisted(() => ({
  clearRemoteActiveSessionId: vi.fn(),
  clearRemoteAccessToken: vi.fn(),
  clearRemotePairingContext: vi.fn(),
  deriveUserKey: vi.fn(() => Promise.resolve('derived-user-key')),
  hydrateRemoteAuthTokensFromSecureStore: vi.fn(() => Promise.resolve(null)),
  persistRemoteActiveSessionId: vi.fn(),
  persistRemoteAccessToken: vi.fn(),
  persistRemoteRefreshToken: vi.fn(),
  resolveRemoteActiveSessionId: vi.fn<() => string | null>(() => null),
  resolveRemoteAccessToken: vi.fn<() => string | null>(() => 'token'),
  resolveRemoteBaseUrl: vi.fn<() => string | null>(() => 'https://remote-code.test'),
  resolveRemotePairingContext: vi.fn(() => ({ offerId: null, pairingSecret: null })),
  stripRemoteSensitiveQueryParams: vi.fn(),
}));

vi.mock('./api', () => mockApi);
vi.mock('../lib/fileDownload', () => mockFileDownload);
vi.mock('../lib/mobile/fileDownload', () => mockMobileFileDownload);
vi.mock('../lib/runtime', () => mockRuntime);
vi.mock('../lib/mobile/pushNotifications', () => mockPush);
vi.mock('../lib/mobile/deepLink', () => mockDeepLink);
vi.mock('../lib/mobile/appLifecycle', () => mockLifecycle);
vi.mock('./offline-queue', () => mockOfflineQueue);
vi.mock('../lib/mobile/biometric', () => mockBiometric);

const HEALTH_RESPONSE = {
  ok: true,
  service: 'remote-code-control-plane',
  phase: 'phase3',
  runner_count: 1,
  available_runner_count: 1,
  session_count: 1,
  artifact_count: 0,
  queued_runner_command_count: 0,
  auth_required: true,
  bootstrap_secret_configured: true,
  owner_claimed: true,
  device_count: 3,
};

const SESSION = {
  session_id: 'session-1',
  workspace_id: 'workspace-main',
  owner_runner_id: 'runner-main',
  owner_runner_available: true,
  owner_runner_state: 'busy' as const,
  owner_runner_last_seen_at: '2026-04-13T00:05:00Z',
  state: 'running' as const,
  metadata: { title: 'Alpha Session' },
  created_at: '2026-04-13T00:00:00Z',
  updated_at: '2026-04-13T00:05:00Z',
};

class MockWebSocket {
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;

  constructor() {
    queueMicrotask(() => {
      this.onopen?.();
    });
  }

  close() {
    this.onclose?.();
  }
}

async function openTimeline() {
  fireEvent.click(await screen.findByRole('button', { name: /Alpha Session/i }));
  return screen.findByPlaceholderText(
    'Send a follow-up prompt to the local runner. Shift+Enter inserts a newline.',
  );
}

describe('MobileRemoteApp', () => {
  beforeEach(() => {
    document.documentElement.lang = '';
    Object.defineProperty(window.navigator, 'language', {
      configurable: true,
      value: 'en-US',
    });
    Object.defineProperty(window.navigator, 'languages', {
      configurable: true,
      value: ['en-US'],
    });

    mockApi.getControlPlaneHealth.mockResolvedValue(HEALTH_RESPONSE);
    mockApi.listSessions.mockResolvedValue({ items: [SESSION] });
    mockApi.listSessionEvents.mockResolvedValue({ items: [], latest_sequence: 0 });
    mockApi.listSessionApprovals.mockResolvedValue({ items: [] });
    mockApi.listSessionArtifacts.mockResolvedValue({ items: [] });
    mockApi.sendPrompt.mockResolvedValue({ session_id: 'session-1', accepted: true });
    mockApi.respondToApproval.mockResolvedValue(undefined);
    mockFileDownload.downloadRemoteArtifact.mockResolvedValue('C:\\tmp\\session.md');
    mockRuntime.resolveRemoteAccessToken.mockReturnValue('token');
    mockRuntime.resolveRemoteActiveSessionId.mockReturnValue(null);
    mockRuntime.deriveUserKey.mockResolvedValue('derived-user-key');

    vi.stubGlobal('WebSocket', MockWebSocket);
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('authenticates a mobile device with username and password', async () => {
    mockRuntime.resolveRemoteAccessToken.mockReturnValue(null);

    render(<MobileRemoteApp />);

    fireEvent.change(await screen.findByPlaceholderText('your-name'), {
      target: { value: 'alice' },
    });
    fireEvent.change(screen.getByPlaceholderText('your-password'), {
      target: { value: 'secret' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    await waitFor(() => {
      expect(mockRuntime.deriveUserKey).toHaveBeenCalledWith('alice', 'secret');
      expect(mockRuntime.persistRemoteAccessToken).toHaveBeenCalledWith('derived-user-key');
    });
  });

  it('selects a session and opens the mobile timeline composer', async () => {
    render(<MobileRemoteApp />);

    const composer = await openTimeline();

    expect(composer).toBeInTheDocument();
    expect(screen.getByText('Alpha Session')).toBeInTheDocument();
  });

  it('sends a follow-up prompt from the mobile timeline', async () => {
    render(<MobileRemoteApp />);

    const composer = await openTimeline();
    fireEvent.change(composer, { target: { value: 'Ship the mobile flow' } });
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => {
      expect(mockApi.sendPrompt).toHaveBeenCalledWith(
        'https://remote-code.test',
        'session-1',
        'Ship the mobile flow',
        undefined,
      );
    });
  });

  it('approves a pending mobile approval', async () => {
    mockApi.listSessionApprovals.mockResolvedValue({
      items: [
        {
          approval_id: 'approval-1',
          session_id: 'session-1',
          runner_id: 'runner-main',
          state: 'pending',
          title: 'Run deployment check',
          description: 'Needs shell access.',
          metadata: {},
          created_at: '2026-04-13T00:01:00Z',
          updated_at: '2026-04-13T00:02:00Z',
          responded_at: null,
          responder: null,
          note: null,
        },
      ],
    });

    render(<MobileRemoteApp />);
    expect((await screen.findAllByText('Alpha Session')).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole('tab', { name: /Approvals/ }));

    expect(await screen.findByText('Run deployment check')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));

    await waitFor(() => {
      expect(mockApi.respondToApproval).toHaveBeenCalledWith(
        'https://remote-code.test',
        'approval-1',
        'approved',
        undefined,
        undefined,
      );
    });
  });

  it('downloads a mobile artifact from the approvals tab', async () => {
    mockApi.listSessionArtifacts.mockResolvedValue({
      items: [
        {
          artifact_id: 'artifact-1',
          session_id: 'session-1',
          runner_id: 'runner-main',
          name: 'Transcript',
          file_name: 'session.md',
          media_type: 'text/markdown',
          size_bytes: 1024,
          metadata: {},
          created_at: '2026-04-13T00:03:00Z',
        },
      ],
    });

    render(<MobileRemoteApp />);
    expect((await screen.findAllByText('Alpha Session')).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole('tab', { name: /Approvals/ }));

    expect(await screen.findByText('session.md')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Rendering response...' }));

    await waitFor(() => {
      expect(mockFileDownload.downloadRemoteArtifact).toHaveBeenCalledWith({
        url: 'https://example.test/artifact-1',
        fileName: 'session.md',
        token: 'token',
      });
    });
  });
});
