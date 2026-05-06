/**
 * Tauri Plugin Share - Share content to other apps
 *
 * This plugin provides APIs to share text, URLs, and files with other applications.
 * On mobile, it uses native share sheets. On desktop, it may copy to clipboard or open links.
 */

import { invoke } from '@tauri-apps/api/core';

export interface ShareOptions {
  /** The title of the shared content */
  title?: string;
  /** The text content to share */
  text?: string;
  /** The URL to share */
  url?: string;
  /** File paths to share (mobile only) */
  files?: string[];
}

/**
 * Share content with other applications.
 * On mobile, this opens the native share sheet.
 * On desktop, URLs are opened in the default browser and text is copied to clipboard.
 */
export async function share(options: ShareOptions): Promise<void> {
  try {
    await invoke('plugin:share|share', { options });
  } catch {
    // Fallback for web/desktop
    if (options.url) {
      window.open(options.url, '_blank', 'noopener,noreferrer');
    } else if (options.text && navigator.clipboard) {
      await navigator.clipboard.writeText(options.text);
    }
  }
}

/**
 * Check if sharing is available on the current platform.
 */
export async function isShareAvailable(): Promise<boolean> {
  try {
    return await invoke('plugin:share|is_available');
  } catch {
    // Web share API availability
    return typeof navigator !== 'undefined' && 'share' in navigator;
  }
}