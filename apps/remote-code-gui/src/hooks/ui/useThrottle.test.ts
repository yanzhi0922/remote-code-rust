import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import { useThrottle } from './useThrottle';

describe('useThrottle', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  it('executes immediately on first call', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100, []));

    result.current('arg');

    expect(callback).toHaveBeenCalledWith('arg');
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('throttles subsequent calls within delay', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 200, []));

    result.current('first');
    result.current('second');

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith('first');
  });

  it('executes trailing call after delay', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100, []));

    result.current('first');

    act(() => {
      vi.advanceTimersByTime(50);
    });

    result.current('second');

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(callback).toHaveBeenCalledTimes(2);
    expect(callback).toHaveBeenLastCalledWith('second');
  });

  it('allows execution after delay period', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100, []));

    result.current('first');

    act(() => {
      vi.advanceTimersByTime(150);
    });

    result.current('second');

    expect(callback).toHaveBeenCalledTimes(2);
    expect(callback).toHaveBeenLastCalledWith('second');
  });

  it('cancels previous trailing timer on new call', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100, []));

    result.current('a');

    act(() => {
      vi.advanceTimersByTime(50);
    });

    result.current('b');

    act(() => {
      vi.advanceTimersByTime(30);
    });

    result.current('c');

    act(() => {
      vi.advanceTimersByTime(100);
    });

    // Should have first call + trailing call for 'c'
    expect(callback).toHaveBeenCalledTimes(2);
    expect(callback).toHaveBeenLastCalledWith('c');
  });

  it('handles multiple arguments', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100, []));

    result.current('x', 'y', 'z');

    expect(callback).toHaveBeenCalledWith('x', 'y', 'z');
  });
});
