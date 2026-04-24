import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { RemoteSessionProgress } from './RemoteSessionProgress';

describe('RemoteSessionProgress', () => {
  afterEach(cleanup);

  it('renders server URL', () => {
    render(<RemoteSessionProgress serverUrl="ws://localhost:8080" />);
    expect(screen.getByTestId('remote-session-progress')).toHaveTextContent('ws://localhost:8080');
  });

  it('shows progress bar when bytes provided', () => {
    const { container } = render(
      <RemoteSessionProgress
        serverUrl="ws://host"
        bytesTransferred={512}
        totalBytes={1024}
      />,
    );
    const bar = container.querySelector('[style*="width: 50%"]');
    expect(bar).toBeInTheDocument();
  });

  it('shows percentage when progress provided', () => {
    render(
      <RemoteSessionProgress
        serverUrl="ws://host"
        bytesTransferred={256}
        totalBytes={1024}
      />,
    );
    expect(screen.getByText('25%')).toBeInTheDocument();
  });

  it('shows byte count formatted', () => {
    render(
      <RemoteSessionProgress
        serverUrl="ws://host"
        bytesTransferred={1536}
        totalBytes={10240}
      />,
    );
    expect(screen.getByText('1.5 KB')).toBeInTheDocument();
  });

  it('hides progress when no bytes provided', () => {
    render(<RemoteSessionProgress serverUrl="ws://host" />);
    expect(screen.queryByText('%')).toBeNull();
  });

  it('applies custom className', () => {
    render(<RemoteSessionProgress serverUrl="ws://host" className="my-cls" />);
    expect(screen.getByTestId('remote-session-progress').className).toContain('my-cls');
  });

  it('clamps progress to 100%', () => {
    render(
      <RemoteSessionProgress
        serverUrl="ws://host"
        bytesTransferred={2000}
        totalBytes={1000}
      />,
    );
    expect(screen.getByText('100%')).toBeInTheDocument();
  });
});
