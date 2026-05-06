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
  if (!hasTauriRuntime()) return false;
  return invoke<boolean>('mobile_check_file_downloaded', { fileName });
}

export async function readDownloadedTextFile(fileName: string): Promise<string | null> {
  if (!hasTauriRuntime()) return null;
  return invoke<string | null>('mobile_read_downloaded_text', { fileName });
}

export async function deleteDownloadedFile(fileName: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  return invoke('mobile_delete_downloaded_file', { fileName });
}

export async function listDownloadedFiles(): Promise<string[]> {
  if (!hasTauriRuntime()) return [];
  return invoke<string[]>('mobile_list_downloaded_files');
}

export async function getDownloadedFilePath(fileName: string): Promise<string | null> {
  if (!hasTauriRuntime()) return null;
  const downloaded = await isFileDownloaded(fileName);
  if (!downloaded) return null;
  // The file is stored in the download dir under "remote-code" subdirectory
  // We return just the filename since the actual path is platform-specific
  return fileName;
}