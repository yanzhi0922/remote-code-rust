/**
 * 智能自动滚动 Hook — 当内容更新时，如果用户处于底部附近则自动滚动
 * Smart auto-scroll hook — auto-scrolls to bottom when user is near bottom on content update
 *
 * Adapted from AionUi useAutoScroll pattern.
 */

import { useEffect } from 'react';

interface UseAutoScrollOptions {
  containerRef: React.RefObject<HTMLDivElement | null>;
  /** 内容字符串，变化时触发滚动检查 */
  content: string;
  /** 是否启用自动滚动 */
  enabled?: boolean;
  /** 距离底部多少像素以内触发自动滚动 */
  threshold?: number;
  /** 滚动行为 */
  behavior?: ScrollBehavior;
}

/**
 * @example
 * ```tsx
 * const containerRef = useRef<HTMLDivElement>(null);
 * useAutoScroll({ containerRef, content: streamingText, enabled: true, threshold: 200 });
 * ```
 */
export function useAutoScroll({
  containerRef,
  content,
  enabled = true,
  threshold = 200,
  behavior = 'smooth',
}: UseAutoScrollOptions) {
  useEffect(() => {
    if (!enabled) return;

    const container = containerRef.current;
    if (!container) return;

    const { scrollTop, scrollHeight, clientHeight } = container;
    const distanceToBottom = scrollHeight - scrollTop - clientHeight;

    if (distanceToBottom < threshold) {
      container.scrollTo({ top: scrollHeight, behavior });
    }
  }, [content, enabled, threshold, behavior, containerRef]);
}
