/**
 * App lifecycle management for Capacitor mobile.
 *
 * Handles foreground/background transitions to manage WebSocket
 * reconnection and session state preservation.
 */

import { App } from '@capacitor/app';

type LifecycleCallback = () => void;

interface AppLifecycleHandlers {
  onResume?: LifecycleCallback;
  onPause?: LifecycleCallback;
  onNetworkChange?: (connected: boolean) => void;
}

let handlers: AppLifecycleHandlers = {};
let initialized = false;

/**
 * Initialize app lifecycle listeners.
 * Should be called once from the main entry point.
 */
export function initAppLifecycle(listeners: AppLifecycleHandlers): void {
  if (initialized) {
    handlers = { ...handlers, ...listeners };
    return;
  }

  handlers = listeners;
  initialized = true;

  // App foreground/background events
  App.addListener('appStateChange', (state) => {
    if (state.isActive) {
      handlers.onResume?.();
    } else {
      handlers.onPause?.();
    }
  });

  // Back button on Android
  // NOTE: Network status changes are handled by network.ts initNetworkMonitoring()
  // to avoid duplicate listener registration.
  App.addListener('backButton', () => {
    // Let the default behavior handle it (exit app or navigate back)
    // We could add custom logic here later
  });
}

/**
 * Check if the app is currently in the foreground.
 */
export async function isAppActive(): Promise<boolean> {
  const state = await App.getState();
  return state.isActive;
}

/**
 * Check current network connectivity.
 * Delegates to the network module which owns the Network plugin listener.
 */
export { isOnline as isNetworkConnected } from './network';
