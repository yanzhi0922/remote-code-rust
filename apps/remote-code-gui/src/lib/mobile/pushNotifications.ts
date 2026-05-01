import { invoke } from '@tauri-apps/api/core';
import { hasTauriRuntime } from '../runtime';

interface PushNotificationPayload {
  type: 'approval' | 'session_update';
  approval_id?: string;
  session_id?: string;
}

type ApprovalHandler = (approvalId: string, sessionId: string) => void;
type SessionUpdateHandler = (sessionId: string) => void;

export interface PushNotificationOptions {
  onApproval: ApprovalHandler;
  onSessionUpdate: SessionUpdateHandler;
}

export async function initPushNotifications(_options: PushNotificationOptions): Promise<void> {
  if (!hasTauriRuntime()) return;
}

export async function requestPushPermission(): Promise<boolean> {
  if (!hasTauriRuntime()) return false;
  return false;
}

export async function registerPushTokenWithControlPlane(
  _baseUrl: string,
  _accessToken: string,
): Promise<void> {}

export async function getStoredPushToken(): Promise<string | null> {
  return null;
}

export async function clearPushToken(): Promise<void> {}
