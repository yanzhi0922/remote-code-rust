import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MessageRow } from './MessageRow';

afterEach(() => {
  cleanup();
});

describe('MessageRow', () => {
  it('renders children', () => {
    render(<MessageRow role="user">Hello</MessageRow>);
    expect(screen.getByTestId('message-row-user')).toBeInTheDocument();
    expect(screen.getByText('Hello')).toBeInTheDocument();
  });

  it('renders assistant role', () => {
    render(<MessageRow role="assistant">Response</MessageRow>);
    expect(screen.getByTestId('message-row-assistant')).toBeInTheDocument();
  });

  it('renders system role', () => {
    render(<MessageRow role="system">System</MessageRow>);
    expect(screen.getByTestId('message-row-system')).toBeInTheDocument();
  });
});
