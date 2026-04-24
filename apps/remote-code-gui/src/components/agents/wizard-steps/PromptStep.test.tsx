import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PromptStep } from './PromptStep';

describe('PromptStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<PromptStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('wizard-prompt-step')).toBeInTheDocument();
  });

  it('renders textarea with current value', () => {
    render(<PromptStep value="测试提示词" onChange={vi.fn()} />);
    const input = screen.getByTestId('prompt-input') as HTMLTextAreaElement;
    expect(input.value).toBe('测试提示词');
  });

  it('calls onChange when text is entered', () => {
    const onChange = vi.fn();
    render(<PromptStep value="" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('prompt-input'), { target: { value: '新提示词' } });
    expect(onChange).toHaveBeenCalledWith('新提示词');
  });

  it('shows template suggestions when toggle is clicked', () => {
    render(<PromptStep value="" onChange={vi.fn()} />);
    expect(screen.queryByTestId('template-suggestions')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('toggle-templates'));
    expect(screen.getByTestId('template-suggestions')).toBeInTheDocument();
  });

  it('applies template when suggestion is clicked', () => {
    const onChange = vi.fn();
    render(<PromptStep value="" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('toggle-templates'));
    fireEvent.click(screen.getByTestId('template-代码审查助手'));
    expect(onChange).toHaveBeenCalled();
  });

  it('shows character count', () => {
    render(<PromptStep value="hello" onChange={vi.fn()} />);
    expect(screen.getByText('5 字符')).toBeInTheDocument();
  });

  it('hides templates on second toggle click', () => {
    render(<PromptStep value="" onChange={vi.fn()} />);
    fireEvent.click(screen.getByTestId('toggle-templates'));
    expect(screen.getByTestId('template-suggestions')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('toggle-templates'));
    expect(screen.queryByTestId('template-suggestions')).not.toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<PromptStep value="" onChange={vi.fn()} className="my-class" />);
    expect(screen.getByTestId('wizard-prompt-step').className).toContain('my-class');
  });
});
