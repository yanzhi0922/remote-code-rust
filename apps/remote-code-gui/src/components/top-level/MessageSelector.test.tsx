import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MessageSelector } from './MessageSelector';

afterEach(() => {
  cleanup();
});

describe('MessageSelector', () => {
  it('renders unselected state', () => {
    render(<MessageSelector selected={false} messageId="m1" onToggle={() => {}} />);
    expect(screen.getByTestId('message-selector-m1')).toBeInTheDocument();
  });

  it('renders selected state', () => {
    render(<MessageSelector selected={true} messageId="m1" onToggle={() => {}} />);
    expect(screen.getByTestId('message-selector-m1')).toBeInTheDocument();
  });

  it('calls onToggle', () => {
    const onToggle = vi.fn();
    render(<MessageSelector selected={false} messageId="m1" onToggle={onToggle} />);
    fireEvent.click(screen.getByTestId('message-selector-m1'));
    expect(onToggle).toHaveBeenCalledWith('m1');
  });
});
