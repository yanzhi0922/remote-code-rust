import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SelectEventMode } from './SelectEventMode';

describe('SelectEventMode', () => {
  afterEach(cleanup);

  it('renders select element', () => {
    render(<SelectEventMode value="PreToolUse" onChange={vi.fn()} />);
    expect(screen.getByTestId('select-event-mode')).toBeInTheDocument();
  });

  it('displays current value', () => {
    render(<SelectEventMode value="PostToolUse" onChange={vi.fn()} />);
    const select = screen.getByTestId('select-event-mode') as HTMLSelectElement;
    expect(select.value).toBe('PostToolUse');
  });

  it('contains all event options', () => {
    render(<SelectEventMode value="PreToolUse" onChange={vi.fn()} />);
    expect(screen.getByText('PreToolUse')).toBeInTheDocument();
    expect(screen.getByText('PostToolUse')).toBeInTheDocument();
    expect(screen.getByText('Notification')).toBeInTheDocument();
    expect(screen.getByText('Stop')).toBeInTheDocument();
  });

  it('calls onChange when value changes', () => {
    const onChange = vi.fn();
    render(<SelectEventMode value="PreToolUse" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('select-event-mode'), {
      target: { value: 'Stop' },
    });
    expect(onChange).toHaveBeenCalledWith('Stop');
  });

  it('applies custom className', () => {
    render(
      <SelectEventMode value="PreToolUse" onChange={vi.fn()} className="my-cls" />,
    );
    expect(screen.getByTestId('select-event-mode').className).toContain('my-cls');
  });

  it('has focus styles', () => {
    render(<SelectEventMode value="PreToolUse" onChange={vi.fn()} />);
    expect(screen.getByTestId('select-event-mode').className).toContain('focus:ring-blue-500');
  });
});
