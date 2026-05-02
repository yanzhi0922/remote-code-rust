import { hasTauriRuntime } from '../runtime';

type LifecycleCallbacks = {
  onResume?: () => void;
  onPause?: () => void;
};

export async function initAppLifecycle(callbacks: LifecycleCallbacks): Promise<void> {
  if (!hasTauriRuntime()) return;

  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      callbacks.onResume?.();
    } else {
      callbacks.onPause?.();
    }
  });
}

export function isAppActive(): boolean {
  if (typeof document === 'undefined') return true;
  return document.visibilityState === 'visible';
}

export { isOnline as isNetworkConnected } from './network';
