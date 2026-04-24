import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ContextSuggestions } from './ContextSuggestions';

afterEach(() => {
  cleanup();
});

describe('ContextSuggestions', () => {
  it('renders nothing when empty', () => {
    const { container } = render(<ContextSuggestions suggestions={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders suggestions', () => {
    const suggestions = [
      { id: '1', text: '打开文件', type: 'file' as const },
      { id: '2', text: '运行测试', type: 'command' as const },
    ];
    render(<ContextSuggestions suggestions={suggestions} />);
    expect(screen.getByTestId('context-suggestions')).toBeInTheDocument();
    expect(screen.getByText('打开文件')).toBeInTheDocument();
  });

  it('calls onSelect', () => {
    const onSelect = vi.fn();
    const suggestions = [{ id: '1', text: '提示', type: 'tip' as const }];
    render(<ContextSuggestions suggestions={suggestions} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('context-suggestion-1'));
    expect(onSelect).toHaveBeenCalledWith(suggestions[0]);
  });
});
