import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('./Sidebar', () => ({
  Sidebar: () => <nav data-testid="sidebar">Sidebar</nav>,
}));
vi.mock('./StatusBar', () => ({
  StatusBar: () => <footer data-testid="status-bar">Status</footer>,
}));

import { Layout } from './Layout';

afterEach(() => { cleanup(); });

describe('Layout', () => {
  it('renders sidebar, status bar, and children', () => {
    render(
      <Layout>
        <div data-testid="content">Main content</div>
      </Layout>,
    );
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
    expect(screen.getByTestId('status-bar')).toBeInTheDocument();
    expect(screen.getByTestId('content')).toBeInTheDocument();
  });

  it('applies full-screen flex layout', () => {
    const { container } = render(<Layout><div /></Layout>);
    const outer = container.firstElementChild as HTMLElement;
    expect(outer.classList.contains('flex')).toBe(true);
    expect(outer.classList.contains('h-dvh')).toBe(true);
  });

  it('renders the desktop workbench regions', () => {
    render(
      <Layout>
        <div data-testid="content">Main content</div>
      </Layout>,
    );

    expect(screen.getByRole('navigation', { name: 'Workbench activity bar' })).toBeInTheDocument();
    expect(screen.getByRole('main', { name: 'Agent conversation workbench' })).toBeInTheDocument();
    expect(screen.getByRole('complementary', { name: 'Environment information' })).toBeInTheDocument();
  });
});
