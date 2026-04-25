/**
 * 输入法合成事件处理 Hook — 正确处理 CJK 输入法的 composition 事件
 * IME composition event handler — correctly handles CJK input method composition
 *
 * Adapted from AionUi useCompositionInput pattern.
 */

import { useRef, useState } from 'react';

/**
 * 处理输入法（IME）的合成事件，避免在中文/日文/韩文输入过程中误触发提交。
 * Handles IME composition events to prevent accidental submission during CJK input.
 */
export function useCompositionInput() {
  const isComposing = useRef(false);
  const [isComposingState, setIsComposingState] = useState(false);

  const compositionHandlers = {
    onCompositionStartCapture: () => {
      isComposing.current = true;
      setIsComposingState(true);
    },
    onCompositionEndCapture: () => {
      isComposing.current = false;
      setIsComposingState(false);
    },
  };

  const createKeyDownHandler = (
    onEnterPress: () => void,
    onKeyDownIntercept?: (e: React.KeyboardEvent) => boolean,
  ) => {
    return (e: React.KeyboardEvent) => {
      if (isComposing.current) return;
      if (onKeyDownIntercept?.(e)) return;
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        onEnterPress();
      }
    };
  };

  return {
    isComposing,
    isComposingState,
    compositionHandlers,
    createKeyDownHandler,
  };
}
