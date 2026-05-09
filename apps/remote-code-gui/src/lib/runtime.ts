declare global {
  interface Window {
    __TAURI__?: unknown;
    __TAURI_INTERNALS__?: unknown;
  }
}

const REMOTE_ACCESS_TOKEN_STORAGE_KEY = 'remote-code-control-plane-access-token';
const REMOTE_REFRESH_TOKEN_STORAGE_KEY = 'remote-code-control-plane-refresh-token';
const REMOTE_ACTIVE_SESSION_STORAGE_KEY_PREFIX = 'remote-code-control-plane-active-session:';
const REMOTE_PAIRING_OFFER_STORAGE_KEY = 'remote_pairing_offer_id';
const REMOTE_PAIRING_SECRET_STORAGE_KEY = 'remote_pairing_secret';

export function hasTauriRuntime(): boolean {
  return Boolean(window.__TAURI__ || window.__TAURI_INTERNALS__);
}

export function resolveRemoteBaseUrl(): string | null {
  const params = new URLSearchParams(window.location.search);
  const queryValue = params.get('control_plane_url')?.trim();
  if (queryValue) {
    return normalizeBaseUrl(queryValue);
  }

  const envValue = import.meta.env.VITE_REMOTE_CONTROL_PLANE_URL?.trim();
  if (envValue) {
    return normalizeBaseUrl(envValue);
  }

  if (window.location.protocol === 'http:' || window.location.protocol === 'https:') {
    return normalizeBaseUrl(window.location.origin);
  }

  return null;
}

export function resolveRemoteAccessToken(): string | null {
  const params = new URLSearchParams(window.location.search);
  const queryValue = params.get('access_token')?.trim() ?? params.get('token')?.trim();
  if (queryValue) {
    return queryValue;
  }

  const envValue = import.meta.env.VITE_REMOTE_CONTROL_PLANE_TOKEN?.trim();
  if (envValue) {
    return envValue;
  }

  try {
    const storedValue = window.localStorage.getItem(REMOTE_ACCESS_TOKEN_STORAGE_KEY)?.trim();
    if (storedValue) {
      return storedValue;
    }
  } catch {
    // Ignore storage access failures.
  }

  return null;
}

export function persistRemoteAccessToken(token: string): void {
  const normalized = token.trim();
  if (!normalized) {
    return;
  }
  try {
    window.localStorage.setItem(REMOTE_ACCESS_TOKEN_STORAGE_KEY, normalized);
  } catch {
    // Ignore storage access failures.
  }
}

export function clearRemoteAccessToken(): void {
  try {
    window.localStorage.removeItem(REMOTE_ACCESS_TOKEN_STORAGE_KEY);
  } catch {
    // Ignore storage access failures.
  }
}

/**
 * Derive a tenant-scoping user key from username and password.
 * Uses SHA-256(username:password) — the server never sees the plaintext,
 * only the irreversible hash which serves as both auth token and tenant ID.
 */
export async function deriveUserKey(username: string, password: string): Promise<string> {
  const raw = `${username}:${password}`;
  const encoded = new TextEncoder().encode(raw);
  const digest = await crypto.subtle.digest('SHA-256', encoded);
  const bytes = new Uint8Array(digest);
  let hex = '';
  for (const byte of bytes) {
    hex += byte.toString(16).padStart(2, '0');
  }
  return hex;
}

export function resolveRemoteRefreshToken(): string | null {
  try {
    return window.localStorage.getItem(REMOTE_REFRESH_TOKEN_STORAGE_KEY)?.trim() ?? null;
  } catch {
    return null;
  }
}

export function persistRemoteRefreshToken(token: string): void {
  const normalized = token.trim();
  if (!normalized) return;
  try {
    window.localStorage.setItem(REMOTE_REFRESH_TOKEN_STORAGE_KEY, normalized);
  } catch {
    // Ignore storage access failures.
  }
}

export function clearRemoteRefreshToken(): void {
  try {
    window.localStorage.removeItem(REMOTE_REFRESH_TOKEN_STORAGE_KEY);
  } catch {
    // Ignore storage access failures.
  }
}

export function resolveRemoteActiveSessionId(baseUrl: string | null): string | null {
  const storageKey = buildRemoteActiveSessionStorageKey(baseUrl);
  if (!storageKey) {
    return null;
  }
  try {
    return window.localStorage.getItem(storageKey)?.trim() ?? null;
  } catch {
    // Ignore storage access failures.
    return null;
  }
}

export function persistRemoteActiveSessionId(baseUrl: string | null, sessionId: string): void {
  const storageKey = buildRemoteActiveSessionStorageKey(baseUrl);
  const normalized = sessionId.trim();
  if (!storageKey || !normalized) {
    return;
  }
  try {
    window.localStorage.setItem(storageKey, normalized);
  } catch {
    // Ignore storage access failures.
  }
}

export function clearRemoteActiveSessionId(baseUrl: string | null): void {
  const storageKey = buildRemoteActiveSessionStorageKey(baseUrl);
  if (!storageKey) {
    return;
  }
  try {
    window.localStorage.removeItem(storageKey);
  } catch {
    // Ignore storage access failures.
  }
}

export function resolveRemotePairingContext(): { offerId: string | null; pairingSecret: string | null } {
  const params = new URLSearchParams(window.location.search);
  return {
    offerId: params.get('pairing_offer')?.trim() ?? null,
    pairingSecret: params.get('pairing_secret')?.trim() ?? null,
  };
}

export function clearRemotePairingContext(): void {
  try {
    window.localStorage.removeItem(REMOTE_PAIRING_OFFER_STORAGE_KEY);
    window.localStorage.removeItem(REMOTE_PAIRING_SECRET_STORAGE_KEY);
  } catch {
    // Ignore storage access failures.
  }
}

export function stripRemoteSensitiveQueryParams(): void {
  const url = new URL(window.location.href);
  let changed = false;
  for (const key of ['access_token', 'token', 'pairing_offer', 'pairing_secret']) {
    if (url.searchParams.has(key)) {
      url.searchParams.delete(key);
      changed = true;
    }
  }
  if (changed) {
    window.history.replaceState({}, document.title, url.toString());
  }
}

export function shouldUseRemoteMode(): boolean {
  const params = new URLSearchParams(window.location.search);
  const mode = params.get('mode');
  if (mode === 'local') {
    return false;
  }
  if (mode === 'remote') {
    return true;
  }
  if (hasTauriRuntime()) {
    return false;
  }
  return resolveRemoteBaseUrl() !== null;
}

function normalizeBaseUrl(raw: string): string | null {
  try {
    const parsed = new URL(raw);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      return null;
    }
    parsed.hash = '';
    return parsed.toString().replace(/\/$/, '');
  } catch {
    return null;
  }
}

function buildRemoteActiveSessionStorageKey(baseUrl: string | null): string | null {
  if (!baseUrl) {
    return null;
  }
  const normalized = normalizeBaseUrl(baseUrl);
  if (!normalized) {
    return null;
  }
  return `${REMOTE_ACTIVE_SESSION_STORAGE_KEY_PREFIX}${normalized}`;
}