import { hasTauriRuntime } from '../runtime';

type LifecycleCallbacks = {
  onResume?: () => void;
  onPause?: () => void;
};

export function initAppLifecycle(callbacks: LifecycleCallbacks): () => void {
  if (!hasTauriRuntime()) return () => {};

  const handler = () => {
    if (document.visibilityState === 'visible') {
      callbacks.onResume?.();
    } else {
      callbacks.onPause?.();
    }
  };

  document.addEventListener('visibilitychange', handler);

  // Return cleanup function to remove the listener.
  return () => {
    document.removeEventListener('visibilitychange', handler);
  };
}
