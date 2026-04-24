import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Passes } from './Passes';

afterEach(() => {
  cleanup();
});

describe('Passes', () => {
  const defaultPasses = [
    {
      id: 'pass-1',
      name: 'Pro Plan',
      status: 'active' as const,
      expiresAt: '2025-12-31',
      features: ['无限对话', '高级模型'],
    },
    {
      id: 'pass-2',
      name: 'Basic Plan',
      status: 'expired' as const,
      expiresAt: '2024-01-01',
      features: [],
    },
  ];

  it('renders passes list', () => {
    render(<Passes passes={defaultPasses} />);
    expect(screen.getByTestId('passes-panel')).toBeInTheDocument();
    expect(screen.getByText('Pro Plan')).toBeInTheDocument();
    expect(screen.getByText('Basic Plan')).toBeInTheDocument();
  });

  it('shows empty state', () => {
    render(<Passes passes={[]} />);
    expect(screen.getByTestId('passes-empty')).toHaveTextContent('暂无订阅');
  });

  it('shows status badges', () => {
    render(<Passes passes={defaultPasses} />);
    expect(screen.getByText('活跃')).toBeInTheDocument();
    expect(screen.getByText('已过期')).toBeInTheDocument();
  });

  it('calls onSubscribe', () => {
    const onSubscribe = vi.fn();
    render(<Passes passes={[]} onSubscribe={onSubscribe} />);
    fireEvent.click(screen.getByTestId('passes-subscribe'));
    expect(onSubscribe).toHaveBeenCalled();
  });

  it('calls onCancel for active pass', () => {
    const onCancel = vi.fn();
    render(<Passes passes={defaultPasses} onCancel={onCancel} />);
    fireEvent.click(screen.getByTestId('pass-cancel-pass-1'));
    expect(onCancel).toHaveBeenCalledWith('pass-1');
  });

  it('does not show cancel for expired pass', () => {
    render(<Passes passes={defaultPasses} onCancel={() => {}} />);
    expect(screen.queryByTestId('pass-cancel-pass-2')).not.toBeInTheDocument();
  });
});
