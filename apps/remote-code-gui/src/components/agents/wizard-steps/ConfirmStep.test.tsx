import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ConfirmStep } from './ConfirmStep';

const BASE_CONFIG = {
  name: 'test-agent',
  type: 'subagent',
  model: 'claude-sonnet-4',
  tools: ['Bash', 'FileEdit'],
  color: '#3b82f6',
};

describe('ConfirmStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ConfirmStep config={BASE_CONFIG} onConfirm={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByTestId('wizard-confirm-step')).toBeInTheDocument();
  });

  it('displays config summary', () => {
    render(<ConfirmStep config={BASE_CONFIG} onConfirm={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByTestId('summary-name')).toHaveTextContent('test-agent');
    expect(screen.getByTestId('summary-type')).toHaveTextContent('subagent');
    expect(screen.getByTestId('summary-model')).toHaveTextContent('claude-sonnet-4');
  });

  it('displays tools in summary', () => {
    render(<ConfirmStep config={BASE_CONFIG} onConfirm={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByText('Bash')).toBeInTheDocument();
    expect(screen.getByText('FileEdit')).toBeInTheDocument();
  });

  it('shows empty tools message when no tools selected', () => {
    render(<ConfirmStep config={{ ...BASE_CONFIG, tools: [] }} onConfirm={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByText('未选择工具')).toBeInTheDocument();
  });

  it('calls onConfirm when confirm button is clicked', () => {
    const onConfirm = vi.fn();
    render(<ConfirmStep config={BASE_CONFIG} onConfirm={onConfirm} onBack={vi.fn()} />);
    fireEvent.click(screen.getByTestId('confirm-button'));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it('calls onBack when back button is clicked', () => {
    const onBack = vi.fn();
    render(<ConfirmStep config={BASE_CONFIG} onConfirm={vi.fn()} onBack={onBack} />);
    fireEvent.click(screen.getByTestId('confirm-back'));
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('renders color preview with correct color', () => {
    render(<ConfirmStep config={BASE_CONFIG} onConfirm={vi.fn()} onBack={vi.fn()} />);
    const colorDot = screen.getByTestId('summary-color');
    expect(colorDot).toHaveStyle({ backgroundColor: '#3b82f6' });
  });

  it('applies custom className', () => {
    render(<ConfirmStep config={BASE_CONFIG} onConfirm={vi.fn()} onBack={vi.fn()} className="my-class" />);
    expect(screen.getByTestId('wizard-confirm-step').className).toContain('my-class');
  });
});
