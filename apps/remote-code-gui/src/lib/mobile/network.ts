import { hasTauriRuntime } from '../runtime';

export interface NetworkStatus {
  connected: boolean;
  connectionType: string;
}

type NetworkChangeListener = (connected: boolean, connectionType: string) => void;

const listeners = new Set<NetworkChangeListener>();
let currentStatus: NetworkStatus = { connected: true, connectionType: 'unknown' };
let cleanup: (() => void) | null = null;

export function initNetworkMonitoring(): void {
  // Prevent double-initialisation
  if (cleanup) return;

  if (!hasTauriRuntime()) {
    if (typeof window !== 'undefined') {
      const online = () => {
        currentStatus = { connected: true, connectionType: 'wifi' };
        listeners.forEach((fn) => fn(true, 'wifi'));
      };
      const offline = () => {
        currentStatus = { connected: false, connectionType: 'none' };
        listeners.forEach((fn) => fn(false, 'none'));
      };
      window.addEventListener('online', online);
      window.addEventListener('offline', offline);
      currentStatus.connected = navigator.onLine;
      cleanup = () => {
        window.removeEventListener('online', online);
        window.removeEventListener('offline', offline);
      };
    }
    return;
  }
  // Use variable to avoid Vite static analysis of the import path
  const modName = '@tauri-apps/plugin-network';
  import(/* @vite-ignore */ modName).then((mod: any) => {
    mod.onNetworkStatusChange((status: any) => {
      currentStatus = { connected: status.connected, connectionType: status.connectionType ?? 'unknown' };
      listeners.forEach((fn) => fn(currentStatus.connected, currentStatus.connectionType));
    }).then((unlisten: () => void) => {
      cleanup = unlisten;
    }).catch(() => {});
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
