import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserPromptMessage } from './UserPromptMessage';
import type { ContextSuggestion } from './UserPromptMessage';

describe('UserPromptMessage', () => {
  afterEach(cleanup);

  it('渲染提示文本', () => {
    render(<UserPromptMessage text="请检查这个文件" />);
    expect(screen.getByText('请检查这个文件')).toBeInTheDocument();
  });

  it('空文本返回 null', () => {
    const { container } = render(<UserPromptMessage text="  " />);
    expect(container.innerHTML).toBe('');
  });

  it('渲染上下文建议', () => {
    const suggestions: ContextSuggestion[] = [
      { label: '文件', description: 'src/main.ts' },
      { label: '命令', description: 'cargo build' },
    ];
    render(<UserPromptMessage text="检查" suggestions={suggestions} />);
    expect(screen.getByText('文件')).toBeInTheDocument();
    expect(screen.getByText('src/main.ts')).toBeInTheDocument();
    expect(screen.getByText('命令')).toBeInTheDocument();
  });

  it('无建议时不渲染建议区域', () => {
    const { container } = render(<UserPromptMessage text="检查" />);
    // 不应有 MessageSquare 图标对应的建议标签
    expect(container.querySelector('.bg-slate-100')).not.toBeInTheDocument();
  });
});
