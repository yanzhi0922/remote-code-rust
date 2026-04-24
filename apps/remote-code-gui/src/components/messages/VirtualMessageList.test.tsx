import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { VirtualMessageList } from './VirtualMessageList';
import type { ConversationEntry } from '../../lib/types';

beforeAll(() => {
  class MockIntersectionObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  Object.defineProperty(globalThis, 'IntersectionObserver', {
    value: MockIntersectionObserver,
    writable: true,
  });
});

function makeEntry(text: string): ConversationEntry {
  return {
    role: 'user',
    text,
    content_blocks: [],
    tool_calls: [],
    tool_call_id: null,
    name: null,
    is_error: false,
  };
}

describe('VirtualMessageList', () => {
  afterEach(cleanup);

  it('渲染虚拟消息列表', () => {
    const messages = [makeEntry('msg-1'), makeEntry('msg-2'), makeEntry('msg-3')];
    render(
      <VirtualMessageList messages={messages}>
        {(entry) => <div>{entry.text}</div>}
      </VirtualMessageList>,
    );
    expect(screen.getByTestId('virtual-message-list')).toBeInTheDocument();
  });

  it('渲染可见消息', () => {
    const messages = [makeEntry('visible-msg')];
    render(
      <VirtualMessageList messages={messages}>
        {(entry) => <div>{entry.text}</div>}
      </VirtualMessageList>,
    );
    expect(screen.getByText('visible-msg')).toBeInTheDocument();
  });

  it('空消息列表渲染空容器', () => {
    const { container } = render(
      <VirtualMessageList messages={[]}>
        {() => <div>item</div>}
      </VirtualMessageList>,
    );
    expect(container.querySelector('[data-testid="virtual-message-list"]')).toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <VirtualMessageList messages={[makeEntry('test')]} className="vlist-custom">
        {(entry) => <div>{entry.text}</div>}
      </VirtualMessageList>,
    );
    expect(container.firstChild).toHaveClass('vlist-custom');
  });

  it('传递正确的 index 给 children', () => {
    const messages = [makeEntry('a'), makeEntry('b')];
    const indices: number[] = [];
    render(
      <VirtualMessageList messages={messages}>
        {(_entry, index) => {
          indices.push(index);
          return <div>{_entry.text}</div>;
        }}
      </VirtualMessageList>,
    );
    expect(indices).toContain(0);
    expect(indices).toContain(1);
  });

  it('使用 overscan 属性', () => {
    const messages = Array.from({ length: 30 }, (_, i) => makeEntry(`msg-${i}`));
    render(
      <VirtualMessageList messages={messages} overscan={10}>
        {(entry) => <div>{entry.text}</div>}
      </VirtualMessageList>,
    );
    expect(screen.getByTestId('virtual-message-list')).toBeInTheDocument();
  });
});
