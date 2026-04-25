/**
 * 流式打字动画 Hook — 逐字符显示流式内容
 * Typing animation hook — displays streaming content character by character
 *
 * Adapted from AionUi useTypingAnimation pattern.
 */

import { useEffect, useRef, useState } from 'react';

interface UseTypingAnimationOptions {
  /** 原始内容 */
  content: string;
  /** 是否启用动画 */
  enabled?: boolean;
  /** 打字速度（字符/秒） */
  speed?: number;
}

/**
 * @example
 * ```tsx
 * const { displayedContent, isAnimating } = useTypingAnimation({
 *   content: streamingText,
 *   enabled: true,
 *   speed: 50,
 * });
 * ```
 */
export function useTypingAnimation({
  content,
  enabled = true,
  speed = 50,
}: UseTypingAnimationOptions) {
  const [displayedContent, setDisplayedContent] = useState(content);
  const [isAnimating, setIsAnimating] = useState(false);
  const animationFrameRef = useRef<number | null>(null);
  const targetContentRef = useRef(content);

  useEffect(() => {
    if (!enabled) {
      setDisplayedContent(content);
      setIsAnimating(false);
      return;
    }

    targetContentRef.current = content;

    // 首次加载直接显示
    setDisplayedContent((prev) => {
      if (content === prev) {
        return prev;
      }

      if (prev.length === 0) {
        setIsAnimating(false);
        return content;
      }

      const contentDiff = content.length - prev.length;

      // 内容变短或一次性增加太多，直接显示
      if (contentDiff < 0 || contentDiff > 1000) {
        setIsAnimating(false);
        return content;
      }

      // 开始打字动画
      setIsAnimating(true);
      let currentIndex = prev.length;
      const targetContent = content;
      const msPerChar = 1000 / speed;

      const lastTime = { value: performance.now() };

      const animate = (now: number) => {
        const elapsed = now - lastTime.value;
        const charsToAdd = Math.floor(elapsed / msPerChar);

        if (charsToAdd > 0) {
          lastTime.value = now - (elapsed % msPerChar);
          currentIndex = Math.min(currentIndex + charsToAdd, targetContent.length);

          if (currentIndex >= targetContent.length) {
            setDisplayedContent(targetContent);
            setIsAnimating(false);
            animationFrameRef.current = null;
            return;
          }

          setDisplayedContent(targetContent.slice(0, currentIndex));
        }

        animationFrameRef.current = requestAnimationFrame(animate);
      };

      // 清理之前的动画
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      animationFrameRef.current = requestAnimationFrame(animate);

      return prev;
    });

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
    };
  }, [content, enabled, speed]);

  return {
    displayedContent,
    isAnimating,
  };
}
