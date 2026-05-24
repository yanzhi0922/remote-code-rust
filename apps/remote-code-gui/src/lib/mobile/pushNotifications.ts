import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { hasTauriRuntime } from '../runtime';

type ApprovalHandler = (approvalId: string, sessionId: string) => void;
type SessionUpdateHandler = (sessionId: string) => void;

export interface PushNotificationOptions {
  onApproval: ApprovalHandler;
  onSessionUpdate: SessionUpdateHandler;
}

interface PushPayload {
  type: string;
  approvalId?: string;
  sessionId?: string;
}

/**
 * Type guard that validates a push notification payload at runtime.
 * Ensures required fields are present and have the expected types before use.
 */
function isValidPushPayload(payload: unknown): payload is PushPayload {
  if (typeof payload !== 'object' || payload === null) return false;
  const p = payload as Record<string, unknown>;
  if (typeof p.type !== 'string') return false;
  if (p.approvalId !== undefined && typeof p.approvalId !== 'string') return false;
  if (p.sessionId !== undefined && typeof p.sessionId !== 'string') return false;
  return true;
}

let _pushToken: string | null = null;
let _permissionGranted = false;
let _unlistenNotification: (() => void) | null = null;
let _unlistenNotificationClicked: (() => void) | null = null;

export async function initPushNotifications(options: PushNotificationOptions): Promise<void> {
  if (!hasTauriRuntime()) return;

  // Remove previous listeners before registering new ones to avoid duplicates.
  _unlistenNotification?.();
  _unlistenNotification = null;
  _unlistenNotificationClicked?.();
  _unlistenNotificationClicked = null;

  // Request permission
  _permissionGranted = await invoke<boolean>('mobile_push_request_permission');
  if (!_permissionGranted) return;

  // Get and register token
  _pushToken = await invoke<string | null>('mobile_push_get_token');

  // Listen for push notification events
  _unlistenNotification = await listen('mobile://push-notification', (event) => {
    if (!isValidPushPayload(event.payload)) return;
    const payload = event.payload;
    if (payload.type === 'approval' && payload.approvalId && payload.sessionId) {
      options.onApproval(payload.approvalId, payload.sessionId);
    } else if (payload.type === 'session-update' && payload.sessionId) {
      options.onSessionUpdate(payload.sessionId);
    }
  });

  _unlistenNotificationClicked = await listen('mobile://push-notification-clicked', (event) => {
    if (!isValidPushPayload(event.payload)) return;
    const payload = event.payload;
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
): Promise<boolean> {
  if (!hasTauriRuntime() || !_permissionGranted || !_pushToken) return false;
  return await invoke<boolean>('mobile_push_register_token', {
    baseUrl,
    accessToken,
    pushToken: _pushToken,
  });
}

export async function showLocalNotification(title: string, body: string, data?: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke('mobile_push_show', { title, body, data: data ?? null });
}
