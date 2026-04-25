/**
 * 文本选择 Hook — 监听容器内的文本选择事件，获取选中文本和位置
 * Text selection hook — monitors text selection within a container
 *
 * Adapted from AionUi useTextSelection pattern.
 */

import { useState, useEffect, useCallback } from 'react';

export interface SelectionPosition {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * @param containerRef 容器 DOM 引用
 * @param enabled 是否启用监听
 * @returns selectedText, selectionPosition, clearSelection
 */
export function useTextSelection(
  containerRef: React.RefObject<HTMLElement | null>,
  enabled = true,
) {
  const [selectedText, setSelectedText] = useState('');
  const [selectionPosition, setSelectionPosition] = useState<SelectionPosition | null>(null);

  const clearSelection = useCallback(() => {
    setSelectedText('');
    setSelectionPosition(null);
    window.getSelection()?.removeAllRanges();
  }, []);

  const handleSelectionChange = useCallback(() => {
    const selection = window.getSelection();
    const text = selection?.toString().trim() || '';

    if (!text) {
      setSelectedText('');
      setSelectionPosition(null);
      return;
    }

    if (containerRef.current && selection && selection.rangeCount > 0) {
      const range = selection.getRangeAt(0);
      const container = containerRef.current;

      if (!container.contains(range.commonAncestorContainer)) {
        setSelectedText('');
        setSelectionPosition(null);
        return;
      }

      setSelectedText(text);
    }
  }, [containerRef]);

  const handleMouseUp = useCallback(
    (e: MouseEvent) => {
      const selection = window.getSelection();
      const text = selection?.toString().trim() || '';

      if (!text || !containerRef.current || !selection || selection.rangeCount === 0) {
        return;
      }

      const range = selection.getRangeAt(0);
      if (!containerRef.current.contains(range.commonAncestorContainer)) {
        return;
      }

      setSelectionPosition({
        x: e.clientX,
        y: e.clientY,
        width: 0,
        height: 0,
      });
    },
    [containerRef],
  );

  useEffect(() => {
    if (!enabled) return;

    document.addEventListener('selectionchange', handleSelectionChange);
    document.addEventListener('mouseup', handleMouseUp as EventListener);

    return () => {
      document.removeEventListener('selectionchange', handleSelectionChange);
      document.removeEventListener('mouseup', handleMouseUp as EventListener);
    };
  }, [enabled, handleSelectionChange, handleMouseUp]);

  return {
    selectedText,
    selectionPosition,
    clearSelection,
  };
}
