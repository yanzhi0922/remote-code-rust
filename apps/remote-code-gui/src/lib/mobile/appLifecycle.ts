type LifecycleCallbacks = {
  onResume?: () => void;
  onPause?: () => void;
};

export function initAppLifecycle(callbacks: LifecycleCallbacks): () => void {
  const cleanups: (() => void)[] = [];

  const handler = () => {
    if (document.visibilityState === 'visible') {
      callbacks.onResume?.();
    } else {
      callbacks.onPause?.();
    }
  };

  document.addEventListener('visibilitychange', handler);
  cleanups.push(() => document.removeEventListener('visibilitychange', handler));

  // Also listen to Tauri native lifecycle events when running inside Tauri.
  if (typeof window !== 'undefined' && '__TAURI__' in window) {
    import('@tauri-apps/api/event').then(({ listen }) => {
      const unlistenResume = listen('tauri://resume', () => {
        callbacks.onResume?.();
      });
      const unlistenPause = listen('tauri://pause', () => {
        callbacks.onPause?.();
      });
      Promise.all([unlistenResume, unlistenPause]).then(([ur, up]) => {
        cleanups.push(ur);
        cleanups.push(up);
      });
    }).catch(() => {
      // Tauri event import failed; visibilitychange remains active.
    });
  }

  return () => {
    cleanups.forEach((fn) => fn());
  };
}
