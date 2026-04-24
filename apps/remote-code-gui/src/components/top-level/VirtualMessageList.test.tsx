import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { VirtualMessageList } from './VirtualMessageList';

afterEach(() => {
  cleanup();
});

describe('VirtualMessageList', () => {
  it('renders messages', () => {
    render(
      <VirtualMessageList>
        <div>Message 1</div>
        <div>Message 2</div>
      </VirtualMessageList>,
    );
    expect(screen.getByTestId('virtual-message-list')).toBeInTheDocument();
    expect(screen.getByText('Message 1')).toBeInTheDocument();
    expect(screen.getByText('Message 2')).toBeInTheDocument();
  });

  it('renders empty list', () => {
    render(<VirtualMessageList children={[]} />);
    expect(screen.getByTestId('virtual-message-list')).toBeInTheDocument();
  });
});
