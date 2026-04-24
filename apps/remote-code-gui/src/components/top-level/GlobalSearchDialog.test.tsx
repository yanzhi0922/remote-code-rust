import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { GlobalSearchDialog } from './GlobalSearchDialog';

afterEach(() => {
  cleanup();
});

describe('GlobalSearchDialog', () => {
  const items = [
    { id: '1', label: 'Session A', group: '会话' },
    { id: '2', label: 'Session B', group: '会话' },
  ];

  it('renders nothing when closed', () => {
    const { container } = render(<GlobalSearchDialog open={false} onClose={() => {}} items={items} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders search dialog', () => {
    render(<GlobalSearchDialog open={true} onClose={() => {}} items={items} />);
    expect(screen.getByTestId('global-search-dialog')).toBeInTheDocument();
  });

  it('filters items', () => {
    render(<GlobalSearchDialog open={true} onClose={() => {}} items={items} />);
    fireEvent.change(screen.getByTestId('global-search-input'), { target: { value: 'A' } });
    expect(screen.getByTestId('global-search-item-1')).toBeInTheDocument();
    expect(screen.queryByTestId('global-search-item-2')).not.toBeInTheDocument();
  });

  it('calls onClose on backdrop click', () => {
    const onClose = vi.fn();
    render(<GlobalSearchDialog open={true} onClose={onClose} items={items} />);
    fireEvent.click(screen.getByTestId('global-search-backdrop'));
    expect(onClose).toHaveBeenCalled();
  });

  it('calls onSelect', () => {
    const onSelect = vi.fn();
    render(<GlobalSearchDialog open={true} onClose={() => {}} items={items} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('global-search-item-1'));
    expect(onSelect).toHaveBeenCalledWith(items[0]);
  });
});
