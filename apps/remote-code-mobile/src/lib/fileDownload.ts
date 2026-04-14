import { downloadArtifact } from '../native/fileDownload';

export interface RemoteArtifactDownloadRequest {
  url: string;
  fileName: string;
  token: string | null;
}

export async function downloadRemoteArtifact(
  request: RemoteArtifactDownloadRequest,
): Promise<void> {
  const token = request.token?.trim();
  if (!token) {
    throw new Error('Remote artifact download requires an authenticated device token.');
  }

  await downloadArtifact(request.url, request.fileName, token);
}
