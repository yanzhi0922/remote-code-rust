import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ToolSelector } from './ToolSelector';

const TOOLS = ['Bash', 'FileRead', 'FileEdit', 'Grep', 'Glob'];

describe('ToolSelector', () => {
  afterEach(cleanup);

  it('renders all available tools sorted alphabetically', () => {
    render(<ToolSelector selectedTools={[]} onToggle={vi.fn()} availableTools={TOOLS} />);
    expect(screen.getByText('Bash')).toBeInTheDocument();
    expect(screen.getByText('FileRead')).toBeInTheDocument();
    expect(screen.getByText('FileEdit')).toBeInTheDocument();
    expect(screen.getByText('Grep')).toBeInTheDocument();
    expect(screen.getByText('Glob')).toBeInTheDocument();
  });

  it('shows selected count badge', () => {
    render(<ToolSelector selectedTools={['Bash', 'Grep']} onToggle={vi.fn()} availableTools={TOOLS} />);
    expect(screen.getByText('已选 2')).toBeInTheDocument();
  });

  it('calls onToggle when a tool checkbox is clicked', () => {
    const onToggle = vi.fn();
    render(<ToolSelector selectedTools={[]} onToggle={onToggle} availableTools={TOOLS} />);
    fireEvent.click(screen.getByText('Bash'));
    expect(onToggle).toHaveBeenCalledWith('Bash');
  });

  it('filters tools by search query', () => {
    render(<ToolSelector selectedTools={[]} onToggle={vi.fn()} availableTools={TOOLS} />);
    const searchInput = screen.getByLabelText('搜索工具');
    fireEvent.change(searchInput, { target: { value: 'file' } });
    expect(screen.getByText('FileRead')).toBeInTheDocument();
    expect(screen.getByText('FileEdit')).toBeInTheDocument();
    expect(screen.queryByText('Bash')).not.toBeInTheDocument();
  });

  it('selects all visible tools when "全选" is clicked', () => {
    const onToggle = vi.fn();
    render(<ToolSelector selectedTools={[]} onToggle={onToggle} availableTools={TOOLS} />);
    fireEvent.click(screen.getByText('全选'));
    expect(onToggle).toHaveBeenCalledTimes(5);
  });

  it('deselects all visible tools when "取消全选" is clicked', () => {
    const onToggle = vi.fn();
    render(<ToolSelector selectedTools={TOOLS} onToggle={onToggle} availableTools={TOOLS} />);
    fireEvent.click(screen.getByText('取消全选'));
    expect(onToggle).toHaveBeenCalledTimes(5);
  });

  it('shows empty message when no tools match search', () => {
    render(<ToolSelector selectedTools={[]} onToggle={vi.fn()} availableTools={TOOLS} />);
    const searchInput = screen.getByLabelText('搜索工具');
    fireEvent.change(searchInput, { target: { value: 'xyz' } });
    expect(screen.getByText('无匹配工具')).toBeInTheDocument();
  });
});
