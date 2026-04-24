import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ModelSelector } from './ModelSelector';

const MODELS = [
  { id: 'claude-sonnet-4', name: 'Claude Sonnet 4', provider: 'Anthropic' },
  { id: 'claude-opus-4', name: 'Claude Opus 4', provider: 'Anthropic' },
  { id: 'gpt-4o', name: 'GPT-4o', provider: 'OpenAI' },
  { id: 'gpt-4o-mini', name: 'GPT-4o Mini', provider: 'OpenAI' },
];

describe('ModelSelector', () => {
  afterEach(cleanup);

  it('renders with default model label when value is null', () => {
    render(<ModelSelector value={null} onChange={vi.fn()} models={MODELS} />);
    expect(screen.getByText('使用默认模型')).toBeInTheDocument();
  });

  it('displays selected model name when value is set', () => {
    render(<ModelSelector value="claude-sonnet-4" onChange={vi.fn()} models={MODELS} />);
    expect(screen.getByText(/Claude Sonnet 4/)).toBeInTheDocument();
  });

  it('opens dropdown on click and shows model options grouped by provider', () => {
    render(<ModelSelector value={null} onChange={vi.fn()} models={MODELS} />);
    fireEvent.click(screen.getByRole('button', { name: /使用默认模型|模型/ }));
    expect(screen.getByText('Anthropic')).toBeInTheDocument();
    expect(screen.getByText('OpenAI')).toBeInTheDocument();
    expect(screen.getByText('GPT-4o')).toBeInTheDocument();
  });

  it('calls onChange with selected model id', () => {
    const onChange = vi.fn();
    render(<ModelSelector value={null} onChange={onChange} models={MODELS} />);
    fireEvent.click(screen.getByRole('button', { name: /使用默认模型|模型/ }));
    fireEvent.click(screen.getByText('GPT-4o'));
    expect(onChange).toHaveBeenCalledWith('gpt-4o');
  });

  it('calls onChange with null when default is selected', () => {
    const onChange = vi.fn();
    render(<ModelSelector value="gpt-4o" onChange={onChange} models={MODELS} />);
    fireEvent.click(screen.getByRole('button', { name: /GPT-4o|模型/ }));
    // Find the "使用默认模型" option inside the dropdown
    const defaultButtons = screen.getAllByText('使用默认模型');
    fireEvent.click(defaultButtons[defaultButtons.length - 1]);
    expect(onChange).toHaveBeenCalledWith(null);
  });

  it('filters models by search query', () => {
    render(<ModelSelector value={null} onChange={vi.fn()} models={MODELS} />);
    fireEvent.click(screen.getByRole('button', { name: /使用默认模型|模型/ }));
    const searchInput = screen.getByLabelText('搜索模型');
    fireEvent.change(searchInput, { target: { value: 'claude' } });
    expect(screen.getByText('Claude Sonnet 4')).toBeInTheDocument();
    expect(screen.queryByText('GPT-4o')).not.toBeInTheDocument();
  });
});
