import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

export async function hapticImpact(style: 'light' | 'medium' | 'heavy'): Promise<void> {
  if (!hasTauriRuntime()) return;
  return invoke('mobile_haptic_impact', { style });
}

export async function hapticNotification(kind: 'success' | 'warning' | 'error'): Promise<void> {
  if (!hasTauriRuntime()) return;
  return invoke('mobile_haptic_notification', { kind });
}

export async function hapticSelection(): Promise<void> {
  if (!hasTauriRuntime()) return;
  return invoke('mobile_haptic_selection');
}

export const hapticLight = () => hapticImpact('light');
export const hapticMedium = () => hapticImpact('medium');
export const hapticHeavy = () => hapticImpact('heavy');
export const hapticSuccess = () => hapticNotification('success');
export const hapticWarning = () => hapticNotification('warning');
export const hapticError = () => hapticNotification('error');
