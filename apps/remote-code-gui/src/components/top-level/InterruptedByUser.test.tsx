import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { InterruptedByUser } from './InterruptedByUser';

afterEach(() => {
  cleanup();
});

describe('InterruptedByUser', () => {
  it('renders interrupted message', () => {
    render(<InterruptedByUser />);
    expect(screen.getByTestId('interrupted-by-user')).toBeInTheDocument();
    expect(screen.getByText('已被用户中断')).toBeInTheDocument();
  });
});
