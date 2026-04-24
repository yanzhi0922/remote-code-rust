import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { McpServerDialogCopy } from './McpServerDialogCopy';

afterEach(() => {
  cleanup();
});

describe('McpServerDialogCopy', () => {
  it('renders copy button', () => {
    render(<McpServerDialogCopy text="test" />);
    expect(screen.getByTestId('mcp-server-dialog-copy')).toBeInTheDocument();
    expect(screen.getByText('复制')).toBeInTheDocument();
  });
});
