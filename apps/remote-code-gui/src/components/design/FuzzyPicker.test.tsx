import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FuzzyPicker } from './FuzzyPicker';

const ITEMS = ['Apple', 'Banana', 'Cherry', 'Date', 'Elderberry'];

describe('FuzzyPicker', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<FuzzyPicker items={ITEMS} onSelect={vi.fn()} />);
    expect(screen.getByTestId('fuzzy-picker')).toBeInTheDocument();
  });

  it('renders all items initially', () => {
    render(<FuzzyPicker items={ITEMS} onSelect={vi.fn()} />);
    expect(screen.getByTestId('fuzzy-picker-item-Apple')).toBeInTheDocument();
    expect(screen.getByTestId('fuzzy-picker-item-Banana')).toBeInTheDocument();
    expect(screen.getByTestId('fuzzy-picker-item-Cherry')).toBeInTheDocument();
  });

  it('filters items based on query', () => {
    render(<FuzzyPicker items={ITEMS} onSelect={vi.fn()} />);
    fireEvent.change(screen.getByTestId('fuzzy-picker-input'), { target: { value: 'app' } });
    expect(screen.getByTestId('fuzzy-picker-item-Apple')).toBeInTheDocument();
    expect(screen.queryByTestId('fuzzy-picker-item-Banana')).not.toBeInTheDocument();
  });

  it('shows empty state when no matches', () => {
    render(<FuzzyPicker items={ITEMS} onSelect={vi.fn()} />);
    fireEvent.change(screen.getByTestId('fuzzy-picker-input'), { target: { value: 'zzz' } });
    expect(screen.getByTestId('fuzzy-picker-empty')).toBeInTheDocument();
  });

  it('calls onSelect when item is clicked', () => {
    const onSelect = vi.fn();
    render(<FuzzyPicker items={ITEMS} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('fuzzy-picker-item-Cherry'));
    expect(onSelect).toHaveBeenCalledWith('Cherry');
  });

  it('clears input after selection', () => {
    render(<FuzzyPicker items={ITEMS} onSelect={vi.fn()} />);
    fireEvent.change(screen.getByTestId('fuzzy-picker-input'), { target: { value: 'App' } });
    fireEvent.click(screen.getByTestId('fuzzy-picker-item-Apple'));
    const input = screen.getByTestId('fuzzy-picker-input') as HTMLInputElement;
    expect(input.value).toBe('');
  });

  it('renders with custom placeholder', () => {
    render(<FuzzyPicker items={ITEMS} onSelect={vi.fn()} placeholder="搜索水果..." />);
    const input = screen.getByTestId('fuzzy-picker-input') as HTMLInputElement;
    expect(input.placeholder).toBe('搜索水果...');
  });

  it('applies custom className', () => {
    render(<FuzzyPicker items={ITEMS} onSelect={vi.fn()} className="custom" />);
    expect(screen.getByTestId('fuzzy-picker').className).toContain('custom');
  });
});
