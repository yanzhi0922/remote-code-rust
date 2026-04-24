import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ValidationErrorsList } from './ValidationErrorsList';

afterEach(() => {
  cleanup();
});

describe('ValidationErrorsList', () => {
  it('renders nothing when no errors', () => {
    const { container } = render(<ValidationErrorsList errors={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders errors', () => {
    const errors = [
      { field: 'name', message: '必填' },
      { field: 'email', message: '格式错误' },
    ];
    render(<ValidationErrorsList errors={errors} />);
    expect(screen.getByTestId('validation-errors-list')).toBeInTheDocument();
    expect(screen.getByText('name')).toBeInTheDocument();
    expect(screen.getByText(/必填/)).toBeInTheDocument();
  });
});
