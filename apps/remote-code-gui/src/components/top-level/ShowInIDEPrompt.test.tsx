import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ShowInIDEPrompt } from './ShowInIDEPrompt';

afterEach(() => {
  cleanup();
});

describe('ShowInIDEPrompt', () => {
  it('renders file path', () => {
    render(<ShowInIDEPrompt filePath="src/app.tsx" />);
    expect(screen.getByTestId('show-in-ide-prompt')).toBeInTheDocument();
    expect(screen.getByText(/src\/app.tsx/)).toBeInTheDocument();
  });

  it('shows line number', () => {
    render(<ShowInIDEPrompt filePath="src/app.tsx" line={42} />);
    expect(screen.getByText(/42/)).toBeInTheDocument();
  });

  it('calls onOpen', () => {
    const onOpen = vi.fn();
    render(<ShowInIDEPrompt filePath="src/app.tsx" onOpen={onOpen} />);
    fireEvent.click(screen.getByTestId('show-in-ide-prompt'));
    expect(onOpen).toHaveBeenCalled();
  });
});
