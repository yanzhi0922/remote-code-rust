import { useRef, useCallback, useEffect } from 'react';

export interface SwipeGestureOptions {
  onSwipeLeft?: () => void;
  onSwipeRight?: () => void;
  onSwipeUp?: () => void;
  onSwipeDown?: () => void;
  threshold?: number;
  preventScroll?: boolean;
}

export function useSwipeGesture(elementRef: React.RefObject<HTMLElement | null>, options: SwipeGestureOptions) {
  const {
    onSwipeLeft, onSwipeRight, onSwipeUp, onSwipeDown,
    threshold = 50, preventScroll = false,
  } = options;

  const startX = useRef(0);
  const startY = useRef(0);
  const startTime = useRef(0);
  const isSwiping = useRef(false);

  const handleTouchStart = useCallback((e: TouchEvent) => {
    startX.current = e.touches[0].clientX;
    startY.current = e.touches[0].clientY;
    startTime.current = Date.now();
    isSwiping.current = true;
  }, []);

  const handleTouchMove = useCallback((e: TouchEvent) => {
    if (!isSwiping.current) return;
    const deltaX = e.touches[0].clientX - startX.current;
    const deltaY = e.touches[0].clientY - startY.current;
    if (preventScroll && Math.abs(deltaX) > 10 && Math.abs(deltaX) > Math.abs(deltaY)) {
      e.preventDefault();
    }
  }, [preventScroll]);

  const handleTouchEnd = useCallback((e: TouchEvent) => {
    if (!isSwiping.current) return;
    isSwiping.current = false;

    const endX = e.changedTouches[0].clientX;
    const endY = e.changedTouches[0].clientY;
    const deltaX = endX - startX.current;
    const deltaY = endY - startY.current;
    const deltaTime = Date.now() - startTime.current;

    if (deltaTime > 500) return;

    const absX = Math.abs(deltaX);
    const absY = Math.abs(deltaY);

    if (absX > threshold && absX > absY) {
      if (deltaX < 0) onSwipeLeft?.();
      else if (deltaX > 0) onSwipeRight?.();
    } else if (absY > threshold && absY > absX) {
      if (deltaY < 0) onSwipeUp?.();
      else if (deltaY > 0) onSwipeDown?.();
    }
  }, [threshold, onSwipeLeft, onSwipeRight, onSwipeUp, onSwipeDown]);

  useEffect(() => {
    const el = elementRef.current;
    if (!el) return;
    el.addEventListener('touchstart', handleTouchStart, { passive: false });
    el.addEventListener('touchmove', handleTouchMove, { passive: false });
    el.addEventListener('touchend', handleTouchEnd);
    return () => {
      el.removeEventListener('touchstart', handleTouchStart);
      el.removeEventListener('touchmove', handleTouchMove);
      el.removeEventListener('touchend', handleTouchEnd);
    };
  }, [elementRef, handleTouchStart, handleTouchMove, handleTouchEnd]);

  return { isSwiping };
}

export interface DoubleTapOptions {
  onDoubleTap: (e: TouchEvent) => void;
  delay?: number;
}

export function useDoubleTap(elementRef: React.RefObject<HTMLElement | null>, options: DoubleTapOptions) {
  const { onDoubleTap, delay = 300 } = options;
  const lastTap = useRef(0);
  const tapTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleTouchEnd = useCallback((e: TouchEvent) => {
    const now = Date.now();
    const elapsed = now - lastTap.current;

    if (elapsed < delay && elapsed > 0) {
      if (tapTimeout.current) { clearTimeout(tapTimeout.current); tapTimeout.current = null; }
      onDoubleTap(e);
      lastTap.current = 0;
    } else {
      lastTap.current = now;
      tapTimeout.current = setTimeout(() => { lastTap.current = 0; tapTimeout.current = null; }, delay);
    }
  }, [onDoubleTap, delay]);

  useEffect(() => {
    const el = elementRef.current;
    if (!el) return;
    el.addEventListener('touchend', handleTouchEnd);
    return () => {
      el.removeEventListener('touchend', handleTouchEnd);
      if (tapTimeout.current) clearTimeout(tapTimeout.current);
    };
  }, [elementRef, handleTouchEnd]);
}

export interface LongPressOptions {
  onLongPress: () => void;
  delay?: number;
}

export function useLongPress(elementRef: React.RefObject<HTMLElement | null>, options: LongPressOptions) {
  const { onLongPress, delay = 500 } = options;
  const pressTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clear = () => {
    if (pressTimeout.current) { clearTimeout(pressTimeout.current); pressTimeout.current = null; }
  };

  const handleTouchStart = useCallback(() => {
    pressTimeout.current = setTimeout(() => { onLongPress(); pressTimeout.current = null; }, delay);
  }, [onLongPress, delay]);

  const handleEnd = useCallback(() => clear(), []);

  useEffect(() => {
    const el = elementRef.current;
    if (!el) return;
    el.addEventListener('touchstart', handleTouchStart, { passive: true });
    el.addEventListener('touchend', handleEnd);
    el.addEventListener('touchmove', handleEnd);
    el.addEventListener('touchcancel', handleEnd);
    return () => {
      el.removeEventListener('touchstart', handleTouchStart);
      el.removeEventListener('touchend', handleEnd);
      el.removeEventListener('touchmove', handleEnd);
      el.removeEventListener('touchcancel', handleEnd);
      clear();
    };
  }, [elementRef, handleTouchStart, handleEnd]);
}

export interface PinchZoomOptions {
  onPinch?: (scale: number) => void;
  minScale?: number;
  maxScale?: number;
}

export function usePinchZoom(elementRef: React.RefObject<HTMLElement | null>, options: PinchZoomOptions) {
  const { onPinch, minScale = 0.5, maxScale = 3 } = options;
  const initialDistance = useRef(0);
  const initialScale = useRef(1);
  const currentScale = useRef(1);
  const isPinching = useRef(false);

  const getDistance = (touches: TouchList) => {
    const dx = touches[0].clientX - touches[1].clientX;
    const dy = touches[0].clientY - touches[1].clientY;
    return Math.sqrt(dx * dx + dy * dy);
  };

  const handleTouchStart = useCallback((e: TouchEvent) => {
    if (e.touches.length === 2) {
      initialDistance.current = getDistance(e.touches);
      initialScale.current = currentScale.current;
      isPinching.current = true;
    }
  }, []);

  const handleTouchMove = useCallback((e: TouchEvent) => {
    if (!isPinching.current || e.touches.length !== 2 || !onPinch) return;
    const distance = getDistance(e.touches);
    let scale = (distance / initialDistance.current) * initialScale.current;
    scale = Math.max(minScale, Math.min(maxScale, scale));
    currentScale.current = scale;
    onPinch(scale);
  }, [onPinch, minScale, maxScale]);

  const handleTouchEnd = useCallback(() => { isPinching.current = false; }, []);

  useEffect(() => {
    const el = elementRef.current;
    if (!el) return;
    el.addEventListener('touchstart', handleTouchStart, { passive: true });
    el.addEventListener('touchmove', handleTouchMove, { passive: false });
    el.addEventListener('touchend', handleTouchEnd);
    return () => {
      el.removeEventListener('touchstart', handleTouchStart);
      el.removeEventListener('touchmove', handleTouchMove);
      el.removeEventListener('touchend', handleTouchEnd);
    };
  }, [elementRef, handleTouchStart, handleTouchMove, handleTouchEnd]);

  return { scale: currentScale };
}