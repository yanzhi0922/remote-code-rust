import { describe, it, expect, vi, beforeEach } from 'vitest';

const { mockGetStatus, mockAddListener } = vi.hoisted(() => ({
  mockGetStatus: vi.fn(),
  mockAddListener: vi.fn(),
}));

vi.mock('@capacitor/network', () => ({
  Network: {
    getStatus: mockGetStatus,
    addListener: mockAddListener,
  },
}));

vi.mock('./platform', () => ({
  isNative: () => true,
}));

describe('network', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('getNetworkStatus returns current status', async () => {
    mockGetStatus.mockResolvedValue({ connected: true, connectionType: 'wifi' });
    const { getNetworkStatus } = await import('./network');
    const status = await getNetworkStatus();
    expect(status.connected).toBe(true);
    expect(status.connectionType).toBe('wifi');
  });

  it('isOnline returns true when connected', async () => {
    mockGetStatus.mockResolvedValue({ connected: true, connectionType: 'wifi' });
    const { isOnline } = await import('./network');
    expect(await isOnline()).toBe(true);
  });

  it('isOnline returns false when disconnected', async () => {
    mockGetStatus.mockResolvedValue({ connected: false, connectionType: 'none' });
    const { isOnline } = await import('./network');
    expect(await isOnline()).toBe(false);
  });

  it('describeConnectionType returns human-readable strings', async () => {
    const { describeConnectionType } = await import('./network');
    expect(describeConnectionType('wifi')).toBe('WiFi');
    expect(describeConnectionType('cellular')).toBe('蜂窝网络');
    expect(describeConnectionType('none')).toBe('无网络');
    expect(describeConnectionType('unknown')).toBe('未知');
  });
});
