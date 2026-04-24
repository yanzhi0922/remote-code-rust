import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { RemoteCallout } from './RemoteCallout';

afterEach(() => {
  cleanup();
});

describe('RemoteCallout', () => {
  it('renders remote info', () => {
    render(<RemoteCallout host="ssh://dev.example.com" />);
    expect(screen.getByTestId('remote-callout')).toBeInTheDocument();
    expect(screen.getByText('ssh://dev.example.com')).toBeInTheDocument();
  });

  it('shows environment', () => {
    render(<RemoteCallout host="ssh://dev.example.com" environment="Docker" />);
    expect(screen.getByText('Docker')).toBeInTheDocument();
  });
});
