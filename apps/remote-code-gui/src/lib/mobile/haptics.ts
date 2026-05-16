import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

async function hapticNotification(kind: 'success' | 'warning' | 'error'): Promise<void> {
  if (!hasTauriRuntime()) return;
  return invoke('mobile_haptic_notification', { kind });
}

export const hapticSuccess = () => hapticNotification('success');
export const hapticWarning = () => hapticNotification('warning');
export const hapticError = () => hapticNotification('error');
