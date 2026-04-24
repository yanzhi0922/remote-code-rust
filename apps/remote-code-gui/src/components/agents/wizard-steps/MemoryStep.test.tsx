import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MemoryStep } from './MemoryStep';

describe('MemoryStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<MemoryStep enabled={false} onToggle={vi.fn()} />);
    expect(screen.getByTestId('wizard-memory-step')).toBeInTheDocument();
  });

  it('renders toggle button', () => {
    render(<MemoryStep enabled={false} onToggle={vi.fn()} />);
    expect(screen.getByTestId('memory-toggle')).toBeInTheDocument();
  });

  it('calls onToggle when toggle is clicked', () => {
    const onToggle = vi.fn();
    render(<MemoryStep enabled={false} onToggle={onToggle} />);
    fireEvent.click(screen.getByTestId('memory-toggle'));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('shows memory settings when enabled', () => {
    render(<MemoryStep enabled={true} onToggle={vi.fn()} />);
    expect(screen.getByTestId('memory-settings')).toBeInTheDocument();
  });

  it('hides memory settings when disabled', () => {
    render(<MemoryStep enabled={false} onToggle={vi.fn()} />);
    expect(screen.queryByTestId('memory-settings')).not.toBeInTheDocument();
  });

  it('renders max entries input with default value', () => {
    render(<MemoryStep enabled={true} onToggle={vi.fn()} maxEntries={50} />);
    const input = screen.getByTestId('max-entries-input') as HTMLInputElement;
    expect(input.value).toBe('50');
  });

  it('calls onMaxEntriesChange when value changes', () => {
    const onMaxEntriesChange = vi.fn();
    render(<MemoryStep enabled={true} onToggle={vi.fn()} onMaxEntriesChange={onMaxEntriesChange} />);
    fireEvent.change(screen.getByTestId('max-entries-input'), { target: { value: '200' } });
    expect(onMaxEntriesChange).toHaveBeenCalledWith(200);
  });

  it('applies custom className', () => {
    render(<MemoryStep enabled={false} onToggle={vi.fn()} className="test-cls" />);
    expect(screen.getByTestId('wizard-memory-step').className).toContain('test-cls');
  });
});
