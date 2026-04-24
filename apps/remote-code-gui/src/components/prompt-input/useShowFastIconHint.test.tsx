import { renderHook, act, cleanup } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useShowFastIconHint } from './useShowFastIconHint';

afterEach(() => {
  cleanup();
});

describe('useShowFastIconHint', () => {
  it('starts hidden', () => {
    const { result } = renderHook(() => useShowFastIconHint());
    expect(result.current.visible).toBe(false);
  });

  it('shows on call', () => {
    const { result } = renderHook(() => useShowFastIconHint());
    act(() => result.current.show());
    expect(result.current.visible).toBe(true);
  });

  it('hides after duration', () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useShowFastIconHint({ duration: 1000 }));
    act(() => result.current.show());
    expect(result.current.visible).toBe(true);
    act(() => vi.advanceTimersByTime(1000));
    expect(result.current.visible).toBe(false);
    vi.useRealTimers();
  });
});
