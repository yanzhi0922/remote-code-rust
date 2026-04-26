import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('./Sidebar', () => ({
  Sidebar: () => <nav data-testid="sidebar">Sidebar</nav>,
}));
vi.mock('./Header', () => ({
  Header: () => <header data-testid="header">Header</header>,
}));

import { Layout } from './Layout';

afterEach(() => { cleanup(); });

describe('Layout', () => {
  it('renders sidebar, header, and children', () => {
    render(
      <Layout>
        <div data-testid="content">Main content</div>
      </Layout>,
    );
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
    expect(screen.getByTestId('header')).toBeInTheDocument();
    expect(screen.getByTestId('content')).toBeInTheDocument();
  });

  it('applies full-screen flex layout', () => {
    const { container } = render(<Layout><div /></Layout>);
    const outer = container.firstElementChild as HTMLElement;
    expect(outer.classList.contains('flex')).toBe(true);
    expect(outer.classList.contains('h-screen')).toBe(true);
  });
});
