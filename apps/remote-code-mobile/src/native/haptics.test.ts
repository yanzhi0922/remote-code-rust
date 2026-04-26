import { describe, it, expect, vi, beforeEach } from 'vitest';

const { mockImpact, mockNotification, mockSelectionStart } = vi.hoisted(() => ({
  mockImpact: vi.fn(),
  mockNotification: vi.fn(),
  mockSelectionStart: vi.fn(),
}));

vi.mock('@capacitor/haptics', () => ({
  Haptics: {
    impact: mockImpact,
    notification: mockNotification,
    selectionStart: mockSelectionStart,
    selectionChanged: vi.fn(),
    selectionStopped: vi.fn(),
  },
  ImpactStyle: { Light: 'LIGHT', Medium: 'MEDIUM', Heavy: 'HEAVY' },
  NotificationType: { Success: 'SUCCESS', Warning: 'WARNING', Error: 'ERROR' },
}));

vi.mock('./platform', () => ({
  isNative: () => true,
}));

import {
  hapticLight,
  hapticMedium,
  hapticHeavy,
  hapticSuccess,
  hapticWarning,
  hapticError,
  hapticSelection,
} from './haptics';

describe('haptics', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('hapticLight calls impact with Light style', async () => {
    await hapticLight();
    expect(mockImpact).toHaveBeenCalledWith({ style: 'LIGHT' });
  });

  it('hapticMedium calls impact with Medium style', async () => {
    await hapticMedium();
    expect(mockImpact).toHaveBeenCalledWith({ style: 'MEDIUM' });
  });

  it('hapticHeavy calls impact with Heavy style', async () => {
    await hapticHeavy();
    expect(mockImpact).toHaveBeenCalledWith({ style: 'HEAVY' });
  });

  it('hapticSuccess calls notification with Success type', async () => {
    await hapticSuccess();
    expect(mockNotification).toHaveBeenCalledWith({ type: 'SUCCESS' });
  });

  it('hapticWarning calls notification with Warning type', async () => {
    await hapticWarning();
    expect(mockNotification).toHaveBeenCalledWith({ type: 'WARNING' });
  });

  it('hapticError calls notification with Error type', async () => {
    await hapticError();
    expect(mockNotification).toHaveBeenCalledWith({ type: 'ERROR' });
  });

  it('hapticSelection calls selectionStart', async () => {
    await hapticSelection();
    expect(mockSelectionStart).toHaveBeenCalled();
  });
});
