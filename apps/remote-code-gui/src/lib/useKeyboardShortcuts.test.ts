import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { useKeyboardShortcuts, getShortcutLabel } from './useKeyboardShortcuts';

function fireKeyDown(key: string, opts?: { ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean; altKey?: boolean }) {
  const event = new KeyboardEvent('keydown', {
    key,
    ctrlKey: opts?.ctrlKey ?? false,
    metaKey: opts?.metaKey ?? false,
    shiftKey: opts?.shiftKey ?? false,
    altKey: opts?.altKey ?? false,
    bubbles: true,
  });
  window.dispatchEvent(event);
  return event;
}

describe('useKeyboardShortcuts', () => {
  it('fires action when shortcut matches', () => {
    const action = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([
        { key: 'k', modifier: 'mod', action, description: 'test' },
      ]),
    );
    fireKeyDown('k', { ctrlKey: true });
    expect(action).toHaveBeenCalledTimes(1);
  });

  it('does not fire when modifier does not match', () => {
    const action = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([
        { key: 'k', modifier: 'mod', action, description: 'test' },
      ]),
    );
    fireKeyDown('k', { altKey: true });
    expect(action).not.toHaveBeenCalled();
  });

  it('respects enabled: false', () => {
    const action = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([
        { key: 'k', modifier: 'mod', action, description: 'test', enabled: false },
      ]),
    );
    fireKeyDown('k', { ctrlKey: true });
    expect(action).not.toHaveBeenCalled();
  });

  it('handles ctrl+shift modifier', () => {
    const action = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([
        { key: 'e', modifier: 'ctrl+shift', action, description: 'test' },
      ]),
    );
    fireKeyDown('e', { ctrlKey: true, shiftKey: true });
    expect(action).toHaveBeenCalledTimes(1);
  });
});

describe('getShortcutLabel', () => {
  it('returns Ctrl+K on non-Mac', () => {
    const label = getShortcutLabel('mod', 'k');
    expect(label).toMatch(/Ctrl/i);
  });

  it('returns Ctrl+Shift+E for ctrl+shift modifier', () => {
    const label = getShortcutLabel('ctrl+shift', 'e');
    expect(label).toBe('Ctrl+Shift+E');
  });
});
