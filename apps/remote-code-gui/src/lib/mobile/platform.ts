import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function isMobile(): Promise<boolean> {
  if (!hasTauriRuntime()) return false;
  return invoke<boolean>('mobile_is_mobile');
}

let _cachedIsMobile: boolean | null = null;

/**
 * Synchronous mobile detection using UA sniffing as a fast initial guess.
 * The async isMobile() call refines the result using the Tauri native runtime,
 * but many callers need a synchronous answer at module-eval / render time.
 * This tradeoff means the first render may use the UA-based heuristic and
 * correct itself on the next tick when the native result arrives.
 */
export function isMobileSync(): boolean {
  if (_cachedIsMobile !== null) return _cachedIsMobile;
  if (typeof window === 'undefined') return false;
  const ua = navigator.userAgent;
  _cachedIsMobile = /Android|iPhone|iPad|iPod/i.test(ua);
  isMobile().then((m) => { _cachedIsMobile = m; }).catch(() => {});
  return _cachedIsMobile;
}

export function isTouchDevice(): boolean {
  if (typeof window === 'undefined') return false;
  return ('ontouchstart' in window || navigator.maxTouchPoints > 0) && window.innerWidth < 768;
}
