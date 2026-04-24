import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SandboxPromptFooterHint } from './SandboxPromptFooterHint';

afterEach(() => {
  cleanup();
});

describe('SandboxPromptFooterHint', () => {
  it('renders nothing when disabled', () => {
    const { container } = render(<SandboxPromptFooterHint sandboxEnabled={false} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders hint when enabled', () => {
    render(<SandboxPromptFooterHint sandboxEnabled={true} />);
    expect(screen.getByTestId('sandbox-prompt-footer-hint')).toBeInTheDocument();
    expect(screen.getByText(/沙箱已启用/)).toBeInTheDocument();
  });

  it('shows sandbox type', () => {
    render(<SandboxPromptFooterHint sandboxEnabled={true} sandboxType="docker" />);
    expect(screen.getByText(/docker/)).toBeInTheDocument();
  });
});
