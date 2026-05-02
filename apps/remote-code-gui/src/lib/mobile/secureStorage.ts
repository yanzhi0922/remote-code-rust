import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function secureStoreGet(key: string): Promise<string | null> {
  if (!hasTauriRuntime()) {
    return localStorage.getItem(`RC:${key}`);
  }
  return invoke<string | null>('mobile_secure_store_get', { key });
}

export async function secureStoreSet(key: string, value: string): Promise<void> {
  if (!hasTauriRuntime()) {
    console.warn('secureStoreSet: using localStorage fallback — not secure for sensitive data');
    localStorage.setItem(`RC:${key}`, value);
    return;
  }
  return invoke('mobile_secure_store_set', { key, value });
}

export async function secureStoreRemove(key: string): Promise<void> {
  if (!hasTauriRuntime()) {
    localStorage.removeItem(`RC:${key}`);
    return;
  }
  return invoke('mobile_secure_store_remove', { key });
}
