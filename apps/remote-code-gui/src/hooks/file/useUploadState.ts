/**
 * 上传状态管理 Hook — 基于 useSyncExternalStore 的全局上传状态追踪
 * Upload state management — global upload tracking via useSyncExternalStore
 *
 * Adapted from AionUi useUploadState pattern. No Context Provider needed.
 */

import { useSyncExternalStore } from 'react';

export type UploadSource = 'sendbox' | 'workspace';

export interface UploadStateSnapshot {
  /** 正在上传的文件数 */
  activeCount: number;
  /** 是否有上传正在进行 */
  isUploading: boolean;
  /** 加权平均进度（0-100） */
  overallPercent: number;
}

// ── Internal store ─────────────────────────────────────────────────────────

let nextId = 0;
const uploads = new Map<number, { percent: number; size: number; source: UploadSource }>();
const listeners = new Set<() => void>();

let globalSnapshot: UploadStateSnapshot = { activeCount: 0, isUploading: false, overallPercent: 0 };
const sourceSnapshots: Record<UploadSource, UploadStateSnapshot> = {
  sendbox: { activeCount: 0, isUploading: false, overallPercent: 0 },
  workspace: { activeCount: 0, isUploading: false, overallPercent: 0 },
};

function calcSnapshot(filter?: UploadSource): UploadStateSnapshot {
  let totalBytes = 0;
  let loadedBytes = 0;
  let count = 0;
  for (const u of uploads.values()) {
    if (filter && u.source !== filter) continue;
    count++;
    totalBytes += u.size;
    loadedBytes += u.size * (u.percent / 100);
  }
  if (count === 0) return { activeCount: 0, isUploading: false, overallPercent: 0 };
  return {
    activeCount: count,
    isUploading: true,
    overallPercent: totalBytes > 0 ? Math.round((loadedBytes / totalBytes) * 100) : 0,
  };
}

function recalcSnapshot(): void {
  globalSnapshot = calcSnapshot();
  sourceSnapshots.sendbox = calcSnapshot('sendbox');
  sourceSnapshots.workspace = calcSnapshot('workspace');
}

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

// ── Public API for upload callers ──────────────────────────────────────────

/**
 * 注册一个新的上传追踪。返回 id, onProgress, finish。
 * Register a new upload tracker. Returns id, onProgress, finish.
 */
export function trackUpload(
  fileSize: number,
  source: UploadSource = 'sendbox',
): {
  id: number;
  onProgress: (percent: number) => void;
  finish: () => void;
} {
  const id = nextId++;
  uploads.set(id, { percent: 0, size: fileSize, source });
  recalcSnapshot();
  notify();

  return {
    id,
    onProgress(percent: number) {
      const entry = uploads.get(id);
      if (entry) {
        entry.percent = Math.min(100, Math.max(0, percent));
        recalcSnapshot();
        notify();
      }
    },
    finish() {
      uploads.delete(id);
      recalcSnapshot();
      notify();
    },
  };
}

/**
 * Reset all upload state. For use in tests only.
 */
export function resetUploadState(): void {
  uploads.clear();
  nextId = 0;
  recalcSnapshot();
  notify();
}

// ── React hook ─────────────────────────────────────────────────────────────

function getGlobalSnapshot(): UploadStateSnapshot {
  return globalSnapshot;
}

function getSourceSnapshot(source: UploadSource): () => UploadStateSnapshot {
  return () => sourceSnapshots[source];
}

/**
 * 订阅上传状态的 React hook。
 * React hook to subscribe to upload state.
 *
 * @param source 可选，按来源过滤（sendbox / workspace）
 * @returns 上传状态快照
 */
export function useUploadState(source?: UploadSource): UploadStateSnapshot {
  const getSnapshot = source ? getSourceSnapshot(source) : getGlobalSnapshot;
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
