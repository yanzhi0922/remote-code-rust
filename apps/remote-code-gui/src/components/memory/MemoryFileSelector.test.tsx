import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { MemoryFileSelector, type MemoryFileInfo } from './MemoryFileSelector';

describe('MemoryFileSelector', () => {
  afterEach(() => { cleanup(); });

  it('renders empty state when no files', () => {
    const { getByTestId, getByText } = render(
      <MemoryFileSelector files={[]} onSelect={() => {}} onCancel={() => {}} />,
    );
    expect(getByTestId('memory-file-selector')).toBeInTheDocument();
    expect(getByText(/No memory files found/)).toBeInTheDocument();
  });

  it('renders file list with type labels', () => {
    const files: MemoryFileInfo[] = [
      { path: 'CLAUDE.md', type: 'Project', exists: true, description: 'Project config' },
      { path: 'global.md', type: 'User', exists: true },
    ];
    const { getByTestId, getByText } = render(
      <MemoryFileSelector files={files} onSelect={() => {}} onCancel={() => {}} />,
    );
    // Verify files are rendered via their data-testid
    expect(getByTestId('memory-file-CLAUDE-md')).toBeInTheDocument();
    expect(getByTestId('memory-file-global-md')).toBeInTheDocument();
    // Type labels shown
    expect(getByText('User memory')).toBeInTheDocument();
    // Description shown
    expect(getByText('Project config')).toBeInTheDocument();
  });

  it('calls onSelect with file path when file clicked', () => {
    const onSelect = vi.fn();
    const files: MemoryFileInfo[] = [
      { path: 'test.md', type: 'Project', exists: true },
    ];
    const { getByTestId } = render(
      <MemoryFileSelector files={files} onSelect={onSelect} onCancel={() => {}} />,
    );
    fireEvent.click(getByTestId('memory-file-test-md'));
    expect(onSelect).toHaveBeenCalledWith('test.md');
  });

  it('calls onCancel when cancel clicked', () => {
    const onCancel = vi.fn();
    const { getByTestId } = render(
      <MemoryFileSelector files={[]} onSelect={() => {}} onCancel={onCancel} />,
    );
    fireEvent.click(getByTestId('memory-selector-cancel'));
    expect(onCancel).toHaveBeenCalled();
  });
});
