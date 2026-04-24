import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AddPermissionRules } from './AddPermissionRules';

describe('AddPermissionRules', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<AddPermissionRules onAdd={vi.fn()} />);
    expect(screen.getByTestId('add-permission-rules')).toBeInTheDocument();
  });

  it('has input field', () => {
    render(<AddPermissionRules onAdd={vi.fn()} />);
    expect(screen.getByTestId('permission-rule-input')).toBeInTheDocument();
  });

  it('calls onAdd when form submitted with value', () => {
    const onAdd = vi.fn();
    render(<AddPermissionRules onAdd={onAdd} />);
    fireEvent.change(screen.getByTestId('permission-rule-input'), {
      target: { value: 'allow Read(*)' },
    });
    fireEvent.click(screen.getByTestId('add-rule-btn'));
    expect(onAdd).toHaveBeenCalledWith('allow Read(*)');
  });

  it('does not call onAdd with empty input', () => {
    const onAdd = vi.fn();
    render(<AddPermissionRules onAdd={onAdd} />);
    fireEvent.click(screen.getByTestId('add-rule-btn'));
    expect(onAdd).not.toHaveBeenCalled();
  });

  it('clears input after submit', () => {
    render(<AddPermissionRules onAdd={vi.fn()} />);
    const input = screen.getByTestId('permission-rule-input') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'rule' } });
    fireEvent.click(screen.getByTestId('add-rule-btn'));
    expect(input.value).toBe('');
  });
});
