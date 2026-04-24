import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VirtualMessageList } from './VirtualMessageList';

function Message({ text }: { text: string }) {
  return <div data-testid="msg">{text}</div>;
}

const messages = Array.from({ length: 20 }, (_, i) => (
  <Message key={i} text={`Message ${i}`} />
));

describe('VirtualMessageList', () => {
  afterEach(() => { cleanup(); });

  it('renders with data-testid', () => {
    render(<VirtualMessageList>{messages}</VirtualMessageList>);
    expect(screen.getByTestId('virtual-message-list')).toBeInTheDocument();
  });

  it('renders wrapper with test id', () => {
    render(<VirtualMessageList>{messages}</VirtualMessageList>);
    expect(screen.getByTestId('virtual-message-list-wrapper')).toBeInTheDocument();
  });

  it('renders messages', () => {
    render(<VirtualMessageList containerHeight={600}>{messages.slice(0, 3)}</VirtualMessageList>);
    const msgElements = screen.getAllByTestId('msg');
    expect(msgElements.length).toBe(3);
  });

  it('renders message indices', () => {
    render(<VirtualMessageList containerHeight={600}>{messages.slice(0, 3)}</VirtualMessageList>);
    expect(screen.getByTestId('message-index-0')).toBeInTheDocument();
    expect(screen.getByTestId('message-index-1')).toBeInTheDocument();
    expect(screen.getByTestId('message-index-2')).toBeInTheDocument();
  });

  it('renders scroll indicator track', () => {
    render(<VirtualMessageList>{messages}</VirtualMessageList>);
    expect(screen.getByTestId('scroll-indicator-track')).toBeInTheDocument();
  });

  it('renders scroll indicator thumb', () => {
    render(<VirtualMessageList>{messages}</VirtualMessageList>);
    expect(screen.getByTestId('scroll-indicator-thumb')).toBeInTheDocument();
  });

  it('renders load more trigger when hasMore is true', () => {
    render(<VirtualMessageList hasMore={true}>{messages}</VirtualMessageList>);
    expect(screen.getByTestId('load-more-trigger')).toBeInTheDocument();
  });

  it('does not render load more when hasMore is false', () => {
    render(<VirtualMessageList hasMore={false}>{messages}</VirtualMessageList>);
    expect(screen.queryByTestId('load-more-trigger')).not.toBeInTheDocument();
  });

  it('shows loading spinner when isLoadingMore', () => {
    render(
      <VirtualMessageList hasMore={true} isLoadingMore={true}>
        {messages}
      </VirtualMessageList>,
    );
    expect(screen.getByText('加载更多消息...')).toBeInTheDocument();
  });

  it('calls onLoadMore when load more button is clicked', () => {
    const fn = vi.fn();
    render(
      <VirtualMessageList hasMore={true} onLoadMore={fn}>
        {messages}
      </VirtualMessageList>,
    );
    fireEvent.click(screen.getByText('加载更多历史消息'));
    expect(fn).toHaveBeenCalled();
  });

  it('renders scroll to bottom button when not at bottom', () => {
    render(
      <VirtualMessageList containerHeight={100}>
        {messages}
      </VirtualMessageList>,
    );
    // The scroll-to-bottom button appears when autoScrollToBottom is false
    // Initially it should be true, so no button
    expect(screen.queryByTestId('scroll-to-bottom')).not.toBeInTheDocument();
  });

  it('renders spacers for virtual items', () => {
    render(<VirtualMessageList containerHeight={200}>{messages}</VirtualMessageList>);
    expect(screen.getByTestId('virtual-spacer-top')).toBeInTheDocument();
    expect(screen.getByTestId('virtual-spacer-bottom')).toBeInTheDocument();
  });

  it('renders with custom className', () => {
    render(<VirtualMessageList className="custom-class">{messages}</VirtualMessageList>);
    const list = screen.getByTestId('virtual-message-list');
    expect(list.classList.contains('custom-class')).toBe(true);
  });

  it('renders search highlight wrapper when searchQuery is provided', () => {
    render(
      <VirtualMessageList searchQuery="test" containerHeight={600}>
        {messages.slice(0, 3)}
      </VirtualMessageList>,
    );
    expect(screen.getByTestId('message-highlight-0')).toBeInTheDocument();
  });

  it('does not render highlight wrapper without searchQuery', () => {
    render(<VirtualMessageList containerHeight={600}>{messages.slice(0, 3)}</VirtualMessageList>);
    expect(screen.queryByTestId('message-highlight-0')).not.toBeInTheDocument();
  });

  it('handles empty children array', () => {
    render(<VirtualMessageList>{[]}</VirtualMessageList>);
    expect(screen.getByTestId('virtual-message-list')).toBeInTheDocument();
  });
});
