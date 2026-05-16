import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

const isWindows =
  typeof navigator !== 'undefined' && /win/i.test(navigator.platform);

export function normalizePathKey(path: string): string {
  if (isWindows) {
    return path.replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
  }
  return path.replace(/\\+$/, '');
}

export function truncateMiddle(value: string, maxLength = 72): string {
  if (value.length <= maxLength) {
    return value;
  }
  const head = Math.ceil((maxLength - 3) / 2);
  const tail = Math.floor((maxLength - 3) / 2);
  return `${value.slice(0, head)}...${value.slice(-tail)}`;
}

export const SENSITIVE_PATH_PLACEHOLDER = '路径已隐藏';

const SENSITIVE_PATH_KEYS = new Set([
  'blocked_path',
  'config_path',
  'cwd',
  'file_path',
  'origin_path',
  'path',
  'plan_file_path',
  'profile_dir',
  'project_path',
  'settings_files',
  'working_directory',
  'workingDirectory',
]);

export function formatSensitivePath(
  value: string | null | undefined,
  privacyMode: boolean,
  maxLength = 72,
): string {
  if (!value) {
    return '';
  }
  return privacyMode ? SENSITIVE_PATH_PLACEHOLDER : truncateMiddle(value, maxLength);
}

export function redactSensitivePathsForDisplay(value: unknown, privacyMode: boolean): unknown {
  if (!privacyMode) {
    return value;
  }
  if (Array.isArray(value)) {
    return value.map((item) => redactSensitivePathsForDisplay(item, privacyMode));
  }
  if (!value || typeof value !== 'object') {
    return value;
  }

  const redacted: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) {
    if (SENSITIVE_PATH_KEYS.has(key)) {
      redacted[key] = SENSITIVE_PATH_PLACEHOLDER;
    } else {
      redacted[key] = redactSensitivePathsForDisplay(item, privacyMode);
    }
  }
  return redacted;
}
