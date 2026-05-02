import { hasTauriRuntime } from '../runtime';

type ApprovalHandler = (approvalId: string, sessionId: string) => void;
type SessionUpdateHandler = (sessionId: string) => void;

export interface PushNotificationOptions {
  onApproval: ApprovalHandler;
  onSessionUpdate: SessionUpdateHandler;
}

// TODO: wire up native push notification initialization pending mobile platform support
export async function initPushNotifications(_options: PushNotificationOptions): Promise<void> {
  if (!hasTauriRuntime()) return;
}

// TODO: request native push notification permission pending mobile platform support
export async function requestPushPermission(): Promise<boolean> {
  if (!hasTauriRuntime()) return false;
  return false;
}

// TODO: register push token with control plane pending mobile platform support
export async function registerPushTokenWithControlPlane(
  _baseUrl: string,
  _accessToken: string,
): Promise<void> {}

// TODO: retrieve stored push token pending mobile platform support
export async function getStoredPushToken(): Promise<string | null> {
  return null;
}

// TODO: clear stored push token pending mobile platform support
export async function clearPushToken(): Promise<void> {}
