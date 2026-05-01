import { hasTauriRuntime } from '../runtime';

export interface NetworkStatus {
  connected: boolean;
  connectionType: string;
}

type NetworkChangeListener = (connected: boolean, connectionType: string) => void;

const listeners = new Set<NetworkChangeListener>();
let currentStatus: NetworkStatus = { connected: true, connectionType: 'unknown' };

export function initNetworkMonitoring(): void {
  if (!hasTauriRuntime()) {
    if (typeof window !== 'undefined') {
      window.addEventListener('online', () => {
        currentStatus = { connected: true, connectionType: 'wifi' };
        listeners.forEach((fn) => fn(true, 'wifi'));
      });
      window.addEventListener('offline', () => {
        currentStatus = { connected: false, connectionType: 'none' };
        listeners.forEach((fn) => fn(false, 'none'));
      });
      currentStatus.connected = navigator.onLine;
    }
    return;
  }
  import('@tauri-apps/plugin-network').then((mod) => {
    const stream = mod.onNetworkStatusChange((status) => {
      currentStatus = { connected: status.connected, connectionType: status.connectionType ?? 'unknown' };
      listeners.forEach((fn) => fn(currentStatus.connected, currentStatus.connectionType));
    });
    stream.catch(() => {});
  }).catch(() => {});
}

export function getNetworkStatus(): NetworkStatus {
  return currentStatus;
}

export async function isOnline(): Promise<boolean> {
  if (typeof navigator !== 'undefined') return navigator.onLine;
  return currentStatus.connected;
}

export function onNetworkChange(listener: NetworkChangeListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function describeConnectionType(type: string): string {
  switch (type) {
    case 'wifi': return 'WiFi';
    case 'cellular': return '蜂窝网络';
    case 'none': return '无网络';
    case 'unknown': return '未知';
    default: return type;
  }
}
