import { memo, useRef, useState, useEffect, useCallback } from 'react';
import type { ConversationEntry } from '../../lib/types';
import { cn } from '../../lib/utils';

/** 虚拟消息列表组件属性 */
export interface VirtualMessageListProps {
  /** 消息列表 */
  messages: ConversationEntry[];
  /** 视窗外额外渲染的消息数 */
  overscan?: number;
  /** 额外的 CSS 类名 */
  className?: string;
  /** 渲染子项的函数 */
  children: (entry: ConversationEntry, index: number) => React.ReactNode;
}

/** 每个消息项的估计高度 */
const ESTIMATED_ITEM_HEIGHT = 80;

/**
 * 虚拟消息列表渲染组件。
 * 使用 IntersectionObserver 实现简单的虚拟滚动，
 * 只渲染视窗内的消息 + overscan 缓冲。
 */
export const VirtualMessageList = memo(function VirtualMessageList({
  messages,
  overscan = 5,
  className,
  children,
}: VirtualMessageListProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [visibleRange, setVisibleRange] = useState<[number, number]>([0, Math.min(20, messages.length)]);
  const itemRefs = useRef<Map<number, HTMLDivElement>>(new Map());

  const updateVisibleRange = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    const containerRect = container.getBoundingClientRect();
    const top = containerRect.top;
    const bottom = containerRect.bottom;

    let firstVisible = -1;
    let lastVisible = -1;

    itemRefs.current.forEach((el, index) => {
      const rect = el.getBoundingClientRect();
      if (rect.bottom >= top && rect.top <= bottom) {
        if (firstVisible === -1 || index < firstVisible) firstVisible = index;
        if (index > lastVisible) lastVisible = index;
      }
    });

    if (firstVisible === -1) {
      const estimatedStart = Math.floor(container.scrollTop / ESTIMATED_ITEM_HEIGHT);
      const estimatedEnd = Math.min(
        estimatedStart + Math.ceil(container.clientHeight / ESTIMATED_ITEM_HEIGHT),
        messages.length - 1,
      );
      firstVisible = Math.max(0, estimatedStart);
      lastVisible = Math.max(firstVisible, estimatedEnd);
    }

    setVisibleRange([
      Math.max(0, firstVisible - overscan),
      Math.min(messages.length - 1, lastVisible + overscan),
    ]);
  }, [messages.length, overscan]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new IntersectionObserver(
      () => {
        updateVisibleRange();
      },
      { root: container, threshold: 0 },
    );

    itemRefs.current.forEach((el) => observer.observe(el));

    return () => observer.disconnect();
  }, [updateVisibleRange, messages]);

  useEffect(() => {
    setVisibleRange([0, Math.min(20 + overscan, messages.length)]);
  }, [messages.length, overscan]);

  const [start, end] = visibleRange;

  return (
    <div
      data-testid="virtual-message-list"
      ref={containerRef}
      onScroll={updateVisibleRange}
      className={cn('overflow-y-auto', className)}
      style={{ position: 'relative' }}
    >
      {messages.map((entry, index) => {
        const isVisible = index >= start && index <= end;
        return (
          <div
            key={index}
            ref={(el) => {
              if (el) {
                itemRefs.current.set(index, el);
              } else {
                itemRefs.current.delete(index);
              }
            }}
            style={{ minHeight: ESTIMATED_ITEM_HEIGHT }}
          >
            {isVisible ? children(entry, index) : null}
          </div>
        );
      })}
    </div>
  );
});
