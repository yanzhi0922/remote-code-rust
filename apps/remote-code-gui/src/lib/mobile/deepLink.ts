import { hasTauriRuntime } from '../runtime';
import { listen } from '@tauri-apps/api/event';

export interface DeepLinkPairing {
  offerId: string;
  secret: string;
}

type DeepLinkHandler = (url: string, path: string, params: Record<string, string>) => void;

export async function initDeepLinks(handler: DeepLinkHandler): Promise<void> {
  if (!hasTauriRuntime()) return;

  await listen<string>('mobile://deep-link', (event) => {
    const url = event.payload;
    const parsed = parseDeepLink(url);
    if (parsed) {
      handler(url, parsed.path, parsed.params);
    }
  });
}

export function parseDeepLink(url: string): { path: string; params: Record<string, string> } | null {
  try {
    const parsed = new URL(url);
    const params: Record<string, string> = {};
    parsed.searchParams.forEach((v, k) => { params[k] = v; });
    return { path: parsed.pathname, params };
  } catch {
    return null;
  }
}

export function parsePairingUrl(url: string): DeepLinkPairing | null {
  const parsed = parseDeepLink(url);
  if (!parsed) return null;
  if (parsed.path === '/pair' || parsed.path === '/pair/' || parsed.params['pairing_offer']) {
    const offerId = parsed.params['offerId'] ?? parsed.params['pairing_offer'];
    const secret = parsed.params['secret'] ?? parsed.params['pairing_secret'];
    if (offerId && secret) return { offerId, secret };
  }
  return null;
}

export function buildPairingUrl(baseUrl: string, offerId: string, secret: string): string {
  return `${baseUrl}/pair?offerId=${encodeURIComponent(offerId)}&secret=${encodeURIComponent(secret)}`;
}
