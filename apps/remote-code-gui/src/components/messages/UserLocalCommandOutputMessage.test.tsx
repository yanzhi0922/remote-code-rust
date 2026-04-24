import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserLocalCommandOutputMessage } from './UserLocalCommandOutputMessage';

describe('UserLocalCommandOutputMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<UserLocalCommandOutputMessage output="done" />);
    expect(screen.getByTestId('user-local-command-output-message')).toBeInTheDocument();
  });

  it('displays output text', () => {
    render(<UserLocalCommandOutputMessage output="Hello World" />);
    expect(screen.getByText('Hello World')).toBeInTheDocument();
  });

  it('shows command in header', () => {
    render(
      <UserLocalCommandOutputMessage command="ls -la" output="files..." />,
    );
    expect(screen.getByText('ls -la')).toBeInTheDocument();
  });

  it('shows exit code 0 as green', () => {
    render(<UserLocalCommandOutputMessage output="ok" exitCode={0} />);
    const badge = screen.getByText('exit 0');
    expect(badge).toBeInTheDocument();
  });

  it('shows non-zero exit code as red', () => {
    render(<UserLocalCommandOutputMessage output="err" exitCode={1} />);
    const badge = screen.getByText('exit 1');
    expect(badge).toBeInTheDocument();
  });
});
