/**
 * Mobile runtime — drop-in replacement for the web runtime.ts.
 *
 * Provides the same API surface but uses Capacitor Preferences for
 * secure token storage instead of raw localStorage.
 */

import { readSecureString, writeSecureString, removeSecureString } from '../native/secureStorage';
import { initAppLifecycle } from '../native/appLifecycle';

// ─── Storage keys (must match the web runtime) ──────────────────────

const STORAGE_KEY_BASE_URL = 'remote_base_url';
const STORAGE_KEY_ACCESS_TOKEN = 'remote_access_token';
const STORAGE_KEY_PAIRING_OFFER_ID = 'remote_pairing_offer_id';
const STORAGE_KEY_PAIRING_SECRET = 'remote_pairing_secret';

function sessionKey(baseUrl: string): string {
  return `remote_active_session:${baseUrl}`;
}

// ─── Base URL ───────────────────────────────────────────────────────

/**
 * Resolve the control plane base URL.
 * Returns synchronously from localStorage cache (populated during initMobileRuntime).
 */
export function resolveRemoteBaseUrl(): string | null {
  const raw = localStorage.getItem(STORAGE_KEY_BASE_URL);
  if (!raw) return null;
  try {
    const parsed = new URL(raw);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      return null;
    }
    return parsed.toString().replace(/\/$/, '');
  } catch {
    return null;
  }
}

/**
 * Persist the control plane base URL.
 */
export async function persistRemoteBaseUrl(url: string): Promise<void> {
  localStorage.setItem(STORAGE_KEY_BASE_URL, url);
  await writeSecureString(STORAGE_KEY_BASE_URL, url);
}

// ─── Access Token ───────────────────────────────────────────────────

/**
 * Resolve the stored access token.
 * Returns synchronously from localStorage cache (populated on startup).
 */
export function resolveRemoteAccessToken(): string | null {
  return localStorage.getItem(STORAGE_KEY_ACCESS_TOKEN);
}

/**
 * Persist the access token securely.
 */
export async function persistRemoteAccessToken(token: string): Promise<void> {
  localStorage.setItem(STORAGE_KEY_ACCESS_TOKEN, token);
  await writeSecureString(STORAGE_KEY_ACCESS_TOKEN, token);
}

/**
 * Clear the stored access token.
 */
export async function clearRemoteAccessToken(): Promise<void> {
  localStorage.removeItem(STORAGE_KEY_ACCESS_TOKEN);
  await removeSecureString(STORAGE_KEY_ACCESS_TOKEN);
}

// ─── Active Session ─────────────────────────────────────────────────

export function resolveRemoteActiveSessionId(baseUrl: string | null): string | null {
  if (!baseUrl) return null;
  return localStorage.getItem(sessionKey(baseUrl));
}

export async function persistRemoteActiveSessionId(
  baseUrl: string | null,
  sessionId: string,
): Promise<void> {
  if (!baseUrl) return;
  const key = sessionKey(baseUrl);
  localStorage.setItem(key, sessionId);
  await writeSecureString(key, sessionId);
}

export async function clearRemoteActiveSessionId(baseUrl: string | null): Promise<void> {
  if (!baseUrl) return;
  const key = sessionKey(baseUrl);
  localStorage.removeItem(key);
  await removeSecureString(key);
}

// ─── Pairing Context ────────────────────────────────────────────────

export function resolveRemotePairingContext(): {
  offerId: string | null;
  pairingSecret: string | null;
} {
  return {
    offerId: localStorage.getItem(STORAGE_KEY_PAIRING_OFFER_ID)?.trim() ?? null,
    pairingSecret: localStorage.getItem(STORAGE_KEY_PAIRING_SECRET)?.trim() ?? null,
  };
}

export async function persistRemotePairingContext(
  offerId: string,
  pairingSecret: string,
): Promise<void> {
  const normalizedOfferId = offerId.trim();
  const normalizedPairingSecret = pairingSecret.trim();
  if (!normalizedOfferId || !normalizedPairingSecret) {
    return;
  }

  localStorage.setItem(STORAGE_KEY_PAIRING_OFFER_ID, normalizedOfferId);
  localStorage.setItem(STORAGE_KEY_PAIRING_SECRET, normalizedPairingSecret);
  await Promise.all([
    writeSecureString(STORAGE_KEY_PAIRING_OFFER_ID, normalizedOfferId),
    writeSecureString(STORAGE_KEY_PAIRING_SECRET, normalizedPairingSecret),
  ]);
}

export async function clearRemotePairingContext(): Promise<void> {
  localStorage.removeItem(STORAGE_KEY_PAIRING_OFFER_ID);
  localStorage.removeItem(STORAGE_KEY_PAIRING_SECRET);
  await Promise.all([
    removeSecureString(STORAGE_KEY_PAIRING_OFFER_ID),
    removeSecureString(STORAGE_KEY_PAIRING_SECRET),
  ]);
}

// ─── URL Helpers ────────────────────────────────────────────────────

/**
 * Strip sensitive query parameters from the current URL.
 * On mobile this is a no-op since we don't use URL params for auth.
 */
export function stripRemoteSensitiveQueryParams(): void {
  // No-op on mobile — tokens are passed via secure storage, not URL params
}

// ─── Mode Detection ─────────────────────────────────────────────────

/**
 * Always returns true on mobile — we are always in remote mode.
 */
export function shouldUseRemoteMode(): boolean {
  return true;
}

// ─── Mobile Initialization ──────────────────────────────────────────

/**
 * Pre-load secure values into localStorage for synchronous access.
 * Must be called before React renders.
 */
export async function initMobileRuntime(): Promise<void> {
  const [baseUrl, accessToken, pairingOfferId, pairingSecret] = await Promise.all([
    readSecureString(STORAGE_KEY_BASE_URL),
    readSecureString(STORAGE_KEY_ACCESS_TOKEN),
    readSecureString(STORAGE_KEY_PAIRING_OFFER_ID),
    readSecureString(STORAGE_KEY_PAIRING_SECRET),
  ]);

  if (baseUrl) {
    localStorage.setItem(STORAGE_KEY_BASE_URL, baseUrl);
  }
  if (accessToken) {
    localStorage.setItem(STORAGE_KEY_ACCESS_TOKEN, accessToken);
  }
  if (pairingOfferId) {
    localStorage.setItem(STORAGE_KEY_PAIRING_OFFER_ID, pairingOfferId);
  }
  if (pairingSecret) {
    localStorage.setItem(STORAGE_KEY_PAIRING_SECRET, pairingSecret);
  }

  // Initialize app lifecycle listeners
  initAppLifecycle({
    onResume: () => {
      // WebSocket reconnection is handled by RemoteApp.tsx's
      // visibility change listener, which also fires in WebView
    },
    onPause: () => {
      // No special handling needed — WebSocket will auto-reconnect
    },
    onNetworkChange: (connected) => {
      if (connected) {
        // Network restored — RemoteApp will detect via its own polling
        window.dispatchEvent(new CustomEvent('network-restored'));
      }
    },
  });
}
