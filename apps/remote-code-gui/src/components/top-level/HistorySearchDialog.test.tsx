import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { HistorySearchDialog } from './HistorySearchDialog';

afterEach(() => {
  cleanup();
});

describe('HistorySearchDialog', () => {
  const entries = [
    { id: '1', text: 'Hello world', timestamp: '10:00' },
    { id: '2', text: 'Fix bug', timestamp: '11:00' },
  ];

  it('renders nothing when closed', () => {
    const { container } = render(<HistorySearchDialog open={false} onClose={() => {}} entries={entries} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders history entries', () => {
    render(<HistorySearchDialog open={true} onClose={() => {}} entries={entries} />);
    expect(screen.getByTestId('history-search-dialog')).toBeInTheDocument();
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('filters entries', () => {
    render(<HistorySearchDialog open={true} onClose={() => {}} entries={entries} />);
    fireEvent.change(screen.getByTestId('history-search-input'), { target: { value: 'bug' } });
    expect(screen.queryByText('Hello world')).not.toBeInTheDocument();
    expect(screen.getByText('Fix bug')).toBeInTheDocument();
  });

  it('calls onSelect', () => {
    const onSelect = vi.fn();
    render(<HistorySearchDialog open={true} onClose={() => {}} entries={entries} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('history-search-entry-1'));
    expect(onSelect).toHaveBeenCalledWith(entries[0]);
  });
});
