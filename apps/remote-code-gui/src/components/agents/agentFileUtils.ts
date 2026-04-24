export interface AgentFileInfo {
  name: string;
  path: string;
  size: number;
  modifiedAt: string;
}

export function parseAgentFileName(filePath: string): string {
  const parts = filePath.replace(/\\/g, '/').split('/');
  const fileName = parts[parts.length - 1];
  return fileName.replace(/\.(md|yaml|yml|json)$/i, '');
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

export function isAgentFile(filePath: string): boolean {
  return /\.(md|yaml|yml|json)$/i.test(filePath);
}
