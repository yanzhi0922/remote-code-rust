import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { HistorySearch } from './HistorySearch';

const SAMPLE_HISTORY = [
  { id: '1', query: '如何使用 React', timestamp: '10:30' },
  { id: '2', query: 'TypeScript 配置', timestamp: '09:15' },
  { id: '3', query: 'Tailwind CSS', timestamp: '昨天' },
];

describe('HistorySearch', () => {
  afterEach(cleanup);

  it('renders history items', () => {
    render(<HistorySearch history={SAMPLE_HISTORY} onSelect={vi.fn()} onClearHistory={vi.fn()} />);
    expect(screen.getByText('如何使用 React')).toBeInTheDocument();
    expect(screen.getByText('TypeScript 配置')).toBeInTheDocument();
    expect(screen.getByText('Tailwind CSS')).toBeInTheDocument();
  });

  it('shows timestamps', () => {
    render(<HistorySearch history={SAMPLE_HISTORY} onSelect={vi.fn()} onClearHistory={vi.fn()} />);
    expect(screen.getByText('10:30')).toBeInTheDocument();
    expect(screen.getByText('09:15')).toBeInTheDocument();
  });

  it('calls onSelect when item is clicked', () => {
    const onSelect = vi.fn();
    render(<HistorySearch history={SAMPLE_HISTORY} onSelect={onSelect} onClearHistory={vi.fn()} />);
    fireEvent.click(screen.getByTestId('history-item-2'));
    expect(onSelect).toHaveBeenCalledWith('2');
  });

  it('calls onClearHistory when clear button is clicked', () => {
    const onClearHistory = vi.fn();
    render(<HistorySearch history={SAMPLE_HISTORY} onSelect={vi.fn()} onClearHistory={onClearHistory} />);
    fireEvent.click(screen.getByTestId('clear-history'));
    expect(onClearHistory).toHaveBeenCalledTimes(1);
  });

  it('shows empty state when no history', () => {
    render(<HistorySearch history={[]} onSelect={vi.fn()} onClearHistory={vi.fn()} />);
    expect(screen.getByTestId('history-empty')).toHaveTextContent('暂无搜索历史');
  });

  it('renders clear history button', () => {
    render(<HistorySearch history={SAMPLE_HISTORY} onSelect={vi.fn()} onClearHistory={vi.fn()} />);
    expect(screen.getByTestId('clear-history')).toBeInTheDocument();
    expect(screen.getByText('清除历史')).toBeInTheDocument();
  });
});
