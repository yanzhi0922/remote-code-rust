import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Select } from './Select';

afterEach(() => {
  cleanup();
});

const sampleOptions = [
  { value: 'a', label: 'Apple' },
  { value: 'b', label: 'Banana' },
  { value: 'c', label: 'Cherry' },
];

const groupedOptions = [
  { value: 'js', label: 'JavaScript', group: 'Web' },
  { value: 'ts', label: 'TypeScript', group: 'Web' },
  { value: 'py', label: 'Python', group: 'Data' },
];

describe('Select', () => {
  it('renders trigger with selected option label', () => {
    render(<Select options={sampleOptions} value="a" onChange={vi.fn()} />);
    expect(screen.getByTestId('select-trigger')).toHaveTextContent('Apple');
  });

  it('renders placeholder when no value matches', () => {
    render(
      <Select options={sampleOptions} value="" onChange={vi.fn()} placeholder="Pick one" />,
    );
    expect(screen.getByTestId('select-trigger')).toHaveTextContent('Pick one');
  });

  it('opens dropdown on trigger click', () => {
    render(<Select options={sampleOptions} value="a" onChange={vi.fn()} />);
    fireEvent.click(screen.getByTestId('select-trigger'));
    expect(screen.getByTestId('select-dropdown')).toBeInTheDocument();
  });

  it('calls onChange when option is selected', () => {
    const onChange = vi.fn();
    render(<Select options={sampleOptions} value="a" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('select-trigger'));
    fireEvent.click(screen.getByTestId('select-option-b'));
    expect(onChange).toHaveBeenCalledWith('b');
  });

  it('highlights selected option', () => {
    render(<Select options={sampleOptions} value="a" onChange={vi.fn()} />);
    fireEvent.click(screen.getByTestId('select-trigger'));
    const option = screen.getByTestId('select-option-a');
    expect(option.className).toContain('bg-slate-100');
    expect(option.className).toContain('font-medium');
  });

  it('renders grouped options', () => {
    render(<Select options={groupedOptions} value="js" onChange={vi.fn()} />);
    fireEvent.click(screen.getByTestId('select-trigger'));
    expect(screen.getByTestId('select-group-Web')).toHaveTextContent('Web');
    expect(screen.getByTestId('select-group-Data')).toHaveTextContent('Data');
  });

  it('renders search input when searchable', () => {
    render(
      <Select options={sampleOptions} value="a" onChange={vi.fn()} searchable />,
    );
    fireEvent.click(screen.getByTestId('select-trigger'));
    expect(screen.getByTestId('select-search')).toBeInTheDocument();
  });

  it('filters options by search text', () => {
    render(
      <Select options={sampleOptions} value="" onChange={vi.fn()} searchable />,
    );
    fireEvent.click(screen.getByTestId('select-trigger'));
    fireEvent.change(screen.getByTestId('select-search'), { target: { value: 'ban' } });
    expect(screen.getByTestId('select-option-b')).toBeInTheDocument();
    expect(screen.queryByTestId('select-option-a')).not.toBeInTheDocument();
    expect(screen.queryByTestId('select-option-c')).not.toBeInTheDocument();
  });

  it('shows empty state when no results match', () => {
    render(
      <Select options={sampleOptions} value="" onChange={vi.fn()} searchable />,
    );
    fireEvent.click(screen.getByTestId('select-trigger'));
    fireEvent.change(screen.getByTestId('select-search'), { target: { value: 'zzz' } });
    expect(screen.getByTestId('select-empty')).toHaveTextContent('No results');
  });

  it('closes dropdown after selecting an option', () => {
    render(<Select options={sampleOptions} value="a" onChange={vi.fn()} />);
    fireEvent.click(screen.getByTestId('select-trigger'));
    expect(screen.getByTestId('select-dropdown')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('select-option-b'));
    expect(screen.queryByTestId('select-dropdown')).not.toBeInTheDocument();
  });

  it('merges custom className', () => {
    render(
      <Select options={sampleOptions} value="a" onChange={vi.fn()} className="my-select" />,
    );
    expect(screen.getByTestId('select').className).toContain('my-select');
  });
});
