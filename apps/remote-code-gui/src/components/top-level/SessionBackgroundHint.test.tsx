import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SessionBackgroundHint } from './SessionBackgroundHint';

afterEach(() => {
  cleanup();
});

describe('SessionBackgroundHint', () => {
  it('renders hint', () => {
    render(<SessionBackgroundHint />);
    expect(screen.getByTestId('session-background-hint')).toBeInTheDocument();
    expect(screen.getByText('会话在后台运行')).toBeInTheDocument();
  });
});
