import { useCallback, useRef, useState, type ReactNode } from 'react';

export type SplitDirection = 'horizontal' | 'vertical';

interface SplitPaneProps {
  direction?: SplitDirection;
  defaultSize?: number; // percentage 0-100
  minSize?: number; // percentage 0-100
  maxSize?: number; // percentage 0-100
  first: ReactNode;
  second: ReactNode;
  className?: string;
}

export function SplitPane({
  direction = 'vertical',
  defaultSize = 60,
  minSize = 20,
  maxSize = 80,
  first,
  second,
  className = '',
}: SplitPaneProps) {
  const [size, setSize] = useState(defaultSize);
  const containerRef = useRef<HTMLDivElement>(null);
  const isDragging = useRef(false);

  const handleMouseDown = useCallback(() => {
    isDragging.current = true;
    const handleMouseUp = () => {
      isDragging.current = false;
      document.removeEventListener('mouseup', handleMouseUp);
      document.removeEventListener('mousemove', handleMouseMove);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
    const handleMouseMove = (e: MouseEvent) => {
      if (!isDragging.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      let newPercent: number;
      if (direction === 'horizontal') {
        newPercent = ((e.clientX - rect.left) / rect.width) * 100;
      } else {
        newPercent = ((e.clientY - rect.top) / rect.height) * 100;
      }
      newPercent = Math.max(minSize, Math.min(maxSize, newPercent));
      setSize(newPercent);
    };
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    document.body.style.cursor = direction === 'horizontal' ? 'col-resize' : 'row-resize';
    document.body.style.userSelect = 'none';
  }, [direction, minSize, maxSize]);

  const isHorizontal = direction === 'horizontal';

  return (
    <div
      ref={containerRef}
      className={`flex ${isHorizontal ? 'flex-row' : 'flex-col'} h-full w-full ${className}`}
    >
      <div
        style={{ [isHorizontal ? 'width' : 'height']: `${size}%` }}
        className="min-h-0 min-w-0 overflow-hidden"
      >
        {first}
      </div>
      <div
        onMouseDown={handleMouseDown}
        className={`group relative shrink-0 ${
          isHorizontal
            ? 'w-1 cursor-col-resize hover:w-1.5'
            : 'h-1 cursor-row-resize hover:h-1.5'
        } bg-rc-border-secondary hover:bg-rc-accent-primary transition-colors`}
      >
        <div
          className={`absolute ${
            isHorizontal
              ? 'inset-y-0 -left-1 -right-1'
              : 'inset-x-0 -top-1 -bottom-1'
          }`}
        />
      </div>
      <div
        style={{ [isHorizontal ? 'width' : 'height']: `${100 - size}%` }}
        className="min-h-0 min-w-0 flex-1 overflow-hidden"
      >
        {second}
      </div>
    </div>
  );
}
