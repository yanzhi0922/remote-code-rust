import { describe, it, expect, vi, beforeEach } from 'vitest';

const { mockIsNative, mockGetPlatform } = vi.hoisted(() => ({
  mockIsNative: vi.fn(() => false),
  mockGetPlatform: vi.fn(() => 'web'),
}));

vi.mock('@capacitor/core', () => ({
  Capacitor: {
    isNativePlatform: mockIsNative,
    getPlatform: mockGetPlatform,
  },
}));

import { getPlatform, isNative } from './platform';

describe('platform', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns "web" when not native platform', () => {
    mockIsNative.mockReturnValue(false);
    expect(getPlatform()).toBe('web');
  });

  it('returns "android" when Capacitor reports android', () => {
    mockIsNative.mockReturnValue(true);
    mockGetPlatform.mockReturnValue('android');
    expect(getPlatform()).toBe('android');
  });

  it('returns "ios" when Capacitor reports ios', () => {
    mockIsNative.mockReturnValue(true);
    mockGetPlatform.mockReturnValue('ios');
    expect(getPlatform()).toBe('ios');
  });

  it('isNative returns true on native platform', () => {
    mockIsNative.mockReturnValue(true);
    expect(isNative()).toBe(true);
  });

  it('isNative returns false on web', () => {
    mockIsNative.mockReturnValue(false);
    expect(isNative()).toBe(false);
  });
});
