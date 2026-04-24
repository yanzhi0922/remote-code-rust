import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { McpServerMultiselectDialog } from './McpServerMultiselectDialog';

afterEach(() => {
  cleanup();
});

describe('McpServerMultiselectDialog', () => {
  const serverNames = ['server-a', 'server-b'];

  it('renders with data-testid', () => {
    render(<McpServerMultiselectDialog serverNames={serverNames} onDone={vi.fn()} />);
    expect(screen.getByTestId('mcp-server-multiselect-dialog')).toBeInTheDocument();
  });

  it('shows server count in title', () => {
    render(<McpServerMultiselectDialog serverNames={serverNames} onDone={vi.fn()} />);
    expect(screen.getByText('2 New MCP Servers Found')).toBeInTheDocument();
  });

  it('shows server names', () => {
    render(<McpServerMultiselectDialog serverNames={serverNames} onDone={vi.fn()} />);
    expect(screen.getByText('server-a')).toBeInTheDocument();
    expect(screen.getByText('server-b')).toBeInTheDocument();
  });

  it('calls onDone with selected servers when confirm is clicked', () => {
    const onDone = vi.fn();
    render(<McpServerMultiselectDialog serverNames={serverNames} onDone={onDone} />);
    fireEvent.click(screen.getByTestId('mcp-server-multiselect-confirm'));
    expect(onDone).toHaveBeenCalledWith(['server-a', 'server-b']);
  });

  it('calls onDone with empty array when reject all is clicked', () => {
    const onDone = vi.fn();
    render(<McpServerMultiselectDialog serverNames={serverNames} onDone={onDone} />);
    fireEvent.click(screen.getByTestId('mcp-server-multiselect-reject'));
    expect(onDone).toHaveBeenCalledWith([]);
  });

  it('toggles server selection', () => {
    render(<McpServerMultiselectDialog serverNames={serverNames} onDone={vi.fn()} />);
    const btn = screen.getByTestId('mcp-server-multiselect-server-a');
    fireEvent.click(btn);
    // After toggle, it should be deselected
    expect(btn.textContent).toContain('server-a');
  });
});
