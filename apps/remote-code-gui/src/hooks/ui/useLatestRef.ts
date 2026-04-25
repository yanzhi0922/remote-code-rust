/**
 * 保持值的最新引用，避免闭包陷阱
 * Keep the latest reference of a value to avoid closure trap
 *
 * Adapted from AionUi useLatestRef / useLatestCallback pattern.
 */

import { useCallback, useLayoutEffect, useRef } from 'react';

/**
 * 返回一个 ref，其 `.current` 始终指向最新传入的值。
 * Returns a ref whose `.current` always points to the latest value.
 *
 * @example
 * ```tsx
 * const callbackRef = useLatestRef(onChange);
 * useEffect(() => {
 *   const handler = (text: string) => callbackRef.current(text);
 *   emitter.on('change', handler);
 *   return () => emitter.off('change', handler);
 * }, []); // 依赖数组为空，不会因为 onChange 变化而重新注册
 * ```
 */
export function useLatestRef<T>(value: T) {
  const ref = useRef(value);

  // 使用 useLayoutEffect 确保在渲染完成前同步更新
  useLayoutEffect(() => {
    ref.current = value;
  });

  return ref;
}

/**
 * 返回一个稳定的函数引用，但内部始终调用最新的函数。
 * Returns a stable function reference that always calls the latest function internally.
 *
 * @example
 * ```tsx
 * const handleClick = useLatestCallback((id: string) => {
 *   setSelected(id); // 始终使用最新的 setSelected
 * });
 * // handleClick 引用永远不变，可以安全地传入依赖数组为空的 useEffect
 * ```
 */
export function useLatestCallback<T extends (...args: unknown[]) => unknown>(fn: T): T {
  const ref = useLatestRef(fn);

  return useCallback(
    ((...args: unknown[]) => {
      return ref.current(...args);
    }) as T,
    [ref],
  );
}
