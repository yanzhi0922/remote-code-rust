import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function isMobile(): Promise<boolean> {
  if (!hasTauriRuntime()) return false;
  return invoke<boolean>('mobile_is_mobile');
}

let _cachedIsMobile: boolean | null = null;

export function isMobileSync(): boolean {
  if (_cachedIsMobile !== null) return _cachedIsMobile;
  if (typeof window === 'undefined') return false;
  const ua = navigator.userAgent;
  _cachedIsMobile = /Android|iPhone|iPad|iPod/i.test(ua);
  isMobile().then((m) => { _cachedIsMobile = m; });
  return _cachedIsMobile;
}

export function isTouchDevice(): boolean {
  if (typeof window === 'undefined') return false;
  return 'ontouchstart' in window || navigator.maxTouchPoints > 0;
}
