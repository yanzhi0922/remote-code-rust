import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function secureStoreGet(key: string): Promise<string | null> {
  if (!hasTauriRuntime()) {
    return localStorage.getItem(`RC:${key}`);
  }
  return invoke<string | null>('mobile_secure_store_get', { key });
}
