import { describe, it, expect, vi, beforeEach } from 'vitest';

import { parsePairingUrl, buildPairingUrl } from './deepLink';

describe('deepLink', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('parsePairingUrl extracts offerId and secret from valid URL', () => {
    const result = parsePairingUrl('remote-code://pair?offerId=offer-123&secret=secret-456');
    expect(result).toEqual({ offerId: 'offer-123', secret: 'secret-456' });
  });

  it('parsePairingUrl returns null for non-pairing URLs', () => {
    expect(parsePairingUrl('not-a-url')).toBeNull();
  });

  it('parsePairingUrl returns null when missing params', () => {
    expect(parsePairingUrl('remote-code://pair?offerId=offer-123')).toBeNull();
    expect(parsePairingUrl('remote-code://pair?secret=secret-456')).toBeNull();
  });

  it('buildPairingUrl constructs valid URL', () => {
    const url = buildPairingUrl('https://example.com', 'offer-abc', 'secret-xyz');
    expect(url).toContain('https://example.com/pair');
    expect(url).toContain('offerId=offer-abc');
    expect(url).toContain('secret=secret-xyz');
  });
});
