import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import RemoteApp from './RemoteApp';

const mockApi = vi.hoisted(() => ({
  acceptPairingOffer: vi.fn(),
  bootstrapControlPlane: vi.fn(),
  buildArtifactDownloadUrl: vi.fn(() => '#artifact'),
  buildSessionEventsStreamUrl: vi.fn(() => 'wss://example.test/events'),
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

const mockRuntime = vi.hoisted(() => ({
  clearRemoteActiveSessionId: vi.fn(),
  clearRemoteAccessToken: vi.fn(),
  persistRemoteActiveSessionId: vi.fn(),
  persistRemoteAccessToken: vi.fn(),
  resolveRemoteActiveSessionId: vi.fn<() => string | null>(() => null),
  resolveRemoteAccessToken: vi.fn(() => 'token'),
  resolveRemoteBaseUrl: vi.fn(() => 'https://remote-code.yz520gzy.top'),
  resolveRemotePairingContext: vi.fn(() => ({ offerId: null, pairingSecret: null })),
  stripRemoteSensitiveQueryParams: vi.fn(),
}));

vi.mock('./api', () => mockApi);
vi.mock('../lib/fileDownload', () => mockFileDownload);
vi.mock('../lib/runtime', () => mockRuntime);

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

const SESSION_RESPONSE = {
  items: [
    {
      session_id: 'session-1',
      workspace_id: 'workspace-main',
      owner_runner_id: 'win-main',
      owner_runner_available: true,
      owner_runner_state: 'busy' as const,
      owner_runner_last_seen_at: '2026-04-13T00:05:00Z',
      state: 'running' as const,
      metadata: { title: 'Alpha Session' },
      created_at: '2026-04-13T00:00:00Z',
      updated_at: '2026-04-13T00:05:00Z',
    },
  ],
};

class MockWebSocket {
  url: string;
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    queueMicrotask(() => {
      this.onopen?.();
    });
  }

  close() {
    // no-op for tests
  }
}

describe('RemoteApp', () => {
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
    mockApi.listSessions.mockResolvedValue(SESSION_RESPONSE);
    mockApi.listSessionEvents.mockResolvedValue({ items: [], latest_sequence: 0 });
    mockApi.listSessionApprovals.mockResolvedValue({ items: [] });
    mockApi.listSessionArtifacts.mockResolvedValue({ items: [] });

    vi.stubGlobal('WebSocket', MockWebSocket);
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('renders Chinese mobile copy when the browser prefers Chinese', async () => {
    Object.defineProperty(window.navigator, 'language', {
      configurable: true,
      value: 'zh-CN',
    });
    Object.defineProperty(window.navigator, 'languages', {
      configurable: true,
      value: ['zh-CN', 'en-US'],
    });

    render(<RemoteApp />);

    expect(await screen.findByText('远程控制')).toBeInTheDocument();
    expect(screen.getByText('刷新会话')).toBeInTheDocument();
    expect(document.documentElement.lang).toBe('zh-CN');
  });

  it('does not enter a session polling loop after the first authenticated render', async () => {
    render(<RemoteApp />);

    await screen.findByText('Refresh Sessions');
    await waitFor(() => {
      expect(mockApi.listSessions).toHaveBeenCalledTimes(1);
      expect(mockApi.listSessionEvents).toHaveBeenCalledTimes(1);
      expect(mockApi.listSessionApprovals).toHaveBeenCalledTimes(1);
      expect(mockApi.listSessionArtifacts).toHaveBeenCalledTimes(1);
    });
  });

  it('renders approvals and artifacts, forwards approval decisions, and downloads securely', async () => {
    mockApi.listSessionApprovals.mockResolvedValue({
      items: [
        {
          approval_id: 'approval-1',
          session_id: 'session-1',
          runner_id: 'win-main',
          state: 'pending',
          title: 'Approve shell command',
          description: 'Run git status before continuing.',
          metadata: {
            blocked_path: 'C:\\repo',
          },
          created_at: '2026-04-13T00:01:00Z',
          updated_at: '2026-04-13T00:02:00Z',
          responded_at: null,
          responder: null,
          note: null,
        },
      ],
    });
    mockApi.listSessionArtifacts.mockResolvedValue({
      items: [
        {
          artifact_id: 'artifact-1',
          session_id: 'session-1',
          runner_id: 'win-main',
          name: 'Transcript',
          file_name: 'session.md',
          media_type: 'text/markdown',
          size_bytes: 1024,
          metadata: {},
          created_at: '2026-04-13T00:03:00Z',
        },
      ],
    });
    mockApi.respondToApproval.mockResolvedValue(undefined);
    mockApi.buildArtifactDownloadUrl.mockReturnValue('https://example.test/artifact-1');
    mockFileDownload.downloadRemoteArtifact.mockResolvedValue(undefined);

    render(<RemoteApp />);

    expect((await screen.findAllByText('Alpha Session')).length).toBeGreaterThan(0);
    expect(await screen.findByText('Approve shell command')).toBeInTheDocument();
    expect(screen.getByText('Run git status before continuing.')).toBeInTheDocument();
    expect(screen.getByText('Transcript')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));

    await waitFor(() => {
      expect(mockApi.respondToApproval).toHaveBeenCalledWith(
        'https://remote-code.yz520gzy.top',
        'approval-1',
        'approved',
      );
    });

    fireEvent.click(screen.getByRole('button', { name: /Transcript/i }));

    await waitFor(() => {
      expect(mockFileDownload.downloadRemoteArtifact).toHaveBeenCalledWith({
        url: 'https://example.test/artifact-1',
        fileName: 'session.md',
        token: 'token',
      });
    });
  });

  it('forwards follow-up prompts and clears the composer', async () => {
    mockApi.sendPrompt.mockResolvedValue({
      session_id: 'session-1',
      accepted: true,
      message: 'queued',
    });

    render(<RemoteApp />);

    const composer = await screen.findByPlaceholderText(
      'Send a follow-up prompt to the local runner. Shift+Enter inserts a newline.',
    );

    fireEvent.change(composer, {
      target: { value: '请只回复 中文链路正常-0413，不要附加任何其他内容。' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => {
      expect(mockApi.sendPrompt).toHaveBeenCalledWith(
        'https://remote-code.yz520gzy.top',
        'session-1',
        '请只回复 中文链路正常-0413，不要附加任何其他内容。',
      );
    });

    expect(composer).toHaveValue('');
  });

  it('forwards interrupt requests for the active session', async () => {
    mockApi.interruptSession.mockResolvedValue({
      session_id: 'session-1',
      accepted: true,
      message: 'queued',
    });

    render(<RemoteApp />);

    fireEvent.click(await screen.findByRole('button', { name: 'Interrupt' }));

    await waitFor(() => {
      expect(mockApi.interruptSession).toHaveBeenCalledWith(
        'https://remote-code.yz520gzy.top',
        'session-1',
      );
    });
  });

  it('disables follow-up controls when the owner runner is offline', async () => {
    mockApi.listSessions.mockResolvedValue({
      items: [
        {
          ...SESSION_RESPONSE.items[0],
          owner_runner_available: false,
          owner_runner_last_seen_at: '2026-04-13T00:04:00Z',
        },
      ],
    });

    render(<RemoteApp />);

    expect(
      await screen.findByText(/Runner win-main is currently offline/i),
    ).toBeInTheDocument();

    const composer = screen.getByPlaceholderText(
      'Send a follow-up prompt to the local runner. Shift+Enter inserts a newline.',
    );
    expect(composer).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Interrupt' })).toBeDisabled();
    expect(screen.getByText(/win-main · offline/i)).toBeInTheDocument();
  });

  it('restores the previously selected session after a reload', async () => {
    mockRuntime.resolveRemoteActiveSessionId.mockReturnValue('session-2');
    mockApi.listSessions.mockResolvedValue({
      items: [
        {
          ...SESSION_RESPONSE.items[0],
          session_id: 'session-1',
          metadata: { title: 'Alpha Session' },
          updated_at: '2026-04-13T00:05:00Z',
        },
        {
          ...SESSION_RESPONSE.items[0],
          session_id: 'session-2',
          workspace_id: 'workspace-secondary',
          metadata: { title: 'Beta Session' },
          updated_at: '2026-04-13T00:04:00Z',
        },
      ],
    });

    render(<RemoteApp />);

    await waitFor(() => {
      expect(mockApi.listSessionEvents).toHaveBeenCalledWith(
        'https://remote-code.yz520gzy.top',
        'session-2',
      );
    });

    expect(mockRuntime.persistRemoteActiveSessionId).toHaveBeenCalledWith(
      'https://remote-code.yz520gzy.top',
      'session-2',
    );
  });

  it('persists the new active session when the user switches sessions', async () => {
    mockApi.listSessions.mockResolvedValue({
      items: [
        {
          ...SESSION_RESPONSE.items[0],
          session_id: 'session-1',
          metadata: { title: 'Alpha Session' },
          updated_at: '2026-04-13T00:05:00Z',
        },
        {
          ...SESSION_RESPONSE.items[0],
          session_id: 'session-2',
          workspace_id: 'workspace-secondary',
          metadata: { title: 'Beta Session' },
          updated_at: '2026-04-13T00:04:00Z',
        },
      ],
    });

    render(<RemoteApp />);

    fireEvent.click(await screen.findByRole('button', { name: /Beta Session/i }));

    await waitFor(() => {
      expect(mockRuntime.persistRemoteActiveSessionId).toHaveBeenCalledWith(
        'https://remote-code.yz520gzy.top',
        'session-2',
      );
    });
  });

  it('renders mobile floating action buttons and opens approval bottom sheet on click', async () => {
    mockApi.listSessionApprovals.mockResolvedValue({
      items: [
        {
          approval_id: 'appr-1',
          session_id: 'session-1',
          state: 'pending',
          title: 'Run script.sh',
          description: 'Shell execution',
          created_at: '2026-04-13T00:10:00Z',
          metadata: { blocked_path: '/home/user/script.sh' },
        },
      ],
    });
    mockApi.listSessionArtifacts.mockResolvedValue({ items: [] });

    render(<RemoteApp />);

    // Wait for the session to load (appears in sidebar card + header)
    const sessionLabels = await screen.findAllByText('Alpha Session');
    expect(sessionLabels.length).toBeGreaterThanOrEqual(2);

    // The floating action buttons should be present (lg:hidden is visible in jsdom)
    const approvalFab = screen.getByRole('button', { name: /Pending Approvals/i });
    expect(approvalFab).toBeTruthy();

    const artifactFab = screen.getByRole('button', { name: /Artifacts/i });
    expect(artifactFab).toBeTruthy();

    // Click the approval FAB to open the bottom sheet
    fireEvent.click(approvalFab);

    // The Dialog title should appear
    await waitFor(() => {
      const dialogTitles = screen.getAllByText(/Pending Approvals/i);
      expect(dialogTitles.length).toBeGreaterThanOrEqual(2);
    });
  });
});
