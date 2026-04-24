import {
  useRef,
  useEffect,
  useState,
  useCallback,
  useMemo,
  type ReactNode,
} from 'react';
import { ArrowDown, Hash } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface VirtualMessageListProps {
  children: ReactNode[];
  overscan?: number;
  className?: string;
  /** Search keyword to highlight in messages */
  searchQuery?: string;
  /** Called when user scrolls to top (load more history) */
  onLoadMore?: () => void;
  /** Whether more history is available */
  hasMore?: boolean;
  /** Loading state for history fetch */
  isLoadingMore?: boolean;
  /** Height of each item estimate (px), used for virtual scrolling */
  estimatedItemHeight?: number;
  /** Container height in px */
  containerHeight?: number;
}

interface HeightCache {
  heights: Map<number, number>;
  offsets: number[];
  totalHeight: number;
}

const DEFAULT_ESTIMATED_HEIGHT = 80;
const OVERSCAN = 3;
const SCROLL_THRESHOLD = 50;

function highlightText(text: string, query: string): ReactNode {
  if (!query.trim()) return text;
  const parts: ReactNode[] = [];
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  let lastIndex = 0;
  let matchIndex = lowerText.indexOf(lowerQuery);

  let key = 0;
  while (matchIndex !== -1) {
    if (matchIndex > lastIndex) {
      parts.push(<span key={key++}>{text.slice(lastIndex, matchIndex)}</span>);
    }
    parts.push(
      <mark key={key++} className="bg-yellow-200 text-yellow-900 rounded px-0.5">
        {text.slice(matchIndex, matchIndex + query.length)}
      </mark>,
    );
    lastIndex = matchIndex + query.length;
    matchIndex = lowerText.indexOf(lowerQuery, lastIndex);
  }
  if (lastIndex < text.length) {
    parts.push(<span key={key++}>{text.slice(lastIndex)}</span>);
  }
  return parts.length > 0 ? <>{parts}</> : text;
}

