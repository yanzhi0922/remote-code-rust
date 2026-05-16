import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { hasTauriRuntime } from '../runtime';

type ApprovalHandler = (approvalId: string, sessionId: string) => void;
type SessionUpdateHandler = (sessionId: string) => void;

export interface PushNotificationOptions {
  onApproval: ApprovalHandler;
  onSessionUpdate: SessionUpdateHandler;
}

let _pushToken: string | null = null;
let _permissionGranted = false;

export async function initPushNotifications(options: PushNotificationOptions): Promise<void> {
  if (!hasTauriRuntime()) return;

  // Request permission
  _permissionGranted = await invoke<boolean>('mobile_push_request_permission');
  if (!_permissionGranted) return;

  // Get and register token
  _pushToken = await invoke<string | null>('mobile_push_get_token');

  // Listen for push notification events
  await listen('mobile://push-notification', (event) => {
    const payload = event.payload as { type: string; approvalId?: string; sessionId?: string };
    if (payload.type === 'approval' && payload.approvalId && payload.sessionId) {
      options.onApproval(payload.approvalId, payload.sessionId);
    } else if (payload.type === 'session-update' && payload.sessionId) {
      options.onSessionUpdate(payload.sessionId);
    }
  });

  await listen('mobile://push-notification-clicked', (event) => {
    const payload = event.payload as { type: string; approvalId?: string; sessionId?: string };
    if (payload.type === 'approval' && payload.approvalId && payload.sessionId) {
      options.onApproval(payload.approvalId, payload.sessionId);
    } else if (payload.type === 'session-update' && payload.sessionId) {
      options.onSessionUpdate(payload.sessionId);
    }
  });
}

export async function registerPushTokenWithControlPlane(
  baseUrl: string,
  accessToken: string,
): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke('mobile_push_register_token', { baseUrl, accessToken });
}

export async function showLocalNotification(title: string, body: string, data?: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke('mobile_push_show', { title, body, data: data ?? null });
}
