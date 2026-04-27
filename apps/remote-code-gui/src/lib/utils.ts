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
