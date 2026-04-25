import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, cleanup } from '@testing-library/react';
import { useLatestRef, useLatestCallback } from './useLatestRef';

describe('useLatestRef', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns a ref with the initial value', () => {
    const { result } = renderHook(() => useLatestRef('initial'));
    expect(result.current.current).toBe('initial');
  });

  it('updates ref.current when value changes', () => {
    const { result, rerender } = renderHook(({ val }) => useLatestRef(val), {
      initialProps: { val: 'first' },
    });

    expect(result.current.current).toBe('first');

    rerender({ val: 'second' });

    expect(result.current.current).toBe('second');
  });

  it('returns the same ref object across rerenders', () => {
    const { result, rerender } = renderHook(({ val }) => useLatestRef(val), {
      initialProps: { val: 1 },
    });

    const firstRef = result.current;
    rerender({ val: 2 });
    const secondRef = result.current;

    expect(firstRef).toBe(secondRef);
  });

  it('works with function values', () => {
    const fn1 = vi.fn();
    const fn2 = vi.fn();
    const { result, rerender } = renderHook(({ fn }) => useLatestRef(fn), {
      initialProps: { fn: fn1 },
    });

    expect(result.current.current).toBe(fn1);

    rerender({ fn: fn2 });

    expect(result.current.current).toBe(fn2);
  });
});

describe('useLatestCallback', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns a stable function reference', () => {
    const fn = vi.fn();
    const { result, rerender } = renderHook(({ cb }) => useLatestCallback(cb), {
      initialProps: { cb: fn },
    });

    const firstCallback = result.current;
    rerender({ cb: vi.fn() });
    const secondCallback = result.current;

    expect(firstCallback).toBe(secondCallback);
  });

  it('always calls the latest function', () => {
    const fn1 = vi.fn(() => 'first');
    const fn2 = vi.fn(() => 'second');

    const { result, rerender } = renderHook(({ cb }) => useLatestCallback(cb), {
      initialProps: { cb: fn1 },
    });

    result.current();
    expect(fn1).toHaveBeenCalledTimes(1);

    rerender({ cb: fn2 });

    result.current();
    expect(fn2).toHaveBeenCalledTimes(1);
    expect(fn1).toHaveBeenCalledTimes(1); // not called again
  });

  it('passes arguments to the callback', () => {
    const fn = vi.fn();
    const { result } = renderHook(() => useLatestCallback(fn));

    result.current('arg1', 'arg2');

    expect(fn).toHaveBeenCalledWith('arg1', 'arg2');
  });

  it('returns the callback return value', () => {
    const fn = vi.fn(() => 42);
    const { result } = renderHook(() => useLatestCallback(fn));

    const returnValue = result.current();
    expect(returnValue).toBe(42);
  });
});
