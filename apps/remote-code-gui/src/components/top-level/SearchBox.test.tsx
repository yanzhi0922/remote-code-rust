import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SearchBox } from './SearchBox';

afterEach(() => {
  cleanup();
});

describe('SearchBox', () => {
  it('renders search input', () => {
    render(<SearchBox />);
    expect(screen.getByTestId('search-box')).toBeInTheDocument();
    expect(screen.getByTestId('search-box-input')).toBeInTheDocument();
  });

  it('calls onChange when typing', () => {
    const onChange = vi.fn();
    render(<SearchBox onChange={onChange} />);
    fireEvent.change(screen.getByTestId('search-box-input'), { target: { value: 'test' } });
    expect(onChange).toHaveBeenCalledWith('test');
  });

  it('calls onSearch on Enter', () => {
    const onSearch = vi.fn();
    render(<SearchBox onSearch={onSearch} />);
    fireEvent.change(screen.getByTestId('search-box-input'), { target: { value: 'query' } });
    fireEvent.keyDown(screen.getByTestId('search-box-input'), { key: 'Enter' });
    expect(onSearch).toHaveBeenCalledWith('query');
  });

  it('shows clear button when value present', () => {
    render(<SearchBox value="test" />);
    expect(screen.getByTestId('search-box-clear')).toBeInTheDocument();
  });
});
