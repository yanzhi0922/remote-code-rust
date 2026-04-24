import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserImageMessage } from './UserImageMessage';

describe('UserImageMessage', () => {
  afterEach(cleanup);

  it('渲染图片消息', () => {
    render(<UserImageMessage src="https://example.com/img.png" />);
    expect(screen.getByTestId('user-image-message')).toBeInTheDocument();
  });

  it('设置正确的 src 属性', () => {
    render(<UserImageMessage src="https://example.com/photo.jpg" />);
    const img = screen.getByRole('img');
    expect(img).toHaveAttribute('src', 'https://example.com/photo.jpg');
  });

  it('设置默认 alt 文本', () => {
    render(<UserImageMessage src="https://example.com/img.png" />);
    const img = screen.getByRole('img');
    expect(img).toHaveAttribute('alt', 'Image');
  });

  it('支持自定义 alt 文本', () => {
    render(
      <UserImageMessage src="https://example.com/img.png" alt="Screenshot" />,
    );
    const img = screen.getByRole('img');
    expect(img).toHaveAttribute('alt', 'Screenshot');
  });

  it('图片有圆角样式', () => {
    const { container } = render(
      <UserImageMessage src="https://example.com/img.png" />,
    );
    const wrapper = container.querySelector('.rounded-2xl');
    expect(wrapper).toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <UserImageMessage
        src="https://example.com/img.png"
        className="img-custom"
      />,
    );
    expect(container.firstChild).toHaveClass('img-custom');
  });
});
