import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function normalizePathKey(path: string): string {
  const normalized = path.replace(/\//g, '\\').replace(/\\+$/, '');
  return normalized.toLowerCase();
}

export function truncateMiddle(value: string, maxLength = 72): string {
  if (value.length <= maxLength) {
    return value;
  }
  const head = Math.ceil((maxLength - 3) / 2);
  const tail = Math.floor((maxLength - 3) / 2);
  return `${value.slice(0, head)}...${value.slice(-tail)}`;
}
