import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FilePermissionOptions } from './FilePermissionOptions';

describe('FilePermissionOptions', () => {
  afterEach(cleanup);

  it('displays the file path', () => {
    render(
      <FilePermissionOptions
        filePath="/src/index.ts"
        currentBehavior="ask"
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByText('/src/index.ts')).toBeInTheDocument();
  });

  it('shows three behavior buttons', () => {
    render(
      <FilePermissionOptions
        filePath="/src/index.ts"
        currentBehavior="ask"
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByText('允许')).toBeInTheDocument();
    expect(screen.getByText('拒绝')).toBeInTheDocument();
    expect(screen.getByText('每次询问')).toBeInTheDocument();
  });

  it('calls onSelect when behavior is clicked', () => {
    const onSelect = vi.fn();
    render(
      <FilePermissionOptions
        filePath="/src/index.ts"
        currentBehavior="ask"
        onSelect={onSelect}
      />,
    );
    fireEvent.click(screen.getByText('允许'));
    expect(onSelect).toHaveBeenCalledWith('allow', 'session');
  });

  it('shows scope options', () => {
    render(
      <FilePermissionOptions
        filePath="/src/index.ts"
        currentBehavior="ask"
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByText('本次会话')).toBeInTheDocument();
    expect(screen.getByText('项目级')).toBeInTheDocument();
    expect(screen.getByText('用户级')).toBeInTheDocument();
  });

  it('calls onSelect when scope is changed', () => {
    const onSelect = vi.fn();
    render(
      <FilePermissionOptions
        filePath="/src/index.ts"
        currentBehavior="ask"
        onSelect={onSelect}
      />,
    );
    fireEvent.click(screen.getByText('项目级'));
    expect(onSelect).toHaveBeenCalledWith('ask', 'project');
  });

  it('shows preview rule text', () => {
    render(
      <FilePermissionOptions
        filePath="/src/index.ts"
        currentBehavior="ask"
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByText(/预览规则:/)).toBeInTheDocument();
  });

  it('defaults to current behavior', () => {
    render(
      <FilePermissionOptions
        filePath="/src/index.ts"
        currentBehavior="deny"
        onSelect={vi.fn()}
      />,
    );
    // The deny button should be active (has red styling)
    const denyButton = screen.getByText('拒绝').closest('button')!;
    expect(denyButton.className).toContain('bg-red-50');
  });
});
