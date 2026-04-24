import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Commands } from './Commands';

afterEach(() => {
  cleanup();
});

describe('Commands', () => {
  it('shows empty state when no commands', () => {
    render(<Commands commands={[]} />);
    expect(screen.getByTestId('commands-empty')).toHaveTextContent('没有可用命令');
  });

  it('renders command items', () => {
    const commands = [
      { name: '/help', description: '帮助', category: '通用' },
      { name: '/clear', description: '清空', category: '通用' },
    ];
    render(<Commands commands={commands} />);
    expect(screen.getByTestId('command-item-help')).toBeInTheDocument();
    expect(screen.getByTestId('command-item-clear')).toBeInTheDocument();
  });
});
