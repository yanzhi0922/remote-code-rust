import type { RemoteApprovalDecision } from './types';

interface QueuedCommand {
  id: string;
  timestamp: number;
  sessionId: string;
  command: TransportCommandPayload;
  retryCount: number;
}

export type TransportCommandPayload =
  | { kind: 'send_prompt'; content: string }
  | { kind: 'interrupt' }
  | { kind: 'respond_to_approval'; approvalId: string; decision: RemoteApprovalDecision; note?: string };

const DB_NAME = 'remote-code-offline-queue';
const STORE_NAME = 'commands';
const MAX_QUEUE_SIZE = 100;
const STALE_THRESHOLD_MS = 5 * 60 * 1000; // 5 minutes

let dbInstance: IDBDatabase | null = null;

function openDb(): Promise<IDBDatabase> {
  if (dbInstance) {
    return Promise.resolve(dbInstance);
  }
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: 'id' });
      }
    };
    request.onsuccess = () => {
      const db = request.result;
      // Invalidate the cached instance if the connection encounters an error
      // later (e.g. database was deleted), allowing reconnection on next call.
      db.onerror = () => {
        dbInstance = null;
      };
      dbInstance = db;
      resolve(db);
    };
    request.onerror = () => {
      dbInstance = null;
      reject(request.error);
    };
  });
}

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

export async function enqueueCommand(
  sessionId: string,
  command: TransportCommandPayload,
): Promise<QueuedCommand> {
  const db = await openDb();
  const entry: QueuedCommand = {
    id: generateId(),
    timestamp: Date.now(),
    sessionId,
    command,
    retryCount: 0,
  };

  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);

    // Enforce max size — evict oldest.
    //
    // Known tradeoff: The count() and subsequent cursor.delete() run in the
    // same readwrite transaction but are not atomic with respect to concurrent
    // writers. If two tabs enqueue simultaneously, both may see count < MAX
    // and neither evicts, allowing the queue to exceed the limit by a small
    // margin. This is acceptable because: (a) the drain path also prunes stale
    // entries, (b) IndexedDB serialises transactions per database connection so
    // the race window is narrow, and (c) a strict compare-and-swap would
    // require a dedicated lock store, adding complexity for negligible benefit.
    const countReq = store.count();
    countReq.onsuccess = () => {
      if (countReq.result >= MAX_QUEUE_SIZE) {
        const cursorReq = store.openCursor();
        cursorReq.onsuccess = () => {
          const cursor = cursorReq.result;
          if (cursor) {
            cursor.delete();
          }
        };
      }
    };

    const addReq = store.add(entry);
    addReq.onsuccess = () => resolve(entry);
    addReq.onerror = () => reject(addReq.error);
    tx.onabort = () => reject(tx.error || new Error('Transaction aborted'));
  });
}

/**
 * Drain queued commands for the given session.
 *
 * Side effects:
 * - Removes all drained (non-stale) commands for the target session from IndexedDB.
 * - **Cross-session pruning:** Also deletes stale commands (older than
 *   {@link STALE_THRESHOLD_MS}) belonging to *any* session, not just the target.
 *   This prevents unbounded growth of the queue when other sessions never reconnect.
 */
export async function drainCommands(sessionId: string): Promise<QueuedCommand[]> {
  const db = await openDb();
  const now = Date.now();

  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const getAllReq = store.getAll();

    getAllReq.onsuccess = () => {
      const all: QueuedCommand[] = getAllReq.result;
      const relevant = all
        .filter((cmd) => cmd.sessionId === sessionId)
        .filter((cmd) => now - cmd.timestamp < STALE_THRESHOLD_MS)
        .sort((a, b) => a.timestamp - b.timestamp);

      // Remove drained items from store.
      for (const cmd of relevant) {
        store.delete(cmd.id);
      }
      // Also prune stale items for any session.
      for (const cmd of all) {
        if (now - cmd.timestamp >= STALE_THRESHOLD_MS) {
          store.delete(cmd.id);
        }
      }

      resolve(relevant);
    };

    getAllReq.onerror = () => reject(getAllReq.error);
    tx.onabort = () => reject(tx.error || new Error('Transaction aborted'));
  });
}
