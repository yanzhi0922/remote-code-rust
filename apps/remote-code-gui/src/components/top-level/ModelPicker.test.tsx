import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { ModelPicker, type ModelOption } from './ModelPicker';

const models: ModelOption[] = [
  { value: 'gpt-4', label: 'GPT-4', description: 'Most capable' },
  { value: 'claude-3', label: 'Claude 3' },
];

describe('ModelPicker', () => {
  afterEach(() => { cleanup(); });

  it('renders all model options', () => {
    const { getByTestId, getByText } = render(
      <ModelPicker models={models} currentModel={null} onSelect={() => {}} />,
    );
    expect(getByTestId('model-picker')).toBeInTheDocument();
    expect(getByText('GPT-4')).toBeInTheDocument();
    expect(getByText('Claude 3')).toBeInTheDocument();
  });

  it('shows check icon for current model', () => {
    const { getByTestId } = render(
      <ModelPicker models={models} currentModel="gpt-4" onSelect={() => {}} />,
    );
    const gptBtn = getByTestId('model-option-gpt-4');
    expect(gptBtn.querySelector('svg')).toBeInTheDocument();
  });

  it('calls onSelect when model clicked', () => {
    const onSelect = vi.fn();
    const { getByTestId } = render(
      <ModelPicker models={models} currentModel={null} onSelect={onSelect} />,
    );
    fireEvent.click(getByTestId('model-option-claude-3'));
    expect(onSelect).toHaveBeenCalledWith('claude-3');
  });

  it('renders cancel button when onCancel provided', () => {
    const { getByTestId } = render(
      <ModelPicker models={models} currentModel={null} onSelect={() => {}} onCancel={() => {}} />,
    );
    expect(getByTestId('model-picker-cancel')).toBeInTheDocument();
  });

  it('calls onCancel when cancel clicked', () => {
    const onCancel = vi.fn();
    const { getByTestId } = render(
      <ModelPicker models={models} currentModel={null} onSelect={() => {}} onCancel={onCancel} />,
    );
    fireEvent.click(getByTestId('model-picker-cancel'));
    expect(onCancel).toHaveBeenCalled();
  });
});
