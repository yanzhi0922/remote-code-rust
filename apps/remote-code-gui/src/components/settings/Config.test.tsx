import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Config } from './Config';

afterEach(() => {
  cleanup();
});

describe('Config', () => {
  it('renders config panel', () => {
    render(<Config config={{ name: 'test' }} />);
    expect(screen.getByTestId('config-panel')).toBeInTheDocument();
  });

  it('shows raw JSON when no schema', () => {
    render(<Config config={{ name: 'test' }} />);
    expect(screen.getByTestId('config-raw')).toHaveTextContent('test');
  });

  it('renders schema fields', () => {
    const schema = [
      { key: 'name', label: '名称', type: 'string' as const },
    ];
    render(<Config config={{ name: 'hello' }} schema={schema} />);
    expect(screen.getByTestId('config-field-name')).toHaveValue('hello');
  });

  it('calls onSave', () => {
    const onSave = vi.fn();
    const schema = [{ key: 'name', label: '名称', type: 'string' as const }];
    render(<Config config={{ name: 'test' }} schema={schema} onSave={onSave} />);
    fireEvent.change(screen.getByTestId('config-field-name'), { target: { value: 'updated' } });
    fireEvent.click(screen.getByTestId('config-save'));
    expect(onSave).toHaveBeenCalled();
  });

  it('calls onReset', () => {
    const onReset = vi.fn();
    render(<Config config={{ name: 'test' }} onReset={onReset} />);
    fireEvent.click(screen.getByTestId('config-reset'));
    expect(onReset).toHaveBeenCalled();
  });
});
