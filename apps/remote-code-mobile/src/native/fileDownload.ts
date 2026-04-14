/**
 * File download service for the mobile app.
 *
 * Downloads artifacts from the Control Plane to the device's
 * local file system using Capacitor Filesystem plugin.
 */

import { Filesystem, Directory, Encoding } from '@capacitor/filesystem';
import { Share } from '@capacitor/share';
import { isNative } from './platform';

interface DownloadResult {
  path: string;
  uri: string;
}

/**
 * Download an artifact file from the Control Plane.
 *
 * @param url - Full download URL from the Control Plane
 * @param fileName - Name for the downloaded file
 * @param token - Bearer token for authentication
 */
export async function downloadArtifact(
  url: string,
  fileName: string,
  token: string,
): Promise<DownloadResult | null> {
  if (!isNative()) {
    const response = await fetch(url, {
      headers: { Authorization: `Bearer ${token}` },
      cache: 'no-store',
    });

    if (!response.ok) {
      throw new Error(`Download failed: ${response.status}`);
    }

    const blob = await response.blob();
    const objectUrl = URL.createObjectURL(blob);

    try {
      const anchor = document.createElement('a');
      anchor.href = objectUrl;
      anchor.download = fileName;
      anchor.rel = 'noopener';
      anchor.style.display = 'none';
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      return null;
    } finally {
      window.setTimeout(() => URL.revokeObjectURL(objectUrl), 60_000);
    }
  }

  try {
    // Fetch the file content
    const response = await fetch(url, {
      headers: { Authorization: `Bearer ${token}` },
    });

    if (!response.ok) {
      throw new Error(`Download failed: ${response.status}`);
    }

    const blob = await response.blob();

    // Convert blob to base64
    const base64 = await blobToBase64(blob);

    // Write to the Downloads directory
    const result = await Filesystem.writeFile({
      path: `Download/remote-code/${fileName}`,
      data: base64,
      directory: Directory.ExternalStorage,
      recursive: true,
    });

    return {
      path: `Download/remote-code/${fileName}`,
      uri: result.uri,
    };
  } catch (error) {
    console.error('[FileDownload] Error:', error);
    throw error;
  }
}

/**
 * Share a downloaded file using the system share sheet.
 */
export async function shareFile(uri: string, fileName: string): Promise<void> {
  if (!isNative()) return;

  try {
    await Share.share({
      title: fileName,
      url: uri,
    });
  } catch {
    // User cancelled or share failed
  }
}

/**
 * Check if a file exists in the download directory.
 */
export async function isFileDownloaded(fileName: string): Promise<boolean> {
  if (!isNative()) return false;

  try {
    await Filesystem.stat({
      path: `Download/remote-code/${fileName}`,
      directory: Directory.ExternalStorage,
    });
    return true;
  } catch {
    return false;
  }
}

/**
 * Read a downloaded file as text.
 */
export async function readDownloadedTextFile(fileName: string): Promise<string | null> {
  if (!isNative()) return null;

  try {
    const result = await Filesystem.readFile({
      path: `Download/remote-code/${fileName}`,
      directory: Directory.ExternalStorage,
      encoding: Encoding.UTF8,
    });
    return result.data as string;
  } catch {
    return null;
  }
}

/**
 * Convert a Blob to base64 string.
 */
function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onloadend = () => {
      const dataUrl = reader.result as string;
      // Remove the data URL prefix (e.g., "data:application/octet-stream;base64,")
      const base64 = dataUrl.split(',')[1];
      if (base64) {
        resolve(base64);
      } else {
        reject(new Error('Failed to convert blob to base64'));
      }
    };
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}