export function VirtualMessageList({
  children,
  overscan = OVERSCAN,
  className,
  searchQuery = '',
  onLoadMore,
  hasMore = false,
  isLoadingMore = false,
  estimatedItemHeight = DEFAULT_ESTIMATED_HEIGHT,
  containerHeight = 600,
}: VirtualMessageListProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const [scrollTop, setScrollTop] = useState(0);
  const [heightCache, setHeightCache] = useState<HeightCache>({
    heights: new Map(),
    offsets: [],
    totalHeight: 0,
  });
  const [autoScrollToBottom, setAutoScrollToBottom] = useState(true);
  const prevChildrenLengthRef = useRef(children.length);
  const jumpToIndexRef = useRef<number | null>(null);

  const itemCount = children.length;

  // Build offsets from cached heights
  const rebuildOffsets = useCallback(
    (heights: Map<number, number>): { offsets: number[]; totalHeight: number } => {
      const offsets: number[] = [];
      let total = 0;
      for (let i = 0; i < itemCount; i++) {
        offsets.push(total);
        total += heights.get(i) ?? estimatedItemHeight;
      }
      return { offsets, totalHeight: total };
    },
    [itemCount, estimatedItemHeight],
  );

  // Measure actual heights using ResizeObserver
  useEffect(() => {
    const observer = new ResizeObserver((entries) => {
      setHeightCache((prev) => {
        const newHeights = new Map(prev.heights);
        let changed = false;
        for (const entry of entries) {
          const el = entry.target as HTMLDivElement;
          const idx = Number(el.dataset.index);
          if (!isNaN(idx)) {
            const newHeight = entry.contentRect.height;
            if (newHeights.get(idx) !== newHeight) {
              newHeights.set(idx, newHeight);
              changed = true;
            }
          }
        }
        if (!changed) return prev;
        const { offsets, totalHeight } = rebuildOffsets(newHeights);
        return { heights: newHeights, offsets, totalHeight };
      });
    });

    itemRefs.current.forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, [rebuildOffsets]);

  // Rebuild offsets when item count changes
  useEffect(() => {
    setHeightCache((prev) => {
      const { offsets, totalHeight } = rebuildOffsets(prev.heights);
      return { ...prev, offsets, totalHeight };
    });
  }, [itemCount, rebuildOffsets]);

  // Auto-scroll to bottom when new children are added
  useEffect(() => {
    if (autoScrollToBottom && itemCount > prevChildrenLengthRef.current) {
      const el = containerRef.current;
      if (el) {
        el.scrollTop = el.scrollHeight;
      }
    }
    prevChildrenLengthRef.current = itemCount;
  }, [itemCount, autoScrollToBottom]);

  // Handle jump to specific index
  useEffect(() => {
    if (jumpToIndexRef.current !== null) {
      const el = containerRef.current;
      if (el && heightCache.offsets[jumpToIndexRef.current] !== undefined) {
        el.scrollTop = heightCache.offsets[jumpToIndexRef.current];
      }
      jumpToIndexRef.current = null;
    }
  }, [heightCache.offsets]);

  // Detect scroll position for auto-scroll and load more
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;

    setScrollTop(el.scrollTop);

    // Detect if scrolled to bottom
    const isAtBottom =
      el.scrollHeight - el.scrollTop - el.clientHeight < SCROLL_THRESHOLD;
    setAutoScrollToBottom(isAtBottom);

    // Detect if scrolled to top for load more
    if (el.scrollTop < SCROLL_THRESHOLD && hasMore && !isLoadingMore) {
      onLoadMore?.();
    }
  }, [hasMore, isLoadingMore, onLoadMore]);

  // Calculate visible range
  const visibleRange = useMemo(() => {
    const startOffset = scrollTop;
    const endOffset = scrollTop + containerHeight;

    let start = 0;
    let end = itemCount - 1;

    // Find start index
    for (let i = 0; i < heightCache.offsets.length; i++) {
      const itemBottom =
        heightCache.offsets[i] + (heightCache.heights.get(i) ?? estimatedItemHeight);
      if (itemBottom > startOffset) {
        start = Math.max(0, i - overscan);
        break;
      }
    }

    // Find end index
    for (let i = start; i < heightCache.offsets.length; i++) {
      if (heightCache.offsets[i] > endOffset) {
        end = Math.min(itemCount - 1, i + overscan);
        break;
      }
    }

    return { start, end };
  }, [scrollTop, containerHeight, heightCache, itemCount, overscan, estimatedItemHeight]);

  // Scroll to bottom button
  const showScrollToBottom = !autoScrollToBottom && itemCount > 0;

  const scrollToBottom = useCallback(() => {
    const el = containerRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
      setAutoScrollToBottom(true);
    }
  }, []);

  // Jump to message index
  const jumpToMessage = useCallback((index: number) => {
    if (index >= 0 && index < itemCount) {
      jumpToIndexRef.current = index;
    }
  }, [itemCount]);

  // Expose jumpToMessage via ref pattern (for external use)
  // We use the container ref to attach the method
  useEffect(() => {
    if (containerRef.current) {
      (containerRef.current as HTMLDivElement & { jumpToMessage: (i: number) => void }).jumpToMessage = jumpToMessage;
    }
  }, [jumpToMessage]);

  // Top spacer and bottom spacer heights
  const topSpacer = heightCache.offsets[visibleRange.start] ?? 0;
  const bottomSpacer = Math.max(
    0,
    heightCache.totalHeight -
      (heightCache.offsets[visibleRange.end] ?? 0) -
      (heightCache.heights.get(visibleRange.end) ?? estimatedItemHeight),
  );

  // Scroll indicator position (0-1)
  const scrollPosition = heightCache.totalHeight > 0 ? scrollTop / heightCache.totalHeight : 0;

  return (
    <div className="relative flex-1" data-testid="virtual-message-list-wrapper">
      {/* Scroll indicator */}
      <div
        className="absolute right-0 top-0 w-1 bg-slate-100"
        style={{ height: containerHeight }}
        data-testid="scroll-indicator-track"
      >
        <div
          className="w-full rounded-full bg-blue-300 transition-all duration-150"
          style={{
            height: `${Math.max(20, (containerHeight / Math.max(1, heightCache.totalHeight)) * containerHeight)}px`,
            transform: `translateY(${scrollPosition * containerHeight}px)`,
          }}
          data-testid="scroll-indicator-thumb"
        />
      </div>

      {/* Message list container */}
      <div
        ref={containerRef}
        data-testid="virtual-message-list"
        className={cn('overflow-y-auto', className)}
        style={{ height: containerHeight }}
        onScroll={handleScroll}
      >
        {/* Load more trigger */}
        {hasMore && (
          <div className="flex justify-center py-2" data-testid="load-more-trigger">
            {isLoadingMore ? (
              <div className="flex items-center gap-2 text-xs text-slate-400">
                <div className="h-3 w-3 animate-spin rounded-full border-2 border-slate-300 border-t-blue-500" />
                加载更多消息...
              </div>
            ) : (
              <button
                type="button"
                className="text-xs text-blue-500 hover:text-blue-600"
                onClick={onLoadMore}
              >
                加载更多历史消息
              </button>
            )}
          </div>
        )}

        {/* Top spacer */}
        <div style={{ height: topSpacer }} data-testid="virtual-spacer-top" />

        {/* Visible items */}
        {children.slice(visibleRange.start, visibleRange.end + 1).map((child, relativeIndex) => {
          const index = visibleRange.start + relativeIndex;
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
              data-index={index}
              data-testid={`virtual-message-${index}`}
              className="relative"
            >
              {/* Message index badge */}
              <div
                className="absolute -left-6 top-1 text-[10px] text-slate-300 font-mono"
                data-testid={`message-index-${index}`}
              >
                <Hash className="h-3 w-3" />
              </div>
              {/* Search highlight wrapper */}
              {searchQuery ? (
                <div data-testid={`message-highlight-${index}`}>
                  {child}
                </div>
              ) : (
                child
              )}
            </div>
          );
        })}

        {/* Bottom spacer */}
        <div style={{ height: bottomSpacer }} data-testid="virtual-spacer-bottom" />
      </div>

      {/* Scroll to bottom button */}
      {showScrollToBottom && (
        <button
          type="button"
          className="absolute bottom-4 right-6 flex h-8 w-8 items-center justify-center rounded-full bg-blue-500 text-white shadow-lg transition-all hover:bg-blue-600"
          onClick={scrollToBottom}
          data-testid="scroll-to-bottom"
          title="滚动到底部"
        >
          <ArrowDown className="h-4 w-4" />
        </button>
      )}
    </div>
  );
}

export { highlightText };
