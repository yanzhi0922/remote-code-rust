/**
 * Platform detection utilities for the mobile app.
 */

import { Capacitor } from '@capacitor/core';

export type MobilePlatform = 'ios' | 'android' | 'web';

/**
 * Get the current platform.
 */
export function getPlatform(): MobilePlatform {
  if (Capacitor.isNativePlatform()) {
    return Capacitor.getPlatform() as MobilePlatform;
  }
  return 'web';
}

/**
 * Check if running on a native mobile platform.
 */
export function isNative(): boolean {
  return Capacitor.isNativePlatform();
}

/**
 * Check if running on iOS.
 */
export function isIOS(): boolean {
  return Capacitor.getPlatform() === 'ios';
}

/**
 * Check if running on Android.
 */
export function isAndroid(): boolean {
  return Capacitor.getPlatform() === 'android';
}
