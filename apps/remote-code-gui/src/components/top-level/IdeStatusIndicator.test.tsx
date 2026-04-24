import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { IdeStatusIndicator } from './IdeStatusIndicator';

afterEach(() => {
  cleanup();
});

describe('IdeStatusIndicator', () => {
  it('returns null when status is null', () => {
    const { container } = render(<IdeStatusIndicator status={null} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows connected status', () => {
    render(<IdeStatusIndicator status="connected" />);
    expect(screen.getByText('IDE已连接')).toBeInTheDocument();
  });

  it('shows disconnected status', () => {
    render(<IdeStatusIndicator status="disconnected" />);
    expect(screen.getByText('IDE未连接')).toBeInTheDocument();
  });

  it('shows file path when connected', () => {
    render(<IdeStatusIndicator status="connected" filePath="/src/app.tsx" />);
    expect(screen.getByText('app.tsx')).toBeInTheDocument();
  });

  it('shows selection count', () => {
    render(<IdeStatusIndicator status="connected" selectedLineCount={5} />);
    expect(screen.getByText(/5/)).toBeInTheDocument();
  });
});
