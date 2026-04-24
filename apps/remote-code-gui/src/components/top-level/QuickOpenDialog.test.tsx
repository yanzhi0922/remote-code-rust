import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { QuickOpenDialog } from './QuickOpenDialog';

afterEach(() => {
  cleanup();
});

const results = [
  { path: 'src/app.tsx', label: 'app.tsx' },
  { path: 'src/components/Button.tsx', label: 'Button.tsx' },
  { path: 'src/utils/helpers.ts', label: 'helpers.ts' },
];

describe('QuickOpenDialog', () => {
  it('renders dialog with search input', () => {
    render(<QuickOpenDialog onDone={vi.fn()} results={results} />);
    expect(screen.getByTestId('quick-open-dialog')).toBeInTheDocument();
    expect(screen.getByTestId('quick-open-input')).toBeInTheDocument();
  });

  it('shows results list', () => {
    render(<QuickOpenDialog onDone={vi.fn()} results={results} />);
    expect(screen.getByTestId('quick-open-results')).toBeInTheDocument();
    expect(screen.getByTestId('quick-open-result-0')).toBeInTheDocument();
    expect(screen.getByTestId('quick-open-result-2')).toBeInTheDocument();
  });

  it('filters results by query', () => {
    render(<QuickOpenDialog onDone={vi.fn()} results={results} />);
    fireEvent.change(screen.getByTestId('quick-open-input'), { target: { value: 'Button' } });
    expect(screen.getByTestId('quick-open-result-0')).toHaveTextContent('Button.tsx');
    expect(screen.queryByTestId('quick-open-result-1')).not.toBeInTheDocument();
  });

  it('shows empty state when no results match', () => {
    render(<QuickOpenDialog onDone={vi.fn()} results={results} />);
    fireEvent.change(screen.getByTestId('quick-open-input'), { target: { value: 'xyz' } });
    expect(screen.getByTestId('quick-open-empty')).toBeInTheDocument();
  });

  it('calls onDone when close button is clicked', () => {
    const onDone = vi.fn();
    render(<QuickOpenDialog onDone={onDone} results={results} />);
    fireEvent.click(screen.getByTestId('quick-open-close'));
    expect(onDone).toHaveBeenCalled();
  });

  it('calls onInsert and onDone when result is clicked', () => {
    const onDone = vi.fn();
    const onInsert = vi.fn();
    render(<QuickOpenDialog onDone={onDone} onInsert={onInsert} results={results} />);
    fireEvent.click(screen.getByTestId('quick-open-result-0'));
    expect(onInsert).toHaveBeenCalledWith('src/app.tsx');
    expect(onDone).toHaveBeenCalled();
  });
});
