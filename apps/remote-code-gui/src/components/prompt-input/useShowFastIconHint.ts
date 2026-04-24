import { useState, useEffect } from 'react';

export interface UseShowFastIconHintOptions {
  /** Duration in ms to show the hint */
  duration?: number;
}

export function useShowFastIconHint(options: UseShowFastIconHintOptions = {}) {
  const { duration = 3000 } = options;
  const [visible, setVisible] = useState(false);

  function show() {
    setVisible(true);
  }

  useEffect(() => {
    if (!visible) return;
    const timer = setTimeout(() => setVisible(false), duration);
    return () => clearTimeout(timer);
  }, [visible, duration]);

  return { visible, show };
}
