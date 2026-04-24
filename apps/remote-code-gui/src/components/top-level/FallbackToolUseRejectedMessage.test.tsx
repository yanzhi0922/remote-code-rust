import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { FallbackToolUseRejectedMessage } from './FallbackToolUseRejectedMessage';

afterEach(() => {
  cleanup();
});

describe('FallbackToolUseRejectedMessage', () => {
  it('renders rejection message', () => {
    render(<FallbackToolUseRejectedMessage />);
    expect(screen.getByTestId('fallback-tool-use-rejected')).toBeInTheDocument();
    expect(screen.getByText('操作已被用户中断')).toBeInTheDocument();
  });
});
