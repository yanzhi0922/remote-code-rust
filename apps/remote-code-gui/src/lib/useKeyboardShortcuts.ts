import { useCallback, useEffect } from 'react';

type KeyModifier = 'mod' | 'ctrl' | 'alt' | 'shift' | 'mod+shift' | 'ctrl+shift' | 'alt+shift';

export interface KeyboardShortcut {
  key: string;
  modifier?: KeyModifier;
  action: () => void;
  description: string;
  enabled?: boolean;
}

const isMac = typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.userAgent);

function matchShortcut(event: KeyboardEvent, shortcut: KeyboardShortcut): boolean {
  if (shortcut.enabled === false) return false;

  const key = shortcut.key.toLowerCase();
  if (event.key.toLowerCase() !== key) return false;

  const mod = shortcut.modifier ?? 'mod';

  switch (mod) {
    case 'mod':
      return isMac ? event.metaKey : event.ctrlKey;
    case 'ctrl':
      return event.ctrlKey;
    case 'alt':
      return event.altKey;
    case 'shift':
      return event.shiftKey;
    case 'mod+shift':
      return (isMac ? event.metaKey : event.ctrlKey) && event.shiftKey;
    case 'ctrl+shift':
      return event.ctrlKey && event.shiftKey;
    case 'alt+shift':
      return event.altKey && event.shiftKey;
    default:
      return false;
  }
}

export function useKeyboardShortcuts(shortcuts: KeyboardShortcut[]) {
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      for (const shortcut of shortcuts) {
        if (matchShortcut(event, shortcut)) {
          event.preventDefault();
          shortcut.action();
          return;
        }
      }
    },
    [shortcuts],
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);
}

export function getShortcutLabel(modifier: KeyModifier | undefined, key: string): string {
  const mod = modifier ?? 'mod';
  const keyLabel = key.length === 1 ? key.toUpperCase() : key;

  switch (mod) {
    case 'mod':
      return isMac ? `⌘${keyLabel}` : `Ctrl+${keyLabel}`;
    case 'ctrl':
      return `Ctrl+${keyLabel}`;
    case 'alt':
      return isMac ? `⌥${keyLabel}` : `Alt+${keyLabel}`;
    case 'shift':
      return `Shift+${keyLabel}`;
    case 'mod+shift':
      return isMac ? `⌘⇧${keyLabel}` : `Ctrl+Shift+${keyLabel}`;
    case 'ctrl+shift':
      return `Ctrl+Shift+${keyLabel}`;
    case 'alt+shift':
      return isMac ? `⌥⇧${keyLabel}` : `Alt+Shift+${keyLabel}`;
    default:
      return keyLabel;
  }
}
