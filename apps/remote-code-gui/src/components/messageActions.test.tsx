import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MessageActions, ToggleCollapse } from './messageActions';

describe('MessageActions', () => {
  afterEach(cleanup);

  it('渲染复制按钮', () => {
    render(<MessageActions text="测试文本" />);
    expect(screen.getByTitle('复制')).toBeInTheDocument();
  });

  it('点击复制按钮', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    render(<MessageActions text="测试文本" />);
    fireEvent.click(screen.getByTitle('复制'));
    expect(writeText).toHaveBeenCalledWith('测试文本');
  });

  it('不显示重新发送按钮（默认）', () => {
    render(<MessageActions text="文本" />);
    expect(screen.queryByTitle('重新发送')).not.toBeInTheDocument();
  });

  it('canResend=true 时显示重新发送按钮', () => {
    render(<MessageActions text="文本" canResend messageId="1" />);
    expect(screen.getByTitle('重新发送')).toBeInTheDocument();
  });

  it('点击重新发送按钮触发回调', () => {
    const onResend = vi.fn();
    render(
      <MessageActions text="文本" canResend messageId="msg-1" onResend={onResend} />,
    );
    fireEvent.click(screen.getByTitle('重新发送'));
    expect(onResend).toHaveBeenCalledWith('msg-1');
  });

  it('canEdit=true 时显示编辑按钮', () => {
    render(<MessageActions text="文本" canEdit messageId="1" />);
    expect(screen.getByTitle('编辑')).toBeInTheDocument();
  });

  it('点击编辑按钮触发回调', () => {
    const onEdit = vi.fn();
    render(
      <MessageActions text="文本" canEdit messageId="msg-1" onEdit={onEdit} />,
    );
    fireEvent.click(screen.getByTitle('编辑'));
    expect(onEdit).toHaveBeenCalledWith('msg-1');
  });
});

describe('ToggleCollapse', () => {
  afterEach(cleanup);

  it('展开状态显示向上箭头', () => {
    render(<ToggleCollapse isExpanded onToggle={vi.fn()} />);
    expect(screen.getByTitle('收起')).toBeInTheDocument();
  });

  it('收起状态显示向下箭头', () => {
    render(<ToggleCollapse isExpanded={false} onToggle={vi.fn()} />);
    expect(screen.getByTitle('展开')).toBeInTheDocument();
  });

  it('点击触发 onToggle', () => {
    const onToggle = vi.fn();
    render(<ToggleCollapse isExpanded onToggle={onToggle} />);
    fireEvent.click(screen.getByTitle('收起'));
    expect(onToggle).toHaveBeenCalled();
  });
});
