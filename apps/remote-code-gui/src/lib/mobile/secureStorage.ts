import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function secureStoreGet(key: string): Promise<string | null> {
  if (!hasTauriRuntime()) {
    return null;
  }
  return invoke<string | null>('mobile_secure_store_get', { key });
}

export async function secureStoreSet(key: string, value: string): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }
  await invoke('mobile_secure_store_set', { key, value });
}

export async function secureStoreRemove(key: string): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }
  await invoke('mobile_secure_store_remove', { key });
}
