import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import { useDebounce } from './useDebounce';

describe('useDebounce', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  it('delays function execution', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 300, []));

    result.current('arg1');

    expect(callback).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(300);
    });

    expect(callback).toHaveBeenCalledWith('arg1');
  });

  it('only executes the last call in a series', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 100, []));

    result.current('first');
    result.current('second');
    result.current('third');

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith('third');
  });

  it('resets timer on each call', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 200, []));

    result.current('first');

    act(() => {
      vi.advanceTimersByTime(100);
    });

    result.current('second');

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(callback).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(callback).toHaveBeenCalledWith('second');
  });

  it('cleans up timer on unmount', () => {
    const callback = vi.fn();
    const { result, unmount } = renderHook(() => useDebounce(callback, 100, []));

    result.current('arg');

    unmount();

    act(() => {
      vi.advanceTimersByTime(200);
    });

    expect(callback).not.toHaveBeenCalled();
  });

  it('recreates debounced function when deps change', () => {
    const callback1 = vi.fn();
    const callback2 = vi.fn();
    const { result, rerender } = renderHook(
      ({ cb, deps }: { cb: () => void; deps: React.DependencyList }) => useDebounce(cb, 100, deps),
      { initialProps: { cb: callback1, deps: [1] } },
    );

    result.current();

    rerender({ cb: callback2, deps: [2] });

    // After deps change, the new debounced function should use callback2
    result.current();

    act(() => {
      vi.advanceTimersByTime(200);
    });

    // callback2 should be called (the new debounced function)
    expect(callback2).toHaveBeenCalled();
  });

  it('handles multiple arguments', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 50, []));

    result.current('a', 'b', 'c');

    act(() => {
      vi.advanceTimersByTime(50);
    });

    expect(callback).toHaveBeenCalledWith('a', 'b', 'c');
  });
});
