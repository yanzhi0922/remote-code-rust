import { useCallback, useEffect, useRef, useState } from 'react';

const STORAGE_KEY = 'rc-sidebar-width';
const DEFAULT_WIDTH = 308;
const MIN_WIDTH = 220;
const MAX_WIDTH = 400;

export function useResizableWidth() {
  const [width, setWidth] = useState(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const parsed = Number(stored);
        if (!Number.isNaN(parsed)) return Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, parsed));
      }
    } catch { /* ignore */ }
    return DEFAULT_WIDTH;
  });

  const isDragging = useRef(false);
  const startX = useRef(0);
  const startWidth = useRef(0);
  const latestWidthRef = useRef(width);
  latestWidthRef.current = width;

  useEffect(() => {
    document.documentElement.style.setProperty('--sidebar-width', `${width}px`);
  }, [width]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    isDragging.current = true;
    startX.current = e.clientX;
    startWidth.current = width;

    const handleMouseMove = (ev: MouseEvent) => {
      if (!isDragging.current) return;
      const delta = ev.clientX - startX.current;
      const next = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, startWidth.current + delta));
      document.documentElement.style.setProperty('--sidebar-width', `${next}px`);
      setWidth(next);
      latestWidthRef.current = next;
    };

    const handleMouseUp = () => {
      isDragging.current = false;
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      try { localStorage.setItem(STORAGE_KEY, String(latestWidthRef.current)); } catch { /* ignore */ }
    };

    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }, [width]);

  return { width, handleMouseDown };
}
