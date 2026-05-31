import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';
import i18n from '../../i18n';
import { isMobile } from './platform';

interface BiometricAvailability {
  available: boolean;
  biometry_type: string;
}

async function checkBiometricAvailability(): Promise<BiometricAvailability> {
  if (!hasTauriRuntime() || !await isMobile()) {
    return { available: false, biometry_type: 'unsupported' };
  }
  return invoke<BiometricAvailability>('mobile_biometric_check_availability');
}

async function authenticateWithBiometrics(reason: string): Promise<boolean> {
  if (!hasTauriRuntime()) return false;
  return invoke<boolean>('mobile_biometric_authenticate', { reason });
}

export async function performBiometricCheck(): Promise<boolean> {
  const enabled = await getBiometricEnabled();
  if (!enabled) return true;
  const avail = await checkBiometricAvailability();
  if (!avail.available) return true;
  return authenticateWithBiometrics(i18n.t('app.biometricPromptAuth'));
}

export async function getBiometricEnabled(): Promise<boolean> {
  const { secureStoreGet } = await import('./secureStorage');
  const val = await secureStoreGet('biometric_enabled');
  return val === 'true';
}

export async function setBiometricEnabled(enabled: boolean): Promise<boolean> {
  if (!enabled) {
    const { secureStoreSet } = await import('./secureStorage');
    await secureStoreSet('biometric_enabled', 'false');
    return true;
  }
  const avail = await checkBiometricAvailability();
  if (!avail.available) return false;
  const ok = await authenticateWithBiometrics(i18n.t('app.biometricEnablePrompt'));
  if (!ok) return false;
  const { secureStoreSet } = await import('./secureStorage');
  await secureStoreSet('biometric_enabled', 'true');
  return true;
}
