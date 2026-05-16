import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { buildArtifactDownloadUrl, buildSessionEventsStreamUrl, createStreamTicket, requestJson } from './api';

const mockRuntime = vi.hoisted(() => ({
  resolveRemoteAccessToken: vi.fn(() => 'device-token'),
}));

vi.mock('../lib/runtime', () => mockRuntime);

describe('remote api', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('builds artifact download urls without leaking access tokens', () => {
    const url = buildArtifactDownloadUrl(
      'https://remote-code.yz520gzy.top',
      'artifact-1',
    );

    expect(url).toBe('https://remote-code.yz520gzy.top/v1/artifacts/artifact-1/download');
    expect(url).not.toContain('access_token');
    expect(url).not.toContain('device-token');
  });

  it('builds session stream urls with one-time tickets instead of access tokens', () => {
    const url = buildSessionEventsStreamUrl(
      'https://remote-code.yz520gzy.top',
      'session-1',
      42,
      'rcst_test',
    );

    expect(url).toBe(
      'wss://remote-code.yz520gzy.top/v1/sessions/session-1/events/stream?after=42&stream_ticket=rcst_test',
    );
    expect(url).not.toContain('access_token');
    expect(url).not.toContain('device-token');
  });

  it('adds auth headers without forcing json content-type on GET requests', async () => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ items: [] }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );

    await requestJson<{ items: unknown[] }>(
      'https://remote-code.yz520gzy.top',
      '/v1/sessions',
    );

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = new Headers(init.headers);

    expect(url).toBe('https://remote-code.yz520gzy.top/v1/sessions');
    expect(init.cache).toBe('no-store');
    expect(headers.get('authorization')).toBe('Bearer device-token');
    expect(headers.get('accept')).toBe('application/json');
    expect(headers.get('content-type')).toBeNull();
  });

  it('adds json content-type for request bodies', async () => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );

    await requestJson<{ ok: boolean }>(
      'https://remote-code.yz520gzy.top',
      '/v1/pairing/offers',
      {
        method: 'POST',
        body: JSON.stringify({ device_name: 'Browser' }),
      },
    );

    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = new Headers(init.headers);
    expect(headers.get('content-type')).toBe('application/json');
  });

  it('requests stream tickets over authenticated HTTP', async () => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ stream_ticket: 'rcst_1', expires_in_secs: 45 }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );

    const response = await createStreamTicket(
      'https://remote-code.yz520gzy.top',
      '/v1/sessions/session-1/events/stream',
    );

    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = new Headers(init.headers);
    expect(url).toBe('https://remote-code.yz520gzy.top/v1/stream-ticket');
    expect(init.method).toBe('POST');
    expect(headers.get('authorization')).toBe('Bearer device-token');
    expect(response.stream_ticket).toBe('rcst_1');
  });

  it('retries one transient GET failure before succeeding', async () => {
    vi.useFakeTimers();
    fetchMock
      .mockRejectedValueOnce(new TypeError('Failed to fetch'))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ items: ['ok'] }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    const request = requestJson<{ items: string[] }>(
      'https://remote-code.yz520gzy.top',
      '/v1/sessions',
    );

    await vi.runAllTimersAsync();
    const response = await request;

    expect(response.items).toEqual(['ok']);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
