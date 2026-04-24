import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { StatusLine } from './StatusLine';
import { TokenIndicator } from './TokenIndicator';

afterEach(() => {
  cleanup();
});

// ─── StatusLine ─────────────────────────────────────────────────────
describe('StatusLine', () => {
  const baseStatus = {
    provider: 'OpenAI',
    model: 'gpt-4o',
    permissionMode: 'auto',
  };

  it('renders provider, model, and permission mode', () => {
    render(<StatusLine status={baseStatus} />);
    expect(screen.getByTestId('status-provider')).toHaveTextContent('OpenAI');
    expect(screen.getByTestId('status-model')).toHaveTextContent('gpt-4o');
    expect(screen.getByTestId('status-permission')).toHaveTextContent('auto');
  });

  it('renders context usage bar when provided', () => {
    render(<StatusLine status={{ ...baseStatus, contextUsage: { ratio: 0.65 } }} />);
    expect(screen.getByTestId('status-context')).toBeInTheDocument();
    expect(screen.getByText('65%')).toBeInTheDocument();
  });

  it('hides context usage when not provided', () => {
    render(<StatusLine status={baseStatus} />);
    expect(screen.queryByTestId('status-context')).not.toBeInTheDocument();
  });

  it('renders session ID when provided', () => {
    render(<StatusLine status={{ ...baseStatus, sessionId: 'abc123def456' }} />);
    expect(screen.getByTestId('status-session')).toHaveTextContent('abc123de');
  });

  it('hides session ID when not provided', () => {
    render(<StatusLine status={baseStatus} />);
    expect(screen.queryByTestId('status-session')).not.toBeInTheDocument();
  });

  it('applies green color for low context usage', () => {
    render(<StatusLine status={{ ...baseStatus, contextUsage: { ratio: 0.3 } }} />);
    const bar = screen.getByTestId('status-context').querySelector('.rounded-full.transition-all');
    expect(bar!.className).toContain('bg-green-500');
  });

  it('applies red color for high context usage', () => {
    render(<StatusLine status={{ ...baseStatus, contextUsage: { ratio: 0.9 } }} />);
    const bar = screen.getByTestId('status-context').querySelector('.rounded-full.transition-all');
    expect(bar!.className).toContain('bg-red-500');
  });
});

// ─── TokenIndicator ─────────────────────────────────────────────────
describe('TokenIndicator', () => {
  it('renders token counts', () => {
    render(<TokenIndicator usage={{ inputTokens: 1500, outputTokens: 500, totalTokens: 2000 }} />);
    expect(screen.getByTestId('input-tokens')).toHaveTextContent('1.5K');
    expect(screen.getByTestId('output-tokens')).toHaveTextContent('500');
    expect(screen.getByTestId('total-tokens')).toHaveTextContent('2.0K');
  });

  it('formats millions correctly', () => {
    render(<TokenIndicator usage={{ inputTokens: 1_500_000, outputTokens: 500_000, totalTokens: 2_000_000 }} />);
    expect(screen.getByTestId('total-tokens')).toHaveTextContent('2.0M');
  });

  it('shows progress bar when maxTokens provided', () => {
    render(<TokenIndicator usage={{ inputTokens: 500, outputTokens: 500, totalTokens: 1000, maxTokens: 2000 }} />);
    expect(screen.getByTestId('token-progress')).toBeInTheDocument();
  });

  it('hides progress bar when maxTokens not provided', () => {
    render(<TokenIndicator usage={{ inputTokens: 500, outputTokens: 500, totalTokens: 1000 }} />);
    expect(screen.queryByTestId('token-progress')).not.toBeInTheDocument();
  });

  it('applies green color for <50% usage', () => {
    const { container } = render(
      <TokenIndicator usage={{ inputTokens: 400, outputTokens: 100, totalTokens: 500, maxTokens: 2000 }} />,
    );
    const bar = container.querySelector('[data-testid="token-progress"] > div');
    expect(bar!.className).toContain('bg-green-500');
  });

  it('applies yellow color for 50-80% usage', () => {
    const { container } = render(
      <TokenIndicator usage={{ inputTokens: 1000, outputTokens: 500, totalTokens: 1500, maxTokens: 2000 }} />,
    );
    const bar = container.querySelector('[data-testid="token-progress"] > div');
    expect(bar!.className).toContain('bg-yellow-500');
  });

  it('applies red color for >80% usage', () => {
    const { container } = render(
      <TokenIndicator usage={{ inputTokens: 1700, outputTokens: 300, totalTokens: 2000, maxTokens: 2000 }} />,
    );
    const bar = container.querySelector('[data-testid="token-progress"] > div');
    expect(bar!.className).toContain('bg-red-500');
  });
});
