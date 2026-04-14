/**
 * Biometric authentication service for the mobile app.
 *
 * Provides optional biometric verification on app launch.
 * Falls back gracefully on devices without biometric hardware
 * or when the biometrics plugin is not installed.
 *
 * The biometrics plugin (@capawesome-team/capacitor-biometrics)
 * should be installed separately when building for production.
 * During development/web builds, all methods return safe defaults.
 */

import { isNative } from './platform';
import { readSecureString, writeSecureString, removeSecureString } from './secureStorage';

const BIOMETRIC_ENABLED_KEY = 'biometric_enabled';

// Lazy-loaded biometrics module
type BiometricsPlugin = {
  checkAvailability: () => Promise<{ available: boolean }>;
  authenticate: (options: {
    reason: string;
    iosFallbackTitle?: string;
    androidTitle?: string;
    androidSubtitle?: string;
    androidConfirmationRequired?: boolean;
  }) => Promise<void>;
};

let biometricsModule: BiometricsPlugin | null | undefined = undefined;

async function loadBiometricsModule(): Promise<BiometricsPlugin | null> {
  if (biometricsModule !== undefined) {
    return biometricsModule;
  }

  if (!isNative()) {
    biometricsModule = null;
    return null;
  }

  try {
    // Dynamic import — the plugin is only needed on native platforms
    const imported = await import('@capawesome-team/capacitor-biometrics');
    biometricsModule = imported as unknown as BiometricsPlugin;
    return biometricsModule;
  } catch {
    // Plugin not installed — biometric features will be disabled
    biometricsModule = null;
    return null;
  }
}

/**
 * Check if biometric authentication is available on this device.
 */
export async function isBiometricAvailable(): Promise<boolean> {
  const mod = await loadBiometricsModule();
  if (!mod) return false;

  try {
    const result = await mod.checkAvailability();
    return result.available;
  } catch {
    return false;
  }
}

/**
 * Check if the user has enabled biometric lock.
 */
export async function isBiometricEnabled(): Promise<boolean> {
  const value = await readSecureString(BIOMETRIC_ENABLED_KEY);
  return value === 'true';
}

/**
 * Enable or disable biometric lock.
 */
export async function setBiometricEnabled(enabled: boolean): Promise<void> {
  if (enabled) {
    await writeSecureString(BIOMETRIC_ENABLED_KEY, 'true');
  } else {
    await removeSecureString(BIOMETRIC_ENABLED_KEY);
  }
}

/**
 * Perform biometric authentication.
 * Returns true if successful, false if failed or cancelled.
 */
export async function authenticateWithBiometrics(): Promise<boolean> {
  const mod = await loadBiometricsModule();
  if (!mod) return true; // No biometric available, proceed

  try {
    await mod.authenticate({
      reason: '请验证身份以访问 Remote Code',
      iosFallbackTitle: '使用密码',
      androidTitle: '身份验证',
      androidSubtitle: '验证以继续',
      androidConfirmationRequired: false,
    });
    return true;
  } catch {
    return false;
  }
}

/**
 * Check if biometric auth should be performed and execute it.
 * Returns true if the user should proceed (either no biometric required, or auth succeeded).
 */
export async function performBiometricCheck(): Promise<boolean> {
  const enabled = await isBiometricEnabled();
  if (!enabled) {
    return true; // Biometric not enabled, proceed
  }

  const available = await isBiometricAvailable();
  if (!available) {
    return true; // No biometric hardware, proceed
  }

  return authenticateWithBiometrics();
}
