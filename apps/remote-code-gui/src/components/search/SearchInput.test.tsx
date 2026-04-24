import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SearchInput } from './SearchInput';

describe('SearchInput', () => {
  afterEach(cleanup);

  it('renders input with placeholder', () => {
    render(<SearchInput value="" onChange={vi.fn()} placeholder="搜索消息..." />);
    expect(screen.getByTestId('search-input')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('搜索消息...')).toBeInTheDocument();
  });

  it('displays the current value', () => {
    render(<SearchInput value="hello" onChange={vi.fn()} />);
    expect(screen.getByTestId('search-input')).toHaveValue('hello');
  });

  it('calls onChange when typing', () => {
    const onChange = vi.fn();
    render(<SearchInput value="" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('search-input'), {
      target: { value: 'test' },
    });
    expect(onChange).toHaveBeenCalledWith('test');
  });

  it('shows clear button when value is non-empty', () => {
    render(<SearchInput value="abc" onChange={vi.fn()} />);
    expect(screen.getByTestId('search-clear')).toBeInTheDocument();
  });

  it('hides clear button when value is empty', () => {
    render(<SearchInput value="" onChange={vi.fn()} />);
    expect(screen.queryByTestId('search-clear')).not.toBeInTheDocument();
  });

  it('clears value when clear button is clicked', () => {
    const onChange = vi.fn();
    render(<SearchInput value="abc" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('search-clear'));
    expect(onChange).toHaveBeenCalledWith('');
  });

  it('calls onKeyDown when a key is pressed', () => {
    const onKeyDown = vi.fn();
    render(<SearchInput value="" onChange={vi.fn()} onKeyDown={onKeyDown} />);
    fireEvent.keyDown(screen.getByTestId('search-input'), { key: 'Enter' });
    expect(onKeyDown).toHaveBeenCalledTimes(1);
  });
});
