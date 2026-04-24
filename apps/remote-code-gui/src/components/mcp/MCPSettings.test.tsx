import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MCPSettings } from './MCPSettings';

describe('MCPSettings', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<MCPSettings />);
    expect(screen.getByTestId('mcp-settings')).toBeInTheDocument();
  });

  it('shows timeout input', () => {
    render(<MCPSettings defaultTimeout={60} />);
    const input = screen.getByTestId('mcp-timeout-input') as HTMLInputElement;
    expect(input.value).toBe('60');
  });
});
