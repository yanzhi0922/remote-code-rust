import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { MemoryUpdateNotification, getRelativeMemoryPath } from './MemoryUpdateNotification';

describe('MemoryUpdateNotification', () => {
  afterEach(() => { cleanup(); });

  it('renders notification with path', () => {
    const { getByTestId, getByText } = render(
      <MemoryUpdateNotification memoryPath="/home/user/.claude/CLAUDE.md" />,
    );
    expect(getByTestId('memory-update-notification')).toBeInTheDocument();
    expect(getByText(/Memory updated/)).toBeInTheDocument();
  });

  it('shows relative path when homeDir provided', () => {
    const { getByText } = render(
      <MemoryUpdateNotification
        memoryPath="/home/user/.claude/CLAUDE.md"
        homeDir="/home/user"
      />,
    );
    expect(getByText(/~\/\.claude\/CLAUDE\.md/)).toBeInTheDocument();
  });
});

describe('getRelativeMemoryPath', () => {
  it('returns path relative to home', () => {
    expect(getRelativeMemoryPath('/home/user/.claude/CLAUDE.md', '/home/user')).toBe(
      '~/.claude/CLAUDE.md',
    );
  });

  it('returns path relative to cwd', () => {
    expect(
      getRelativeMemoryPath('/project/CLAUDE.md', '/home/user', '/project'),
    ).toBe('./CLAUDE.md');
  });

  it('returns original path when no match', () => {
    expect(getRelativeMemoryPath('/some/random/path.md', '/home/user', '/project')).toBe(
      '/some/random/path.md',
    );
  });
});
