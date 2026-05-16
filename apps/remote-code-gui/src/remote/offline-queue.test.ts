import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { enqueueCommand, drainCommands } from './offline-queue';

// ---------------------------------------------------------------------------
// Minimal IndexedDB mock — in-memory store
// ---------------------------------------------------------------------------

type StoreEntry = Record<string, unknown>;

let store: Map<string, StoreEntry> = new Map();

const mockIDBFactory: IDBFactory = {
  databases: vi.fn(() => Promise.resolve([])),
  cmp: vi.fn((a, b) => (a === b ? 0 : a < b ? -1 : 1)),
  open: vi.fn((_name: string, _version?: number) => {
    const request = {
      onupgradeneeded: null as (() => void) | null,
      onsuccess: null as (() => void) | null,
      onerror: null as (() => void) | null,
      result: {
        objectStoreNames: { contains: () => true },
        createObjectStore: vi.fn(),
        transaction: vi.fn(() => {
          const s = {
            objectStore: vi.fn(() => {
              const ops = {
                add: vi.fn((entry: StoreEntry) => {
                  store.set(entry.id as string, entry);
                  return mockRequest({ result: entry });
                }),
                put: vi.fn((entry: StoreEntry) => {
                  store.set(entry.id as string, entry);
                  return mockRequest({ result: entry });
                }),
                get: vi.fn((id: string) => mockRequest({ result: store.get(id) ?? null })),
                getAll: vi.fn(() => mockRequest({ result: Array.from(store.values()) })),
                delete: vi.fn((id: string) => {
                  store.delete(id);
                  return mockRequest({ result: undefined });
                }),
                count: vi.fn(() => mockRequest({ result: store.size })),
                clear: vi.fn(() => {
                  store.clear();
                  return mockRequest({ result: undefined });
                }),
                openCursor: vi.fn(() => {
                  const entries = Array.from(store.values()).sort(
                    (a, b) => (a.timestamp as number) - (b.timestamp as number),
                  );
                  let idx = 0;
                  const cursor = {
                    value: entries[0] ?? null,
                    delete: vi.fn(() => {
                      if (entries[idx]) store.delete(entries[idx].id as string);
                    }),
                    continue: vi.fn(() => {
                      idx++;
                      cursor.value = entries[idx] ?? null;
                    }),
                  };
                  return mockRequest({
                    result: entries.length > 0 ? cursor : null,
                  });
                }),
              };
              return ops;
            }),
          };
          return s;
        }),
        close: vi.fn(),
      } as unknown as IDBDatabase,
    };

    // Simulate async IDB open
    queueMicrotask(() => {
      request.onsuccess?.();
    });

    return request as unknown as IDBOpenDBRequest;
  }),
  deleteDatabase: vi.fn(),
};

function mockRequest(opts: { result: unknown }): IDBRequest {
  const req = {
    result: opts.result,
    onsuccess: null as (() => void) | null,
    onerror: null as (() => void) | null,
  } as unknown as IDBRequest;
  // Resolve on next microtask
  queueMicrotask(() => {
    (req as any).onsuccess?.();
  });
  return req;
}

// Reset module-level dbInstance between tests
function resetModuleState() {
  // The module caches dbInstance; we re-import to reset, but since vitest
  // modules are cached we manipulate the store directly and clear it.
  store = new Map();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('offline-queue', () => {
  beforeEach(() => {
    resetModuleState();
    vi.stubGlobal('indexedDB', mockIDBFactory);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('enqueues a command', async () => {
    const entry = await enqueueCommand('session-1', {
      kind: 'send_prompt',
      content: 'hello',
    });

    expect(entry.id).toBeTruthy();
    expect(entry.sessionId).toBe('session-1');
    expect(entry.command).toEqual({ kind: 'send_prompt', content: 'hello' });
    expect(entry.retryCount).toBe(0);

    expect(store.size).toBe(1);
  });

  it('drains commands for the matching session and removes them', async () => {
    await enqueueCommand('session-1', { kind: 'interrupt' });
    await enqueueCommand('session-2', { kind: 'interrupt' });
    await enqueueCommand('session-1', { kind: 'send_prompt', content: 'world' });

    const drained = await drainCommands('session-1');
    expect(drained).toHaveLength(2);
    expect(drained[0].command.kind).toBe('interrupt');
    expect(drained[1].command.kind).toBe('send_prompt');

    // session-2 should still be in the store
    expect(store.size).toBe(1);
  });

  it('filters out stale commands older than 5 minutes during drain', async () => {
    vi.useFakeTimers();
    const now = Date.now();

    // Manually insert a stale entry into the mock store
    store.set('stale-1', {
      id: 'stale-1',
      timestamp: now - 6 * 60 * 1000, // 6 min ago
      sessionId: 'session-1',
      command: { kind: 'interrupt' },
      retryCount: 0,
    });
    store.set('fresh-1', {
      id: 'fresh-1',
      timestamp: now - 1000,
      sessionId: 'session-1',
      command: { kind: 'send_prompt', content: 'fresh' },
      retryCount: 0,
    });

    const drained = await drainCommands('session-1');
    expect(drained).toHaveLength(1);
    expect(drained[0].id).toBe('fresh-1');

    // Stale entry should have been pruned
    expect(store.has('stale-1')).toBe(false);

    vi.useRealTimers();
  });

  it('drain returns empty array when nothing is queued', async () => {
    const drained = await drainCommands('nonexistent');
    expect(drained).toEqual([]);
  });

  it('drains commands in timestamp order', async () => {
    const now = Date.now();

    // Enqueue with controlled timestamps (recent enough to not be stale)
    store.set('cmd-1', {
      id: 'cmd-1',
      timestamp: now - 2000,
      sessionId: 'session-1',
      command: { kind: 'send_prompt', content: 'first' },
      retryCount: 0,
    });
    store.set('cmd-2', {
      id: 'cmd-2',
      timestamp: now - 4000,
      sessionId: 'session-1',
      command: { kind: 'send_prompt', content: 'second' },
      retryCount: 0,
    });

    const drained = await drainCommands('session-1');
    expect(drained).toHaveLength(2);
    // Sorted by timestamp ascending
    expect(drained[0].id).toBe('cmd-2');
    expect(drained[1].id).toBe('cmd-1');
  });
});
