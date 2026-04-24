import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserForkBoilerplateMessage } from './UserForkBoilerplateMessage';

describe('UserForkBoilerplateMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<UserForkBoilerplateMessage templateName="react-starter" />);
    expect(screen.getByTestId('user-fork-boilerplate-message')).toBeInTheDocument();
  });

  it('displays template name', () => {
    render(<UserForkBoilerplateMessage templateName="next-app" />);
    expect(screen.getByText(/next-app/)).toBeInTheDocument();
  });

  it('shows target path when provided', () => {
    render(
      <UserForkBoilerplateMessage templateName="t" targetPath="/home/proj" />,
    );
    expect(screen.getByText(/\/home\/proj/)).toBeInTheDocument();
  });

  it('shows completed status', () => {
    render(<UserForkBoilerplateMessage templateName="t" status="completed" />);
    expect(screen.getByText('已完成')).toBeInTheDocument();
  });

  it('shows failed status', () => {
    render(<UserForkBoilerplateMessage templateName="t" status="failed" />);
    expect(screen.getByText('失败')).toBeInTheDocument();
  });

  it('shows pending status by default', () => {
    render(<UserForkBoilerplateMessage templateName="t" />);
    expect(screen.getByText('等待中')).toBeInTheDocument();
  });
});
