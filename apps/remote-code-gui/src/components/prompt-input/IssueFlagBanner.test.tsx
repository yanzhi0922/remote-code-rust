import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { IssueFlagBanner } from './IssueFlagBanner';

afterEach(() => {
  cleanup();
});

describe('IssueFlagBanner', () => {
  it('renders issue text', () => {
    render(<IssueFlagBanner issue="检测到问题" />);
    expect(screen.getByTestId('issue-flag-banner')).toBeInTheDocument();
    expect(screen.getByText('检测到问题')).toBeInTheDocument();
  });

  it('calls onDismiss', () => {
    const onDismiss = vi.fn();
    render(<IssueFlagBanner issue="test" onDismiss={onDismiss} />);
    fireEvent.click(screen.getByTestId('issue-flag-dismiss'));
    expect(onDismiss).toHaveBeenCalled();
  });
});
