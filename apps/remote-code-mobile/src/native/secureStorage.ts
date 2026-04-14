/**
 * Secure storage adapter using Capacitor Preferences.
 *
 * On iOS, data is stored in UserDefaults (encrypted at rest on modern iOS).
 * On Android, data is stored in SharedPreferences.
 * For production, consider adding a custom encrypted storage plugin.
 */

import { Preferences } from '@capacitor/preferences';

const GROUP = 'RemoteCode';

function prefixedKey(key: string): string {
  return `${GROUP}:${key}`;
}

/**
 * Read a string value from secure storage.
 * Falls back to localStorage when running in a browser (dev mode).
 */
export async function readSecureString(key: string): Promise<string | null> {
  try {
    const result = await Preferences.get({ key: prefixedKey(key) });
    return result.value;
  } catch {
    // Fallback for browser dev mode
    return localStorage.getItem(prefixedKey(key));
  }
}

/**
 * Write a string value to secure storage.
 */
export async function writeSecureString(key: string, value: string): Promise<void> {
  try {
    await Preferences.set({
      key: prefixedKey(key),
      value,
    });
  } catch {
    // Fallback for browser dev mode
    localStorage.setItem(prefixedKey(key), value);
  }
}

/**
 * Remove a value from secure storage.
 */
export async function removeSecureString(key: string): Promise<void> {
  try {
    await Preferences.remove({ key: prefixedKey(key) });
  } catch {
    // Fallback for browser dev mode
    localStorage.removeItem(prefixedKey(key));
  }
}
