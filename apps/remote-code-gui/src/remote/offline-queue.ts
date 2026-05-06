import type { RemoteApprovalDecision } from './types';

export interface QueuedCommand {
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
      dbInstance = request.result;
      resolve(dbInstance);
    };
    request.onerror = () => reject(request.error);
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
  });
}

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
  });
}

export async function clearQueue(): Promise<void> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const req = store.clear();
    req.onsuccess = () => resolve();
    req.onerror = () => reject(req.error);
  });
}

export async function getQueueSize(): Promise<number> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const store = tx.objectStore(STORE_NAME);
    const req = store.count();
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}