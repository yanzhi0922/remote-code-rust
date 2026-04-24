import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { MemoryFileSelector } from './MemoryFileSelector';
import { MemoryUpdateNotification, getRelativeMemoryPath } from './MemoryUpdateNotification';

afterEach(() => {
  cleanup();
});

const mockFiles = [
  { path: '/home/user/.claude/CLAUDE.md', type: 'User' as const, exists: true, description: 'Saved in ~/.claude/CLAUDE.md' },
  { path: '/home/user/project/CLAUDE.md', type: 'Project' as const, exists: true, description: 'Checked in at ./CLAUDE.md' },
  { path: '/home/user/project/.claude/extra.md', type: 'Nested' as const, exists: false, description: '@-imported' },
];

describe('MemoryFileSelector', () => {
  it('renders memory files', () => {
    render(<MemoryFileSelector files={mockFiles} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('memory-file-selector')).toBeInTheDocument();
    expect(screen.getByText('User memory')).toBeInTheDocument();
    expect(screen.getByText('Project memory')).toBeInTheDocument();
  });

  it('shows empty state when no files', () => {
    render(<MemoryFileSelector files={[]} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText(/No memory files found/)).toBeInTheDocument();
  });

  it('calls onSelect when file clicked', () => {
    const onSelect = vi.fn();
    render(<MemoryFileSelector files={mockFiles} onSelect={onSelect} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByText('User memory'));
    expect(onSelect).toHaveBeenCalledWith('/home/user/.claude/CLAUDE.md');
  });

  it('calls onCancel when cancel clicked', () => {
    const onCancel = vi.fn();
    render(<MemoryFileSelector files={mockFiles} onSelect={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByTestId('memory-selector-cancel'));
    expect(onCancel).toHaveBeenCalled();
  });

  it('shows (new) for non-existent files', () => {
    render(<MemoryFileSelector files={mockFiles} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText('(new)')).toBeInTheDocument();
  });

  it('shows descriptions', () => {
    render(<MemoryFileSelector files={mockFiles} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText('Saved in ~/.claude/CLAUDE.md')).toBeInTheDocument();
  });
});

describe('MemoryUpdateNotification', () => {
  it('renders notification with relative path', () => {
    render(
      <MemoryUpdateNotification
        memoryPath="/home/user/.claude/CLAUDE.md"
        homeDir="/home/user"
        cwd="/home/user/project"
      />,
    );
    expect(screen.getByTestId('memory-update-notification')).toBeInTheDocument();
    expect(screen.getByText(/Memory updated in/)).toBeInTheDocument();
  });

  it('uses tilde for home-relative paths', () => {
    render(
      <MemoryUpdateNotification
        memoryPath="/home/user/.claude/CLAUDE.md"
        homeDir="/home/user"
        cwd="/home/user/project"
      />,
    );
    expect(screen.getByText(/~\/\.claude\/CLAUDE\.md/)).toBeInTheDocument();
  });

  it('uses relative for cwd-relative paths', () => {
    render(
      <MemoryUpdateNotification
        memoryPath="/home/user/project/CLAUDE.md"
        homeDir="/home/user"
        cwd="/home/user/project"
      />,
    );
    expect(screen.getByText(/\.\/CLAUDE\.md/)).toBeInTheDocument();
  });

  it('shows absolute path when no relative match', () => {
    render(
      <MemoryUpdateNotification
        memoryPath="/some/absolute/path"
        homeDir="/home/user"
        cwd="/home/user/project"
      />,
    );
    expect(screen.getByText(/\/some\/absolute\/path/)).toBeInTheDocument();
  });
});

describe('getRelativeMemoryPath', () => {
  it('returns tilde path for home-relative', () => {
    expect(getRelativeMemoryPath('/home/user/.claude/CLAUDE.md', '/home/user', '/home/user/project'))
      .toBe('~/.claude/CLAUDE.md');
  });

  it('returns relative path for cwd-relative', () => {
    expect(getRelativeMemoryPath('/home/user/project/CLAUDE.md', '/home/user', '/home/user/project'))
      .toBe('./CLAUDE.md');
  });

  it('returns absolute when no match', () => {
    expect(getRelativeMemoryPath('/some/path', '/home/user', '/home/user/project'))
      .toBe('/some/path');
  });
});
