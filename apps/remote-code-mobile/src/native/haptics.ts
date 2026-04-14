/**
 * Haptic feedback service for the mobile app.
 *
 * Provides tactile feedback for user interactions:
 * - Light impact: button presses
 * - Medium impact: approval actions
 * - Heavy impact: important confirmations
 * - Notification: success/error feedback
 */

import { Haptics, ImpactStyle, NotificationType } from '@capacitor/haptics';
import { isNative } from './platform';

/**
 * Light haptic feedback — button taps, list item selection.
 */
export async function hapticLight(): Promise<void> {
  if (!isNative()) return;
  try {
    await Haptics.impact({ style: ImpactStyle.Light });
  } catch {
    // Ignore haptic errors
  }
}

/**
 * Medium haptic feedback — approval actions, toggle switches.
 */
export async function hapticMedium(): Promise<void> {
  if (!isNative()) return;
  try {
    await Haptics.impact({ style: ImpactStyle.Medium });
  } catch {
    // Ignore haptic errors
  }
}

/**
 * Heavy haptic feedback — important confirmations, destructive actions.
 */
export async function hapticHeavy(): Promise<void> {
  if (!isNative()) return;
  try {
    await Haptics.impact({ style: ImpactStyle.Heavy });
  } catch {
    // Ignore haptic errors
  }
}

/**
 * Success notification haptic — approval granted, session completed.
 */
export async function hapticSuccess(): Promise<void> {
  if (!isNative()) return;
  try {
    await Haptics.notification({ type: NotificationType.Success });
  } catch {
    // Ignore haptic errors
  }
}

/**
 * Warning notification haptic — session failed, connection lost.
 */
export async function hapticWarning(): Promise<void> {
  if (!isNative()) return;
  try {
    await Haptics.notification({ type: NotificationType.Warning });
  } catch {
    // Ignore haptic errors
  }
}

/**
 * Error notification haptic — authentication failed, critical error.
 */
export async function hapticError(): Promise<void> {
  if (!isNative()) return;
  try {
    await Haptics.notification({ type: NotificationType.Error });
  } catch {
    // Ignore haptic errors
  }
}

/**
 * Selection haptic — spinning picker, segment control changes.
 */
export async function hapticSelection(): Promise<void> {
  if (!isNative()) return;
  try {
    await Haptics.selectionStart();
  } catch {
    // Ignore haptic errors
  }
}
