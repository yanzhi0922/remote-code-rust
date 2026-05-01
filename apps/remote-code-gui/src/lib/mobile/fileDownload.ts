import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function downloadArtifact(
  url: string,
  fileName: string,
  token?: string,
): Promise<string> {
  if (!hasTauriRuntime()) {
    const a = document.createElement('a');
    a.href = url;
    a.download = fileName;
    a.click();
    return fileName;
  }
  return invoke<string>('mobile_download_artifact', { url, fileName, token: token ?? null });
}

export async function shareFile(
  filePath: string,
  title?: string,
): Promise<void> {
  if (!hasTauriRuntime()) return;
  return invoke('mobile_share_file', { filePath, title: title ?? null });
}

export async function isFileDownloaded(fileName: string): Promise<boolean> {
  return false;
}

export async function readDownloadedTextFile(fileName: string): Promise<string | null> {
  return null;
}
