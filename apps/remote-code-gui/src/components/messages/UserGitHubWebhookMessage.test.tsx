import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserGitHubWebhookMessage } from './UserGitHubWebhookMessage';

describe('UserGitHubWebhookMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<UserGitHubWebhookMessage event="push" />);
    expect(screen.getByTestId('user-github-webhook-message')).toBeInTheDocument();
  });

  it('displays event name', () => {
    render(<UserGitHubWebhookMessage event="pull_request" />);
    expect(screen.getByText('pull_request')).toBeInTheDocument();
  });

  it('shows action when provided', () => {
    render(<UserGitHubWebhookMessage event="push" action="opened" />);
    expect(screen.getByText(/opened/)).toBeInTheDocument();
  });

  it('shows repository and sender', () => {
    render(
      <UserGitHubWebhookMessage
        event="push"
        repository="org/repo"
        sender="alice"
      />,
    );
    expect(screen.getByText('org/repo')).toBeInTheDocument();
    expect(screen.getByText(/alice/)).toBeInTheDocument();
  });

  it('shows payload when provided', () => {
    render(
      <UserGitHubWebhookMessage event="push" payload='{"ref":"main"}' />,
    );
    expect(screen.getByText('{"ref":"main"}')).toBeInTheDocument();
  });
});
