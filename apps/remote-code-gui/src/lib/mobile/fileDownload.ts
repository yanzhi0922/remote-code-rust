import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function shareFile(
  filePath: string,
  title?: string,
): Promise<void> {
  if (!hasTauriRuntime()) return;
  return invoke('mobile_share_file', { filePath, title: title ?? null });
}
