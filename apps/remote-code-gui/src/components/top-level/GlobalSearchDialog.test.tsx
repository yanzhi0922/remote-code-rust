import { cleanup, fireEvent, render, screen, act } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { GlobalSearchDialog, type SearchItem } from './GlobalSearchDialog';

const baseItems: SearchItem[] = [
  { id: '1', label: 'src/index.ts', group: 'src', preview: 'export function main()', lineNumber: 1 },
  { id: '2', label: 'src/App.tsx', group: 'src', preview: 'export function App()', lineNumber: 5 },
  { id: '3', label: 'README.md', group: 'root', preview: '# Project Title' },
  { id: '4', label: 'package.json', group: 'root', preview: '"name": "test"' },
];

describe('GlobalSearchDialog', () => {
  afterEach(() => { cleanup(); });

  it('renders when open is true', () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} />);
    expect(screen.getByTestId('global-search-dialog')).toBeInTheDocument();
  });

  it('does not render when open is false', () => {
    render(<GlobalSearchDialog open={false} onClose={vi.fn()} items={baseItems} />);
    expect(screen.queryByTestId('global-search-dialog')).not.toBeInTheDocument();
  });

  it('renders search input', () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} />);
    expect(screen.getByTestId('global-search-input')).toBeInTheDocument();
  });

  it('renders backdrop', () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} />);
    expect(screen.getByTestId('global-search-backdrop')).toBeInTheDocument();
  });

  it('calls onClose when backdrop is clicked', () => {
    const fn = vi.fn();
    render(<GlobalSearchDialog open={true} onClose={fn} items={baseItems} />);
    fireEvent.click(screen.getByTestId('global-search-backdrop'));
    expect(fn).toHaveBeenCalled();
  });

  it('renders all items by default', () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} debounceMs={0} />);
    expect(screen.getByTestId('global-search-item-1')).toBeInTheDocument();
    expect(screen.getByTestId('global-search-item-2')).toBeInTheDocument();
    expect(screen.getByTestId('global-search-item-3')).toBeInTheDocument();
    expect(screen.getByTestId('global-search-item-4')).toBeInTheDocument();
  });

  it('filters items by search query', async () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} debounceMs={0} />);
    const input = screen.getByTestId('global-search-input');
    fireEvent.change(input, { target: { value: 'index' } });
    // Wait for debounce
    await act(async () => { await new Promise((r) => setTimeout(r, 50)); });
    expect(screen.getByTestId('global-search-item-1')).toBeInTheDocument();
    expect(screen.queryByTestId('global-search-item-2')).not.toBeInTheDocument();
  });

  it('shows empty state when no results', async () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} debounceMs={0} />);
    const input = screen.getByTestId('global-search-input');
    fireEvent.change(input, { target: { value: 'zzzzz' } });
    await act(async () => { await new Promise((r) => setTimeout(r, 50)); });
    expect(screen.getByTestId('global-search-empty')).toBeInTheDocument();
  });

  it('calls onSelect when item is clicked', () => {
    const fn = vi.fn();
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} onSelect={fn} debounceMs={0} />);
    fireEvent.click(screen.getByTestId('global-search-item-1'));
    expect(fn).toHaveBeenCalledWith(expect.objectContaining({ id: '1' }));
  });

  it('renders group headers', () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} debounceMs={0} />);
    // Should show group headers for "src" and "root"
    expect(screen.getByText('src')).toBeInTheDocument();
    expect(screen.getByText('root')).toBeInTheDocument();
  });

  it('renders footer with keyboard hints', () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} />);
    expect(screen.getByText('↑↓ 导航')).toBeInTheDocument();
    expect(screen.getByText('Enter 选择')).toBeInTheDocument();
    expect(screen.getByText('Esc 关闭')).toBeInTheDocument();
  });

  it('calls onClose on Escape key', () => {
    const fn = vi.fn();
    render(<GlobalSearchDialog open={true} onClose={fn} items={baseItems} />);
    fireEvent.keyDown(screen.getByTestId('global-search-input'), { key: 'Escape' });
    expect(fn).toHaveBeenCalled();
  });

  it('shows loading indicator while searching', () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} debounceMs={500} />);
    const input = screen.getByTestId('global-search-input');
    fireEvent.change(input, { target: { value: 'test' } });
    // Should show loading immediately after typing
    expect(screen.getByTestId('global-search-loading')).toBeInTheDocument();
  });

  it('truncates results beyond maxResults', () => {
    const manyItems = Array.from({ length: 60 }, (_, i) => ({
      id: String(i),
      label: `file-${i}.ts`,
      group: 'src',
    }));
    render(
      <GlobalSearchDialog open={true} onClose={vi.fn()} items={manyItems} maxResults={50} debounceMs={0} />,
    );
    // Should show truncation notice with the max count
    const resultsContainer = screen.getByTestId('global-search-results');
    expect(resultsContainer.textContent).toContain('50');
  });

  it('renders results container', () => {
    render(<GlobalSearchDialog open={true} onClose={vi.fn()} items={baseItems} />);
    expect(screen.getByTestId('global-search-results')).toBeInTheDocument();
  });
});
