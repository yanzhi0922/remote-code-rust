import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TypeStep } from './TypeStep';

describe('TypeStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<TypeStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('wizard-type-step')).toBeInTheDocument();
  });

  it('renders all three type options', () => {
    render(<TypeStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('type-option-subagent')).toBeInTheDocument();
    expect(screen.getByTestId('type-option-worker')).toBeInTheDocument();
    expect(screen.getByTestId('type-option-coordinator')).toBeInTheDocument();
  });

  it('shows selected state for current value', () => {
    render(<TypeStep value="worker" onChange={vi.fn()} />);
    const workerBtn = screen.getByTestId('type-option-worker');
    expect(workerBtn.className).toContain('border-blue-500');
  });

  it('does not show selected state for non-selected types', () => {
    render(<TypeStep value="subagent" onChange={vi.fn()} />);
    const workerBtn = screen.getByTestId('type-option-worker');
    expect(workerBtn.className).not.toContain('border-blue-500');
  });

  it('calls onChange when a type option is clicked', () => {
    const onChange = vi.fn();
    render(<TypeStep value="" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('type-option-coordinator'));
    expect(onChange).toHaveBeenCalledWith('coordinator');
  });

  it('applies custom className', () => {
    render(<TypeStep value="" onChange={vi.fn()} className="my-custom" />);
    expect(screen.getByTestId('wizard-type-step').className).toContain('my-custom');
  });

  it('renders type labels', () => {
    render(<TypeStep value="" onChange={vi.fn()} />);
    expect(screen.getByText('子代理')).toBeInTheDocument();
    expect(screen.getByText('工作节点')).toBeInTheDocument();
    expect(screen.getByText('协调器')).toBeInTheDocument();
  });

  it('renders section heading', () => {
    render(<TypeStep value="" onChange={vi.fn()} />);
    expect(screen.getByText('选择 Agent 类型')).toBeInTheDocument();
  });
});
