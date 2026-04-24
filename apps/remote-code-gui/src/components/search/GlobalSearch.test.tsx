import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { GlobalSearch } from './GlobalSearch';

describe('GlobalSearch', () => {
  afterEach(cleanup);

  it('renders when visible is true', () => {
    render(<GlobalSearch visible={true} onClose={vi.fn()} onSelectResult={vi.fn()} />);
    expect(screen.getByTestId('global-search')).toBeInTheDocument();
  });

  it('does not render when visible is false', () => {
    render(<GlobalSearch visible={false} onClose={vi.fn()} onSelectResult={vi.fn()} />);
    expect(screen.queryByTestId('global-search')).not.toBeInTheDocument();
  });

  it('shows history when query is empty', () => {
    render(<GlobalSearch visible={true} onClose={vi.fn()} onSelectResult={vi.fn()} />);
    expect(screen.getByTestId('history-search')).toBeInTheDocument();
  });

  it('shows filtered results when typing', () => {
    render(<GlobalSearch visible={true} onClose={vi.fn()} onSelectResult={vi.fn()} />);
    const input = screen.getByTestId('search-input');
    fireEvent.change(input, { target: { value: '格式化' } });
    expect(screen.getByText('格式化文档')).toBeInTheDocument();
  });

  it('calls onClose when close button is clicked', () => {
    const onClose = vi.fn();
    render(<GlobalSearch visible={true} onClose={onClose} onSelectResult={vi.fn()} />);
    fireEvent.click(screen.getByTestId('global-search-close'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when overlay is clicked', () => {
    const onClose = vi.fn();
    render(<GlobalSearch visible={true} onClose={onClose} onSelectResult={vi.fn()} />);
    fireEvent.click(screen.getByTestId('global-search-overlay'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onSelectResult when a result is clicked', () => {
    const onSelectResult = vi.fn();
    render(
      <GlobalSearch visible={true} onClose={vi.fn()} onSelectResult={onSelectResult} />,
    );
    const input = screen.getByTestId('search-input');
    fireEvent.change(input, { target: { value: '格式化' } });
    fireEvent.click(screen.getByTestId('search-result-0'));
    expect(onSelectResult).toHaveBeenCalledTimes(1);
    expect(onSelectResult.mock.calls[0][0].title).toBe('格式化文档');
  });

  it('shows keyboard navigation hint', () => {
    render(<GlobalSearch visible={true} onClose={vi.fn()} onSelectResult={vi.fn()} />);
    expect(screen.getByText(/↑↓ 导航/)).toBeInTheDocument();
  });
});
