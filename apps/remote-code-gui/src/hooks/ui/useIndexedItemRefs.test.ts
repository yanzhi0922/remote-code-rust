import { describe, it, expect, afterEach } from 'vitest';
import { renderHook, cleanup } from '@testing-library/react';
import { useIndexedItemRefs } from './useIndexedItemRefs';

describe('useIndexedItemRefs', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns itemRefs and setItemRef', () => {
    const { result } = renderHook(() => useIndexedItemRefs<HTMLDivElement>(3));

    expect(result.current.itemRefs).toBeDefined();
    expect(result.current.setItemRef).toBeDefined();
    expect(typeof result.current.setItemRef).toBe('function');
  });

  it('setItemRef sets refs by index', () => {
    const { result } = renderHook(() => useIndexedItemRefs<HTMLDivElement>(3));

    const mockNode = document.createElement('div');
    result.current.setItemRef(0)(mockNode);
    result.current.setItemRef(1)(null);
    result.current.setItemRef(2)(document.createElement('div'));

    expect(result.current.itemRefs.current[0]).toBe(mockNode);
    expect(result.current.itemRefs.current[1]).toBeNull();
    expect(result.current.itemRefs.current[2]).toBeDefined();
  });

  it('setItemRef can be called multiple times for same index', () => {
    const { result } = renderHook(() => useIndexedItemRefs<HTMLDivElement>(2));

    const node1 = document.createElement('div');
    const node2 = document.createElement('div');

    result.current.setItemRef(0)(node1);
    expect(result.current.itemRefs.current[0]).toBe(node1);

    result.current.setItemRef(0)(node2);
    expect(result.current.itemRefs.current[0]).toBe(node2);
  });

  it('truncates refs array when count decreases', () => {
    const { result, rerender } = renderHook(
      ({ count }) => useIndexedItemRefs<HTMLDivElement>(count),
      { initialProps: { count: 5 } },
    );

    const nodes = Array.from({ length: 5 }, () => document.createElement('div'));
    nodes.forEach((node, i) => result.current.setItemRef(i)(node));

    expect(result.current.itemRefs.current.length).toBe(5);

    rerender({ count: 3 });

    expect(result.current.itemRefs.current.length).toBe(3);
    expect(result.current.itemRefs.current[0]).toBe(nodes[0]);
  });

  it('setItemRef creates a ref setter function for each index', () => {
    const { result } = renderHook(() => useIndexedItemRefs<HTMLDivElement>(3));

    const setter = result.current.setItemRef(0);
    expect(typeof setter).toBe('function');

    const node = document.createElement('div');
    setter(node);
    expect(result.current.itemRefs.current[0]).toBe(node);
  });

  it('initializes with empty array', () => {
    const { result } = renderHook(() => useIndexedItemRefs<HTMLDivElement>(0));

    expect(result.current.itemRefs.current).toEqual([]);
  });
});
