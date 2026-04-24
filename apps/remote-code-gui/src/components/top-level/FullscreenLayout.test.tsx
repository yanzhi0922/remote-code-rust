import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { FullscreenLayout } from './FullscreenLayout';

afterEach(() => {
  cleanup();
});

describe('FullscreenLayout', () => {
  it('renders children', () => {
    render(<FullscreenLayout><div data-testid="child">Content</div></FullscreenLayout>);
    expect(screen.getByTestId('fullscreen-layout')).toBeInTheDocument();
    expect(screen.getByTestId('child')).toBeInTheDocument();
  });

  it('renders header when provided', () => {
    render(<FullscreenLayout header={<div>Header</div>}>Content</FullscreenLayout>);
    expect(screen.getByTestId('fullscreen-header')).toBeInTheDocument();
    expect(screen.getByText('Header')).toBeInTheDocument();
  });

  it('renders footer when provided', () => {
    render(<FullscreenLayout footer={<div>Footer</div>}>Content</FullscreenLayout>);
    expect(screen.getByTestId('fullscreen-footer')).toBeInTheDocument();
    expect(screen.getByText('Footer')).toBeInTheDocument();
  });

  it('does not render header/footer when not provided', () => {
    render(<FullscreenLayout>Content</FullscreenLayout>);
    expect(screen.queryByTestId('fullscreen-header')).not.toBeInTheDocument();
    expect(screen.queryByTestId('fullscreen-footer')).not.toBeInTheDocument();
  });
});
