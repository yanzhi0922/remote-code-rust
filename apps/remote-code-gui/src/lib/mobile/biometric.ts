import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';
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
  return authenticateWithBiometrics('请验证身份以访问 Remote Code');
}

async function getBiometricEnabled(): Promise<boolean> {
  const { secureStoreGet } = await import('./secureStorage');
  const val = await secureStoreGet('biometric_enabled');
  return val === 'true';
}
