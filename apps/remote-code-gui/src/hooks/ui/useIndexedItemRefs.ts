/**
 * 索引化 Ref 管理 Hook — 为列表中的每一项创建独立的 ref
 * Indexed item refs hook — creates individual refs for each list item
 *
 * Adapted from AionUi useIndexedItemRefs pattern.
 */

import { useCallback, useEffect, useRef } from 'react';

/**
 * 管理一组按索引排列的 DOM 引用，常用于虚拟列表中的元素定位。
 * Manages an indexed array of DOM refs, commonly used for positioning in virtual lists.
 *
 * @param count 预期的元素数量
 * @returns itemRefs 和 setItemRef 回调
 */
export function useIndexedItemRefs<T>(count: number) {
  const itemRefs = useRef<Array<T | null>>([]);

  useEffect(() => {
    itemRefs.current = itemRefs.current.slice(0, count);
  }, [count]);

  const setItemRef = useCallback(
    (index: number) => (node: T | null) => {
      itemRefs.current[index] = node;
    },
    [],
  );

  return {
    itemRefs,
    setItemRef,
  };
}
