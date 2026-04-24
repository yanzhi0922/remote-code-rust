import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ModelStep } from './ModelStep';

const MODELS = ['claude-sonnet-4', 'gpt-4o', 'gemini-pro'];

describe('ModelStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ModelStep value="" onChange={vi.fn()} availableModels={MODELS} />);
    expect(screen.getByTestId('wizard-model-step')).toBeInTheDocument();
  });

  it('renders all available models', () => {
    render(<ModelStep value="" onChange={vi.fn()} availableModels={MODELS} />);
    expect(screen.getByTestId('model-option-claude-sonnet-4')).toBeInTheDocument();
    expect(screen.getByTestId('model-option-gpt-4o')).toBeInTheDocument();
    expect(screen.getByTestId('model-option-gemini-pro')).toBeInTheDocument();
  });

  it('shows selected state for current value', () => {
    render(<ModelStep value="gpt-4o" onChange={vi.fn()} availableModels={MODELS} />);
    const selectedBtn = screen.getByTestId('model-option-gpt-4o');
    expect(selectedBtn.className).toContain('border-blue-500');
  });

  it('calls onChange when a model is clicked', () => {
    const onChange = vi.fn();
    render(<ModelStep value="" onChange={onChange} availableModels={MODELS} />);
    fireEvent.click(screen.getByTestId('model-option-gemini-pro'));
    expect(onChange).toHaveBeenCalledWith('gemini-pro');
  });

  it('shows empty state when no models available', () => {
    render(<ModelStep value="" onChange={vi.fn()} availableModels={[]} />);
    expect(screen.getByTestId('no-models')).toBeInTheDocument();
    expect(screen.getByText('暂无可用模型')).toBeInTheDocument();
  });

  it('shows check icon for selected model', () => {
    render(<ModelStep value="claude-sonnet-4" onChange={vi.fn()} availableModels={MODELS} />);
    const selectedBtn = screen.getByTestId('model-option-claude-sonnet-4');
    expect(selectedBtn.querySelector('svg')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<ModelStep value="" onChange={vi.fn()} availableModels={MODELS} className="test-cls" />);
    expect(screen.getByTestId('wizard-model-step').className).toContain('test-cls');
  });
});
