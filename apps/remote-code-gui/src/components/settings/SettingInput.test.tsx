import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SettingInput } from './SettingInput';

describe('SettingInput', () => {
  afterEach(cleanup);

  it('renders label text', () => {
    render(<SettingInput label="Timeout" value={30000} onChange={vi.fn()} />);
    expect(screen.getByText('Timeout')).toBeInTheDocument();
  });

  it('renders description when provided', () => {
    render(<SettingInput label="Timeout" value={30000} onChange={vi.fn()} description="Request timeout in ms" />);
    expect(screen.getByText('Request timeout in ms')).toBeInTheDocument();
  });

  it('renders input with correct value', () => {
    render(<SettingInput label="Base URL" value="https://api.example.com" onChange={vi.fn()} />);
    expect(screen.getByTestId('setting-input')).toHaveValue('https://api.example.com');
  });

  it('calls onChange when input value changes', () => {
    const onChange = vi.fn();
    render(<SettingInput label="Base URL" value="" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('setting-input'), { target: { value: 'https://new.url' } });
    expect(onChange).toHaveBeenCalledWith('https://new.url');
  });

  it('renders password input type by default for password type', () => {
    render(<SettingInput label="API Key" value="secret" onChange={vi.fn()} type="password" />);
    expect(screen.getByTestId('setting-input')).toHaveAttribute('type', 'password');
  });

  it('shows password toggle button for password type', () => {
    render(<SettingInput label="API Key" value="secret" onChange={vi.fn()} type="password" />);
    expect(screen.getByTestId('toggle-password')).toBeInTheDocument();
  });

  it('toggles password visibility when eye icon is clicked', () => {
    render(<SettingInput label="API Key" value="secret" onChange={vi.fn()} type="password" />);
    const input = screen.getByTestId('setting-input');
    expect(input).toHaveAttribute('type', 'password');
    fireEvent.click(screen.getByTestId('toggle-password'));
    expect(input).toHaveAttribute('type', 'text');
  });

  it('does not show password toggle for text type', () => {
    render(<SettingInput label="Name" value="test" onChange={vi.fn()} type="text" />);
    expect(screen.queryByTestId('toggle-password')).toBeNull();
  });
});
