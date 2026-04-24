import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PluginHintMenu } from './PluginHintMenu';

afterEach(() => {
  cleanup();
});

describe('PluginHintMenu', () => {
  const defaultProps = {
    pluginName: 'TestPlugin',
    pluginDescription: 'A test plugin',
    marketplaceName: 'npm',
    sourceCommand: '/test',
    onResponse: vi.fn(),
  };

  it('renders plugin info', () => {
    render(<PluginHintMenu {...defaultProps} />);
    expect(screen.getByTestId('plugin-hint-menu')).toBeInTheDocument();
    expect(screen.getByText('TestPlugin')).toBeInTheDocument();
    expect(screen.getByText('npm')).toBeInTheDocument();
  });

  it('shows description when provided', () => {
    render(<PluginHintMenu {...defaultProps} />);
    expect(screen.getByText('A test plugin')).toBeInTheDocument();
  });

  it('calls onResponse with yes', () => {
    const onResponse = vi.fn();
    render(<PluginHintMenu {...defaultProps} onResponse={onResponse} />);
    fireEvent.click(screen.getByTestId('plugin-hint-yes'));
    expect(onResponse).toHaveBeenCalledWith('yes');
  });

  it('calls onResponse with no', () => {
    const onResponse = vi.fn();
    render(<PluginHintMenu {...defaultProps} onResponse={onResponse} />);
    fireEvent.click(screen.getByTestId('plugin-hint-no'));
    expect(onResponse).toHaveBeenCalledWith('no');
  });

  it('calls onResponse with disable', () => {
    const onResponse = vi.fn();
    render(<PluginHintMenu {...defaultProps} onResponse={onResponse} />);
    fireEvent.click(screen.getByTestId('plugin-hint-disable'));
    expect(onResponse).toHaveBeenCalledWith('disable');
  });

  it('calls onResponse with no on dismiss', () => {
    const onResponse = vi.fn();
    render(<PluginHintMenu {...defaultProps} onResponse={onResponse} />);
    fireEvent.click(screen.getByTestId('plugin-hint-dismiss'));
    expect(onResponse).toHaveBeenCalledWith('no');
  });
});
