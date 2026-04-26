import { describe, it, expect, vi, beforeEach } from 'vitest';

const { mockGet, mockSet, mockRemove } = vi.hoisted(() => ({
  mockGet: vi.fn(),
  mockSet: vi.fn(),
  mockRemove: vi.fn(),
}));

vi.mock('@capacitor/preferences', () => ({
  Preferences: {
    get: mockGet,
    set: mockSet,
    remove: mockRemove,
  },
}));

import { readSecureString, writeSecureString, removeSecureString } from './secureStorage';

describe('secureStorage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('readSecureString returns value from Preferences', async () => {
    mockGet.mockResolvedValue({ value: 'test-value' });
    const result = await readSecureString('test-key');
    expect(mockGet).toHaveBeenCalled();
    expect(result).toBe('test-value');
  });

  it('readSecureString returns null when no value', async () => {
    mockGet.mockResolvedValue({ value: null });
    const result = await readSecureString('missing-key');
    expect(result).toBeNull();
  });

  it('writeSecureString calls Preferences.set', async () => {
    await writeSecureString('my-key', 'my-value');
    expect(mockSet).toHaveBeenCalled();
  });

  it('removeSecureString calls Preferences.remove', async () => {
    await removeSecureString('old-key');
    expect(mockRemove).toHaveBeenCalled();
  });
});
