import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';
import { isMobile } from './platform';

export interface BiometricAvailability {
  available: boolean;
  biometry_type: string;
}

export async function checkBiometricAvailability(): Promise<BiometricAvailability> {
  if (!hasTauriRuntime() || !await isMobile()) {
    return { available: false, biometry_type: 'unsupported' };
  }
  return invoke<BiometricAvailability>('mobile_biometric_check_availability');
}

export async function authenticateWithBiometrics(reason: string): Promise<boolean> {
  if (!hasTauriRuntime()) return false;
  return invoke<boolean>('mobile_biometric_authenticate', { reason });
}

export async function performBiometricCheck(): Promise<boolean> {
  const enabled = await getBiometricEnabled();
  if (!enabled) return true;
  const avail = await checkBiometricAvailability();
  if (!avail.available) return true;
  return authenticateWithBiometrics('请验证身份以访问 Remote Code');
}

export async function getBiometricEnabled(): Promise<boolean> {
  const { secureStoreGet } = await import('./secureStorage');
  const val = await secureStoreGet('biometric_enabled');
  return val === 'true';
}

export async function setBiometricEnabled(enabled: boolean): Promise<void> {
  const { secureStoreSet, secureStoreRemove } = await import('./secureStorage');
  if (enabled) {
    await secureStoreSet('biometric_enabled', 'true');
  } else {
    await secureStoreRemove('biometric_enabled');
  }
}
