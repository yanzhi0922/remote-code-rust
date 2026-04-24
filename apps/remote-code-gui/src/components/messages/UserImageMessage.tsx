import { memo } from 'react';
import { ImageIcon } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 用户图片消息组件属性 */
export interface UserImageMessageProps {
  /** 图片 URL */
  src: string;
  /** 图片替代文本 */
  alt?: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 用户图片消息渲染组件。
 * 显示图片，带圆角和最大宽度限制。
 */
export const UserImageMessage = memo(function UserImageMessage({
  src,
  alt = 'Image',
  className,
}: UserImageMessageProps) {
  return (
    <div
      data-testid="user-image-message"
      className={cn('flex justify-end', className)}
    >
      <div className="max-w-md overflow-hidden rounded-2xl shadow-md">
        <img
          src={src}
          alt={alt}
          className="max-h-96 w-full object-contain"
          loading="lazy"
        />
      </div>
    </div>
  );
});
