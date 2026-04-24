import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AntModelSwitchCallout } from './AntModelSwitchCallout';

afterEach(() => {
  cleanup();
});

describe('AntModelSwitchCallout', () => {
  it('renders model switch info', () => {
    render(<AntModelSwitchCallout fromModel="claude-3" toModel="claude-4" onDismiss={() => {}} />);
    expect(screen.getByTestId('ant-model-switch-callout')).toBeInTheDocument();
    expect(screen.getByText('claude-3')).toBeInTheDocument();
    expect(screen.getByText('claude-4')).toBeInTheDocument();
  });

  it('calls onDismiss', () => {
    const onDismiss = vi.fn();
    render(<AntModelSwitchCallout fromModel="a" toModel="b" onDismiss={onDismiss} />);
    fireEvent.click(screen.getByTestId('ant-model-switch-dismiss'));
    expect(onDismiss).toHaveBeenCalled();
  });
});
