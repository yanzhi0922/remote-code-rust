import { hasTauriRuntime } from '../runtime';
import i18n from '../../i18n';

export interface NetworkStatus {
  connected: boolean;
  connectionType: string;
}

type NetworkChangeListener = (connected: boolean, connectionType: string) => void;

interface NetworkPluginModule {
  onNetworkStatusChange: (
    cb: (status: { connected: boolean; connectionType?: string }) => void,
  ) => Promise<() => void>;
}

const listeners = new Set<NetworkChangeListener>();
let currentStatus: NetworkStatus = { connected: true, connectionType: 'unknown' };
let cleanup: (() => void) | null = null;

export function initNetworkMonitoring(): void {
  // Prevent double-initialisation
  if (cleanup) return;

  if (!hasTauriRuntime()) {
    if (typeof window !== 'undefined') {
      const online = () => {
        currentStatus = { connected: true, connectionType: 'unknown' };
        listeners.forEach((fn) => fn(true, 'unknown'));
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
  // Try to load the native Tauri network plugin for richer status info
  // (connection type like WiFi/cellular). Falls back to browser events silently.
  try {
    const modName = '@tauri-apps/plugin-network';
    import(/* @vite-ignore */ modName).then((mod: NetworkPluginModule) => {
      mod.onNetworkStatusChange((status) => {
        currentStatus = { connected: status.connected, connectionType: status.connectionType ?? 'unknown' };
        listeners.forEach((fn) => fn(currentStatus.connected, currentStatus.connectionType));
      }).then((unlisten: () => void) => {
        cleanup = unlisten;
      }).catch(() => {
        // Listener setup failed — browser fallback already active
      });
    }).catch(() => {
      // Plugin not registered on Rust side — browser fallback already active
    });
  } catch {
    // Dynamic import not available — browser fallback already active
  }
}

export function getNetworkStatus(): NetworkStatus {
  return currentStatus;
}

export async function isOnline(): Promise<boolean> {
  if (currentStatus.connected) return true;
  if (typeof navigator !== 'undefined') return navigator.onLine;
  return false;
}

export function onNetworkChange(listener: NetworkChangeListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function describeConnectionType(type: string): string {
  switch (type) {
    case 'wifi': return 'WiFi';
    case 'cellular': return i18n.t('mobile.cellular');
    case 'none': return i18n.t('mobile.noNetwork');
    case 'unknown': return i18n.t('mobile.unknownNetwork');
    default: return type;
  }
}
