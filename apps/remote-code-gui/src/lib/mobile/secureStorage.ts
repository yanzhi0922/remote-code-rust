import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function secureStoreGet(key: string): Promise<string | null> {
  if (!hasTauriRuntime()) {
    return null;
  }
  return invoke<string | null>('mobile_secure_store_get', { key });
}
