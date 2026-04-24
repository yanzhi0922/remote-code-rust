import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { WorkflowMultiselectDialog } from './WorkflowMultiselectDialog';

afterEach(() => {
  cleanup();
});

describe('WorkflowMultiselectDialog', () => {
  it('renders with data-testid', () => {
    render(<WorkflowMultiselectDialog onSubmit={vi.fn()} />);
    expect(screen.getByTestId('workflow-multiselect-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<WorkflowMultiselectDialog onSubmit={vi.fn()} />);
    expect(screen.getByText('Select Workflows')).toBeInTheDocument();
  });

  it('shows workflow options', () => {
    render(<WorkflowMultiselectDialog onSubmit={vi.fn()} />);
    expect(screen.getByText(/@Claude Code/)).toBeInTheDocument();
    expect(screen.getByText(/Claude Code Review/)).toBeInTheDocument();
  });

  it('calls onSubmit with selected workflows', () => {
    const onSubmit = vi.fn();
    render(<WorkflowMultiselectDialog onSubmit={onSubmit} defaultSelections={['claude']} />);
    fireEvent.click(screen.getByTestId('workflow-multiselect-confirm'));
    expect(onSubmit).toHaveBeenCalledWith(['claude']);
  });

  it('shows error when no workflows selected', () => {
    render(<WorkflowMultiselectDialog onSubmit={vi.fn()} />);
    // Deselect the default (none selected by default)
    fireEvent.click(screen.getByTestId('workflow-multiselect-confirm'));
    expect(screen.getByText(/Please select at least one/)).toBeInTheDocument();
  });

  it('shows examples link', () => {
    render(<WorkflowMultiselectDialog onSubmit={vi.fn()} />);
    expect(screen.getByText(/More workflow examples/)).toBeInTheDocument();
  });
});
