import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { General } from './General';

afterEach(() => {
  cleanup();
});

describe('General', () => {
  it('renders help info', () => {
    render(<General />);
    expect(screen.getByTestId('general-help')).toBeInTheDocument();
    expect(screen.getByText('Remote Code GUI')).toBeInTheDocument();
  });

  it('shows default version', () => {
    render(<General />);
    expect(screen.getByText('版本 1.0.0')).toBeInTheDocument();
  });

  it('shows custom version', () => {
    render(<General version="2.0.0" />);
    expect(screen.getByText('版本 2.0.0')).toBeInTheDocument();
  });

  it('shows keyboard shortcuts', () => {
    render(<General />);
    expect(screen.getByText('发送消息')).toBeInTheDocument();
    expect(screen.getByText('换行')).toBeInTheDocument();
  });
});
