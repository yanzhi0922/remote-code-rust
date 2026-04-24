import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { HistorySearchInput } from './HistorySearchInput';

describe('HistorySearchInput', () => {
  afterEach(cleanup);

  const defaultProps = {
    visible: true,
    query: '',
    onQueryChange: vi.fn(),
    onSelect: vi.fn(),
    results: ['cmd1', 'cmd2', 'cmd3'],
    selectedIndex: 0,
    onClose: vi.fn(),
  };

  it('visible=true 时渲染并显示 data-testid', () => {
    render(<HistorySearchInput {...defaultProps} />);
    expect(screen.getByTestId('history-search-input')).toBeInTheDocument();
  });

  it('visible=false 时返回 null', () => {
    render(<HistorySearchInput {...defaultProps} visible={false} />);
    expect(screen.queryByTestId('history-search-input')).not.toBeInTheDocument();
  });

  it('显示搜索输入框', () => {
    render(<HistorySearchInput {...defaultProps} />);
    expect(screen.getByPlaceholderText('搜索历史命令...')).toBeInTheDocument();
  });

  it('显示搜索结果列表', () => {
    render(<HistorySearchInput {...defaultProps} />);
    expect(screen.getByText('cmd1')).toBeInTheDocument();
    expect(screen.getByText('cmd2')).toBeInTheDocument();
    expect(screen.getByText('cmd3')).toBeInTheDocument();
  });

  it('selectedIndex 高亮当前选项', () => {
    render(<HistorySearchInput {...defaultProps} selectedIndex={1} />);
    const secondItem = screen.getByText('cmd2').closest('button');
    expect(secondItem?.className).toContain('bg-blue-50');
  });

  it('输入搜索词触发 onQueryChange', () => {
    const onQueryChange = vi.fn();
    render(<HistorySearchInput {...defaultProps} onQueryChange={onQueryChange} />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'test' } });
    expect(onQueryChange).toHaveBeenCalledWith('test');
  });

  it('点击结果项触发 onSelect', () => {
    const onSelect = vi.fn();
    render(<HistorySearchInput {...defaultProps} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('cmd2'));
    expect(onSelect).toHaveBeenCalledWith('cmd2');
  });

  it('点击关闭按钮触发 onClose', () => {
    const onClose = vi.fn();
    render(<HistorySearchInput {...defaultProps} onClose={onClose} />);
    fireEvent.click(screen.getByText('按 Esc 关闭'));
    expect(onClose).toHaveBeenCalled();
  });
});
