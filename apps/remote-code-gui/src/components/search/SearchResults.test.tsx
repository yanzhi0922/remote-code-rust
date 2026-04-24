import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SearchResults, type SearchResult } from './SearchResults';

const SAMPLE_RESULTS: SearchResult[] = [
  { type: 'file', title: 'src/app.ts', subtitle: 'TypeScript 文件' },
  { type: 'message', title: '关于重构的讨论', subtitle: '昨天' },
  { type: 'command', title: '格式化文档' },
  { type: 'setting', title: '主题设置', subtitle: '外观' },
];

describe('SearchResults', () => {
  afterEach(cleanup);

  it('renders all results', () => {
    render(
      <SearchResults
        results={SAMPLE_RESULTS}
        selectedIndex={-1}
        onSelect={vi.fn()}
        onHover={vi.fn()}
      />,
    );
    expect(screen.getByText('src/app.ts')).toBeInTheDocument();
    expect(screen.getByText('关于重构的讨论')).toBeInTheDocument();
    expect(screen.getByText('格式化文档')).toBeInTheDocument();
    expect(screen.getByText('主题设置')).toBeInTheDocument();
  });

  it('shows empty state when no results', () => {
    render(
      <SearchResults results={[]} selectedIndex={-1} onSelect={vi.fn()} onHover={vi.fn()} />,
    );
    expect(screen.getByTestId('search-results-empty')).toHaveTextContent('未找到匹配结果');
  });

  it('highlights selected item', () => {
    render(
      <SearchResults
        results={SAMPLE_RESULTS}
        selectedIndex={1}
        onSelect={vi.fn()}
        onHover={vi.fn()}
      />,
    );
    const el = screen.getByTestId('search-result-1');
    expect(el.className).toContain('bg-blue-50');
  });

  it('calls onSelect when result is clicked', () => {
    const onSelect = vi.fn();
    render(
      <SearchResults
        results={SAMPLE_RESULTS}
        selectedIndex={-1}
        onSelect={onSelect}
        onHover={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('search-result-0'));
    expect(onSelect).toHaveBeenCalledWith(0);
  });

  it('calls onHover on mouse enter', () => {
    const onHover = vi.fn();
    render(
      <SearchResults
        results={SAMPLE_RESULTS}
        selectedIndex={-1}
        onSelect={vi.fn()}
        onHover={onHover}
      />,
    );
    fireEvent.mouseEnter(screen.getByTestId('search-result-2'));
    expect(onHover).toHaveBeenCalledWith(2);
  });

  it('shows result count', () => {
    render(
      <SearchResults
        results={SAMPLE_RESULTS}
        selectedIndex={-1}
        onSelect={vi.fn()}
        onHover={vi.fn()}
      />,
    );
    expect(screen.getByText('4 个结果')).toBeInTheDocument();
  });

  it('groups results by type', () => {
    render(
      <SearchResults
        results={SAMPLE_RESULTS}
        selectedIndex={-1}
        onSelect={vi.fn()}
        onHover={vi.fn()}
      />,
    );
    expect(screen.getByText('文件')).toBeInTheDocument();
    expect(screen.getByText('消息')).toBeInTheDocument();
    expect(screen.getByText('命令')).toBeInTheDocument();
    expect(screen.getByText('设置')).toBeInTheDocument();
  });
});
