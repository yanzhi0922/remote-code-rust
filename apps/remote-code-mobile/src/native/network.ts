/**
 * Network status monitoring for the mobile app.
 *
 * Provides real-time network connectivity detection using
 * Capacitor's Network plugin, which uses native APIs instead
 * of the browser's navigator.onLine.
 */

import { Network, type ConnectionStatus } from '@capacitor/network';
import { isNative } from './platform';

type NetworkStatusListener = (connected: boolean, connectionType: string) => void;

let currentStatus: ConnectionStatus | null = null;
let listeners: NetworkStatusListener[] = [];

/**
 * Initialize network monitoring.
 */
export async function initNetworkMonitoring(): Promise<void> {
  if (!isNative()) {
    return;
  }

  // Get initial status
  currentStatus = await Network.getStatus();

  // Listen for changes
  Network.addListener('networkStatusChange', (status: ConnectionStatus) => {
    const previousConnected = currentStatus?.connected ?? true;
    currentStatus = status;

    // Notify all listeners
    for (const listener of listeners) {
      listener(status.connected, status.connectionType);
    }

    // Dispatch a custom event for components that don't use listeners
    if (previousConnected !== status.connected) {
      window.dispatchEvent(
        new CustomEvent(status.connected ? 'network-restored' : 'network-lost'),
      );
    }
  });
}

/**
 * Get the current network status.
 */
export async function getNetworkStatus(): Promise<ConnectionStatus> {
  if (!isNative()) {
    return {
      connected: navigator.onLine,
      connectionType: 'unknown',
    };
  }

  if (currentStatus) {
    return currentStatus;
  }

  currentStatus = await Network.getStatus();
  return currentStatus;
}

/**
 * Check if the device is currently online.
 */
export async function isOnline(): Promise<boolean> {
  const status = await getNetworkStatus();
  return status.connected;
}

/**
 * Add a network status change listener.
 * Returns a function to remove the listener.
 */
export function onNetworkChange(listener: NetworkStatusListener): () => void {
  listeners.push(listener);

  // If we already have a status, call immediately
  if (currentStatus) {
    listener(currentStatus.connected, currentStatus.connectionType);
  }

  return () => {
    listeners = listeners.filter((l) => l !== listener);
  };
}

/**
 * Get a human-readable description of the connection type.
 */
export function describeConnectionType(type: string): string {
  switch (type) {
    case 'wifi':
      return 'WiFi';
    case 'cellular':
      return '蜂窝网络';
    case 'none':
      return '无网络';
    case 'unknown':
    default:
      return '未知';
  }
}
