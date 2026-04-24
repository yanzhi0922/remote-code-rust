import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PermissionPrompt } from './PermissionPrompt';

const options = [
  { label: 'Allow', value: 'allow', description: 'Allow this action' },
  { label: 'Deny', value: 'deny' },
];

describe('PermissionPrompt', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<PermissionPrompt title="Test" options={options} onSelect={vi.fn()} />);
    expect(screen.getByTestId('permission-prompt')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<PermissionPrompt title="Choose action" options={options} onSelect={vi.fn()} />);
    expect(screen.getByText('Choose action')).toBeInTheDocument();
  });

  it('shows description', () => {
    render(<PermissionPrompt title="T" description="Pick one" options={options} onSelect={vi.fn()} />);
    expect(screen.getByText('Pick one')).toBeInTheDocument();
  });

  it('calls onSelect with value', () => {
    const onSelect = vi.fn();
    render(<PermissionPrompt title="T" options={options} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('permission-option-allow'));
    expect(onSelect).toHaveBeenCalledWith('allow');
  });
});
