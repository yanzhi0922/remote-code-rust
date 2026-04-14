/**
 * Push notification service for the mobile app.
 *
 * Handles:
 * - Permission request
 * - Token registration with the Control Plane
 * - Foreground notification display
 * - Notification action handling (open approval, view artifact)
 */

import { PushNotifications, type Token, type PushNotificationSchema, type ActionPerformed } from '@capacitor/push-notifications';
import { isNative } from './platform';
import { readSecureString, writeSecureString, removeSecureString } from './secureStorage';

const PUSH_TOKEN_KEY = 'push_token';
const PUSH_TOKEN_REGISTERED_KEY = 'push_token_registered';

type OnApprovalNotification = (approvalId: string, sessionId: string) => void;
type OnSessionNotification = (sessionId: string) => void;

interface NotificationHandlers {
  onApproval?: OnApprovalNotification;
  onSessionUpdate?: OnSessionNotification;
}

let handlers: NotificationHandlers = {};

/**
 * Initialize push notification listeners.
 * Should be called once during app startup.
 */
export async function initPushNotifications(listeners: NotificationHandlers): Promise<void> {
  handlers = { ...handlers, ...listeners };

  if (!isNative()) {
    return;
  }

  // Listen for registration token
  PushNotifications.addListener('registration', (token: Token) => {
    void handleRegistrationToken(token.value);
  });

  // Listen for registration errors
  PushNotifications.addListener('registrationError', (error) => {
    console.error('[Push] Registration error:', error.error);
  });

  // Listen for foreground notifications
  PushNotifications.addListener('pushNotificationReceived', (notification: PushNotificationSchema) => {
    console.log('[Push] Foreground notification:', notification.title, notification.body);
    handleForegroundNotification(notification);
  });

  // Listen for notification actions (tap)
  PushNotifications.addListener('pushNotificationActionPerformed', (action: ActionPerformed) => {
    handleNotificationAction(action);
  });
}

/**
 * Request push notification permission and register for remote notifications.
 */
export async function requestPushPermission(): Promise<boolean> {
  if (!isNative()) {
    return false;
  }

  let permStatus = await PushNotifications.checkPermissions();

  if (permStatus.receive === 'prompt') {
    permStatus = await PushNotifications.requestPermissions();
  }

  if (permStatus.receive !== 'granted') {
    console.warn('[Push] Permission denied');
    return false;
  }

  // Register with Apple/Google push services
  await PushNotifications.register();
  return true;
}

/**
 * Handle the FCM/APNs registration token.
 * Stores locally and registers with the Control Plane.
 */
async function handleRegistrationToken(token: string): Promise<void> {
  console.log('[Push] Received push token:', token.substring(0, 20) + '...');
  await writeSecureString(PUSH_TOKEN_KEY, token);
  // Token will be registered with Control Plane when the device authenticates
}

/**
 * Register the push token with the Control Plane.
 * Called after successful authentication.
 */
export async function registerPushTokenWithControlPlane(
  baseUrl: string,
  accessToken: string,
): Promise<void> {
  const token = await readSecureString(PUSH_TOKEN_KEY);
  if (!token) {
    return;
  }

  const alreadyRegistered = await readSecureString(PUSH_TOKEN_REGISTERED_KEY);
  if (alreadyRegistered === token) {
    return; // Already registered with this token
  }

  try {
    const response = await fetch(`${baseUrl}/v1/devices/push-token`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${accessToken}`,
      },
      body: JSON.stringify({ push_token: token }),
    });

    if (response.ok) {
      await writeSecureString(PUSH_TOKEN_REGISTERED_KEY, token);
      console.log('[Push] Token registered with Control Plane');
    } else {
      console.warn('[Push] Failed to register token:', response.status);
    }
  } catch (error) {
    console.warn('[Push] Error registering token:', error);
  }
}

/**
 * Handle a notification received while the app is in the foreground.
 */
function handleForegroundNotification(notification: PushNotificationSchema): void {
  const data = notification.data as Record<string, string> | undefined;
  if (!data) return;

  if (data['type'] === 'approval' && data['approval_id'] && data['session_id']) {
    handlers.onApproval?.(data['approval_id'], data['session_id']);
  } else if (data['type'] === 'session_update' && data['session_id']) {
    handlers.onSessionUpdate?.(data['session_id']);
  }
}

/**
 * Handle a notification action (user tapped on notification).
 */
function handleNotificationAction(action: ActionPerformed): void {
  const data = action.notification.data as Record<string, string> | undefined;
  if (!data) return;

  if (data['type'] === 'approval' && data['approval_id'] && data['session_id']) {
    handlers.onApproval?.(data['approval_id'], data['session_id']);
  } else if (data['type'] === 'session_update' && data['session_id']) {
    handlers.onSessionUpdate?.(data['session_id']);
  }
}

/**
 * Get the stored push token (if any).
 */
export async function getStoredPushToken(): Promise<string | null> {
  return readSecureString(PUSH_TOKEN_KEY);
}

/**
 * Clear push token data (on logout).
 */
export async function clearPushToken(): Promise<void> {
  await removeSecureString(PUSH_TOKEN_KEY);
  await removeSecureString(PUSH_TOKEN_REGISTERED_KEY);
}
