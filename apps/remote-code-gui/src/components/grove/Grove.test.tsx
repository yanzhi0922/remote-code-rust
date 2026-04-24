import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Grove } from './Grove';

afterEach(() => {
  cleanup();
});

describe('Grove', () => {
  const defaultConfig = {
    enabled: true,
    endpoint: 'https://grove.example.com',
    status: 'connected' as const,
    lastSync: '2024-01-01',
  };

  it('renders grove panel', () => {
    render(<Grove config={defaultConfig} />);
    expect(screen.getByTestId('grove-panel')).toBeInTheDocument();
    expect(screen.getByText('Grove 集成')).toBeInTheDocument();
  });

  it('shows connected status', () => {
    render(<Grove config={defaultConfig} />);
    expect(screen.getByText('已连接')).toBeInTheDocument();
  });

  it('shows disconnected status', () => {
    render(<Grove config={{ ...defaultConfig, status: 'disconnected' }} />);
    expect(screen.getByText('未连接')).toBeInTheDocument();
  });

  it('shows endpoint', () => {
    render(<Grove config={defaultConfig} />);
    expect(screen.getByText(/grove.example.com/)).toBeInTheDocument();
  });

  it('calls onToggle', () => {
    const onToggle = vi.fn();
    render(<Grove config={defaultConfig} onToggle={onToggle} />);
    fireEvent.click(screen.getByTestId('grove-toggle'));
    expect(onToggle).toHaveBeenCalled();
  });

  it('calls onConfigure', () => {
    const onConfigure = vi.fn();
    render(<Grove config={defaultConfig} onConfigure={onConfigure} />);
    fireEvent.click(screen.getByTestId('grove-configure'));
    expect(onConfigure).toHaveBeenCalled();
  });
});
