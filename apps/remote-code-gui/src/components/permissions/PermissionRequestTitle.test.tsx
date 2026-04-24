import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PermissionRequestTitle } from './PermissionRequestTitle';

describe('PermissionRequestTitle', () => {
  afterEach(cleanup);

  it('renders the title text', () => {
    render(<PermissionRequestTitle title="Bash Command" />);
    expect(screen.getByText('Bash Command')).toBeInTheDocument();
  });

  it('renders a ShieldAlert icon', () => {
    const { container } = render(<PermissionRequestTitle title="Test" />);
    const svg = container.querySelector('svg');
    expect(svg).toBeInTheDocument();
  });

  it('renders subtitle when provided', () => {
    render(<PermissionRequestTitle title="Test" subtitle="A subtitle" />);
    expect(screen.getByText('A subtitle')).toBeInTheDocument();
  });

  it('does not render subtitle when not provided', () => {
    const { container } = render(<PermissionRequestTitle title="Test" />);
    const subtitleEl = container.querySelector('.text-slate-500');
    expect(subtitleEl).toBeNull();
  });

  it('renders worker badge when provided', () => {
    render(
      <PermissionRequestTitle
        title="Test"
        workerBadge={{ name: 'worker-1', color: '#6366f1' }}
      />,
    );
    expect(screen.getByText('@worker-1')).toBeInTheDocument();
  });

  it('does not render worker badge when not provided', () => {
    render(<PermissionRequestTitle title="Test" />);
    expect(screen.queryByText(/@/)).toBeNull();
  });

  it('renders ReactNode subtitle', () => {
    render(
      <PermissionRequestTitle
        title="Test"
        subtitle={<span data-testid="custom-subtitle">Custom</span>}
      />,
    );
    expect(screen.getByTestId('custom-subtitle')).toBeInTheDocument();
  });

  it('applies custom color to icon', () => {
    const { container } = render(<PermissionRequestTitle title="Test" color="#00ff00" />);
    const svg = container.querySelector('svg');
    expect(svg?.getAttribute('style')).toContain('rgb(0, 255, 0)');
  });
});
