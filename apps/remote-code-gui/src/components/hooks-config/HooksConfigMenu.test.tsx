import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { HooksConfigMenu } from './HooksConfigMenu';

const hooks = [
  { id: 'h1', event: 'PreToolUse', matcher: 'Bash', command: 'echo hi', enabled: true },
  { id: 'h2', event: 'PostToolUse', matcher: 'Edit', command: 'echo done', enabled: false },
];

describe('HooksConfigMenu', () => {
  afterEach(cleanup);

  it('renders hooks config menu', () => {
    render(
      <HooksConfigMenu
        hooks={hooks}
        onToggle={vi.fn()}
        onEdit={vi.fn()}
        onAdd={vi.fn()}
      />,
    );
    expect(screen.getByTestId('hooks-config-menu')).toBeInTheDocument();
  });

  it('shows hook items with event and matcher', () => {
    render(
      <HooksConfigMenu
        hooks={hooks}
        onToggle={vi.fn()}
        onEdit={vi.fn()}
        onAdd={vi.fn()}
      />,
    );
    expect(screen.getByText('PreToolUse')).toBeInTheDocument();
    expect(screen.getByText('PostToolUse')).toBeInTheDocument();
    expect(screen.getByText('Bash')).toBeInTheDocument();
  });

  it('shows empty message when no hooks', () => {
    render(
      <HooksConfigMenu
        hooks={[]}
        onToggle={vi.fn()}
        onEdit={vi.fn()}
        onAdd={vi.fn()}
      />,
    );
    expect(screen.getByText('暂无 Hook 配置')).toBeInTheDocument();
  });

  it('calls onAdd when add button clicked', () => {
    const onAdd = vi.fn();
    render(
      <HooksConfigMenu
        hooks={hooks}
        onToggle={vi.fn()}
        onEdit={vi.fn()}
        onAdd={onAdd}
      />,
    );
    fireEvent.click(screen.getByTestId('hooks-add-btn'));
    expect(onAdd).toHaveBeenCalledOnce();
  });

  it('calls onToggle when toggle clicked', () => {
    const onToggle = vi.fn();
    render(
      <HooksConfigMenu
        hooks={hooks}
        onToggle={onToggle}
        onEdit={vi.fn()}
        onAdd={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('hook-toggle-h1'));
    expect(onToggle).toHaveBeenCalledWith('h1');
  });

  it('calls onEdit when edit clicked', () => {
    const onEdit = vi.fn();
    render(
      <HooksConfigMenu
        hooks={hooks}
        onToggle={vi.fn()}
        onEdit={onEdit}
        onAdd={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('hook-edit-h2'));
    expect(onEdit).toHaveBeenCalledWith('h2');
  });

  it('shows enabled toggle for enabled hook', () => {
    render(
      <HooksConfigMenu
        hooks={hooks}
        onToggle={vi.fn()}
        onEdit={vi.fn()}
        onAdd={vi.fn()}
      />,
    );
    const toggleBtn = screen.getByTestId('hook-toggle-h1');
    expect(toggleBtn.querySelector('.text-green-500')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(
      <HooksConfigMenu
        hooks={hooks}
        onToggle={vi.fn()}
        onEdit={vi.fn()}
        onAdd={vi.fn()}
        className="my-menu"
      />,
    );
    expect(screen.getByTestId('hooks-config-menu').className).toContain('my-menu');
  });
});
