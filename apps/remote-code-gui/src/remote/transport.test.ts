import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { loadRemoteSessionBundle, subscribeToRemoteSessionEvents } from './transport';

const mockApi = vi.hoisted(() => ({
  buildSessionEventsStreamUrl: vi.fn(() => 'wss://example.test/events'),
  listSessionApprovals: vi.fn(),
  listSessionArtifacts: vi.fn(),
  listSessionEvents: vi.fn(),
}));

vi.mock('./api', () => mockApi);

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static autoOpen = true;

  url: string;
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
    if (MockWebSocket.autoOpen) {
      queueMicrotask(() => {
        this.onopen?.();
      });
    }
  }

  close() {
    // no-op for tests
  }
}

describe('remote transport', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    MockWebSocket.autoOpen = true;
    vi.stubGlobal('WebSocket', MockWebSocket);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('loads a session bundle with hydrated events and sorted side panels', async () => {
    mockApi.listSessionEvents.mockResolvedValue({
      items: [
        {
          sequence: 1,
          recorded_at: '2026-04-14T09:00:01Z',
          runner_id: 'runner-1',
          session_id: 'session-1',
          detail: { kind: 'message_delta', role: 'assistant', delta: 'Hel', message_id: 'msg-1' },
        },
        {
          sequence: 2,
          recorded_at: '2026-04-14T09:00:02Z',
          runner_id: 'runner-1',
          session_id: 'session-1',
          detail: { kind: 'message_committed', role: 'assistant', text: 'Hello', message_id: 'msg-1' },
        },
      ],
      latest_sequence: 2,
    });
    mockApi.listSessionApprovals.mockResolvedValue({
      items: [
        {
          approval_id: 'approval-old',
          session_id: 'session-1',
          runner_id: 'runner-1',
          state: 'pending',
          title: 'Older',
          description: 'Older request',
          metadata: {},
          created_at: '2026-04-14T09:00:00Z',
          updated_at: '2026-04-14T09:00:00Z',
          responded_at: null,
          responder: null,
          note: null,
        },
        {
          approval_id: 'approval-new',
          session_id: 'session-1',
          runner_id: 'runner-1',
          state: 'pending',
          title: 'Newer',
          description: 'Newer request',
          metadata: {},
          created_at: '2026-04-14T09:01:00Z',
          updated_at: '2026-04-14T09:01:00Z',
          responded_at: null,
          responder: null,
          note: null,
        },
      ],
    });
    mockApi.listSessionArtifacts.mockResolvedValue({
      items: [
        {
          artifact_id: 'artifact-old',
          session_id: 'session-1',
          runner_id: 'runner-1',
          name: 'older',
          file_name: 'older.txt',
          media_type: 'text/plain',
          size_bytes: 10,
          metadata: {},
          created_at: '2026-04-14T09:00:00Z',
        },
        {
          artifact_id: 'artifact-new',
          session_id: 'session-1',
          runner_id: 'runner-1',
          name: 'newer',
          file_name: 'newer.txt',
          media_type: 'text/plain',
          size_bytes: 20,
          metadata: {},
          created_at: '2026-04-14T09:02:00Z',
        },
      ],
    });

    const bundle = await loadRemoteSessionBundle('https://remote-code.yz520gzy.top', 'session-1');

    expect(bundle.events).toHaveLength(1);
    expect(bundle.events[0].detail.kind).toBe('message_committed');
    expect(bundle.approvals[0].approval_id).toBe('approval-new');
    expect(bundle.artifacts[0].artifact_id).toBe('artifact-new');
    expect(bundle.latestSequence).toBe(2);
  });

  it('subscribes to live session events and forwards parsed frames', async () => {
    const states: string[] = [];
    const events: unknown[] = [];

    const handle = subscribeToRemoteSessionEvents({
      baseUrl: 'https://remote-code.yz520gzy.top',
      sessionId: 'session-1',
      getAfterSequence: () => 3,
      onConnectionStateChange: (state) => {
        states.push(state);
      },
      onEvent: (event) => {
        events.push(event);
      },
    });

    await Promise.resolve();

    expect(mockApi.buildSessionEventsStreamUrl).toHaveBeenCalledWith(
      'https://remote-code.yz520gzy.top',
      'session-1',
      3,
    );
    expect(states).toEqual(['reconnecting', 'open']);

    MockWebSocket.instances[0].onmessage?.({
      data: JSON.stringify({
        sequence: 4,
        recorded_at: '2026-04-14T09:00:03Z',
        runner_id: 'runner-1',
        session_id: 'session-1',
        detail: { kind: 'runtime_error', message: 'boom' },
      }),
    } as MessageEvent);

    expect(events).toHaveLength(1);

    handle.close();
  });

  it('backs off reconnect attempts and waits for online recovery when offline', async () => {
    vi.useFakeTimers();
    MockWebSocket.autoOpen = false;
    Object.defineProperty(window.navigator, 'onLine', {
      configurable: true,
      value: true,
    });

    const states: string[] = [];
    const handle = subscribeToRemoteSessionEvents({
      baseUrl: 'https://remote-code.yz520gzy.top',
      sessionId: 'session-1',
      getAfterSequence: () => 0,
      onConnectionStateChange: (state) => {
        states.push(state);
      },
      onEvent: () => {},
    });

    expect(MockWebSocket.instances).toHaveLength(1);

    MockWebSocket.instances[0].onclose?.();
    await vi.advanceTimersByTimeAsync(999);
    expect(MockWebSocket.instances).toHaveLength(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(MockWebSocket.instances).toHaveLength(2);

    Object.defineProperty(window.navigator, 'onLine', {
      configurable: true,
      value: false,
    });
    MockWebSocket.instances[1].onclose?.();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(MockWebSocket.instances).toHaveLength(2);

    Object.defineProperty(window.navigator, 'onLine', {
      configurable: true,
      value: true,
    });
    window.dispatchEvent(new Event('online'));
    expect(MockWebSocket.instances).toHaveLength(3);
    expect(states).toContain('reconnecting');

    handle.close();
  });
});
