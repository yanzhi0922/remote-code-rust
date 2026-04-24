import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PromptInputFooterSuggestions } from './PromptInputFooterSuggestions';

describe('PromptInputFooterSuggestions', () => {
  afterEach(cleanup);

  const suggestions = ['选项一', '选项二', '选项三'];

  it('visible=true 时渲染并显示 data-testid', () => {
    render(
      <PromptInputFooterSuggestions
        suggestions={suggestions}
        selectedIndex={0}
        onSelect={vi.fn()}
        visible
      />,
    );
    expect(screen.getByTestId('prompt-suggestions')).toBeInTheDocument();
  });

  it('visible=false 时返回 null', () => {
    render(
      <PromptInputFooterSuggestions
        suggestions={suggestions}
        selectedIndex={0}
        onSelect={vi.fn()}
        visible={false}
      />,
    );
    expect(screen.queryByTestId('prompt-suggestions')).not.toBeInTheDocument();
  });

  it('suggestions 为空时返回 null', () => {
    render(
      <PromptInputFooterSuggestions
        suggestions={[]}
        selectedIndex={0}
        onSelect={vi.fn()}
        visible
      />,
    );
    expect(screen.queryByTestId('prompt-suggestions')).not.toBeInTheDocument();
  });

  it('显示所有建议项', () => {
    render(
      <PromptInputFooterSuggestions
        suggestions={suggestions}
        selectedIndex={0}
        onSelect={vi.fn()}
        visible
      />,
    );
    expect(screen.getByText('选项一')).toBeInTheDocument();
    expect(screen.getByText('选项二')).toBeInTheDocument();
    expect(screen.getByText('选项三')).toBeInTheDocument();
  });

  it('selectedIndex 高亮当前选项', () => {
    render(
      <PromptInputFooterSuggestions
        suggestions={suggestions}
        selectedIndex={1}
        onSelect={vi.fn()}
        visible
      />,
    );
    const secondItem = screen.getByText('选项二').closest('button');
    expect(secondItem?.className).toContain('bg-blue-50');
  });

  it('点击建议项触发 onSelect', () => {
    const onSelect = vi.fn();
    render(
      <PromptInputFooterSuggestions
        suggestions={suggestions}
        selectedIndex={0}
        onSelect={onSelect}
        visible
      />,
    );
    fireEvent.click(screen.getByText('选项二'));
    expect(onSelect).toHaveBeenCalledWith(1);
  });
});
