import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MessageTimestamp } from './MessageTimestamp';

afterEach(() => {
  cleanup();
});

describe('MessageTimestamp', () => {
  it('renders timestamp', () => {
    render(<MessageTimestamp timestamp="10:30:00" />);
    expect(screen.getByTestId('message-timestamp')).toBeInTheDocument();
    expect(screen.getByText('10:30:00')).toBeInTheDocument();
  });
});
