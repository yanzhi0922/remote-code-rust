import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from './runtime';

export interface RemoteArtifactDownloadRequest {
  url: string;
  fileName: string;
  token: string | null;
}

export async function downloadRemoteArtifact(
  request: RemoteArtifactDownloadRequest,
): Promise<string | null> {
  const token = request.token?.trim();
  if (!token) {
    throw new Error('Remote artifact download requires an authenticated device token.');
  }

  if (hasTauriRuntime()) {
    return invoke<string>('mobile_download_artifact', {
      url: request.url,
      fileName: sanitizeFileName(request.fileName),
      token,
    });
  }

  const response = await fetch(request.url, {
    headers: {
      authorization: `Bearer ${token}`,
    },
    cache: 'no-store',
  });

  if (!response.ok) {
    throw new Error(`Artifact download failed with HTTP ${response.status}.`);
  }

  const blob = await response.blob();
  const objectUrl = window.URL.createObjectURL(blob);

  try {
    const anchor = document.createElement('a');
    anchor.href = objectUrl;
    anchor.download = sanitizeFileName(request.fileName);
    anchor.rel = 'noopener';
    anchor.style.display = 'none';
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
  } finally {
    window.setTimeout(() => {
      window.URL.revokeObjectURL(objectUrl);
    }, 1_000);
  }

  return null;
}

function sanitizeFileName(value: string): string {
  const normalized = value.trim().replace(/[\\/:*?"<>|]+/g, '-');
  return normalized || 'artifact';
}
