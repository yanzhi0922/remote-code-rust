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
