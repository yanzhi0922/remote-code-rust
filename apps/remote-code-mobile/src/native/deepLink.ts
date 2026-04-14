/**
 * Deep Link handling for the mobile app.
 *
 * Handles incoming URLs from:
 * - QR code scans (pairing URLs)
 * - Push notification deep links
 * - Custom URL scheme (remotecode://)
 * - Universal Links / App Links (https://)
 */

import { App, type URLOpenListenerEvent } from '@capacitor/app';
import { isNative } from './platform';

type DeepLinkHandler = (url: string, path: string, params: Record<string, string>) => void;

let deepLinkHandler: DeepLinkHandler | null = null;

/**
 * Initialize deep link listener.
 */
export function initDeepLinks(handler: DeepLinkHandler): void {
  deepLinkHandler = handler;

  if (!isNative()) {
    return;
  }

  App.addListener('appUrlOpen', (event: URLOpenListenerEvent) => {
    handleDeepLink(event.url);
  });
}

/**
 * Handle an incoming deep link URL.
 *
 * Supported URL patterns:
 * - remotecode://pair?offerId=xxx&secret=yyy
 * - remotecode://session?id=xxx
 * - https://remotecode.app/pair?offerId=xxx&secret=yyy
 */
function handleDeepLink(url: string): void {
  if (!deepLinkHandler) return;

  try {
    const parsed = new URL(url);
    const params: Record<string, string> = {};

    parsed.searchParams.forEach((value, key) => {
      params[key] = value;
    });

    deepLinkHandler(url, parsed.pathname, params);
  } catch {
    console.warn('[DeepLink] Invalid URL:', url);
  }
}

/**
 * Parse a pairing URL and extract offer ID and secret.
 *
 * @returns Pairing data or null if not a valid pairing URL
 */
export function parsePairingUrl(url: string): { offerId: string; secret: string } | null {
  try {
    const parsed = new URL(url);
    const offerId = parsed.searchParams.get('offerId');
    const secret = parsed.searchParams.get('secret');

    if (offerId && secret) {
      return { offerId, secret };
    }
  } catch {
    // Invalid URL
  }

  return null;
}

/**
 * Build a pairing URL for QR code generation.
 */
export function buildPairingUrl(
  baseUrl: string,
  offerId: string,
  secret: string,
): string {
  const url = new URL(baseUrl);
  url.pathname = '/pair';
  url.searchParams.set('offerId', offerId);
  url.searchParams.set('secret', secret);
  return url.toString();
}
