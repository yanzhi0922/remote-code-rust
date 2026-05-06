/**
 * Tauri Plugin Network - Network status monitoring
 *
 * This plugin provides APIs to monitor network connectivity and connection type.
 * On mobile, it uses native platform APIs. On web, it falls back to browser APIs.
 */

import { invoke } from '@tauri-apps/api/core';

export interface NetworkStatus {
  connected: boolean;
  connectionType: 'wifi' | 'cellular' | 'ethernet' | 'none' | 'unknown';
}

export type NetworkStatusChangeListener = (status: NetworkStatus) => void;

/**
 * Get the current network status.
 * Falls back to browser APIs when the Tauri runtime is not available.
 */
export async function getNetworkStatus(): Promise<NetworkStatus> {
  try {
    return await invoke('plugin:network|get_network_status');
  } catch {
    // Fallback to browser API
    if (typeof navigator !== 'undefined') {
      return {
        connected: navigator.onLine,
        connectionType: navigator.onLine ? 'wifi' : 'none',
      };
    }
    return { connected: true, connectionType: 'unknown' };
  }
}

/**
 * Listen for network status changes.
 * Returns a promise that resolves to an unsubscribe function.
 */
export async function onNetworkStatusChange(
  listener: NetworkStatusChangeListener
): Promise<() => void> {
  try {
    const unlisten = await (window as any).__TAURI__.event.listen<NetworkStatus>(
      'network-status-changed',
      (event) => listener(event.payload)
    );
    return unlisten;
  } catch {
    // Fallback to browser APIs
    if (typeof window !== 'undefined') {
      const onlineHandler = () => listener({ connected: true, connectionType: 'wifi' });
      const offlineHandler = () => listener({ connected: false, connectionType: 'none' });
      window.addEventListener('online', onlineHandler);
      window.addEventListener('offline', offlineHandler);
      return () => {
        window.removeEventListener('online', onlineHandler);
        window.removeEventListener('offline', offlineHandler);
      };
    }
    return () => {};
  }
}

/**
 * Check if the device is currently online.
 */
export async function isOnline(): Promise<boolean> {
  const status = await getNetworkStatus();
  return status.connected;
}