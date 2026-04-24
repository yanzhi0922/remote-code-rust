import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { KeybindingWarnings } from './KeybindingWarnings';

afterEach(() => {
  cleanup();
});

describe('KeybindingWarnings', () => {
  it('renders nothing when no conflicts', () => {
    const { container } = render(<KeybindingWarnings conflicts={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders conflicts', () => {
    const conflicts = [
      { action: '保存', shortcut: 'ctrl+s', conflictingAction: '搜索' },
    ];
    render(<KeybindingWarnings conflicts={conflicts} />);
    expect(screen.getByTestId('keybinding-warnings')).toBeInTheDocument();
    expect(screen.getByText('保存')).toBeInTheDocument();
    expect(screen.getByText('搜索')).toBeInTheDocument();
  });
});
